mod backend;
mod demo;
mod editor;
mod model;
mod ui;

#[cfg(test)]
mod tests;

use std::{
    io::{self, IsTerminal},
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use crossterm::{
    cursor::{Hide, Show},
    event::{
        self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        Event, KeyEventKind, MouseButton, MouseEvent, MouseEventKind,
    },
    execute,
    style::ResetColor,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend, layout::Position};

use crate::config::ConfigManager;

use self::{
    backend::Backend,
    model::{Focus, Model, Workspace},
    ui::HitMap,
};

pub struct Options {
    pub remote: Option<String>,
    pub local: Option<PathBuf>,
    pub demo: bool,
    pub mouse: bool,
}

pub fn run(options: Options) -> Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        bail!(
            "tui requires an interactive terminal on stdin and stdout; use thufs ls or thufs --json repos in scripts"
        );
    }
    if std::env::var("TERM").as_deref() == Ok("dumb") {
        bail!("tui requires a terminal with cursor control (TERM must not be dumb)");
    }
    let local = options.local.unwrap_or(std::env::current_dir()?);
    let local = std::fs::canonicalize(&local)
        .with_context(|| format!("Cannot open local directory {}", local.display()))?;
    if !local.is_dir() {
        bail!("--local must point to an existing directory");
    }
    let initial_remote = if options.demo {
        options.remote
    } else {
        let config = ConfigManager::new().load_resolved()?;
        if config.token.as_ref().is_none_or(|t| t.trim().is_empty()) {
            bail!("no token configured; set THUFS_TOKEN or run thufs auth set-token <token>");
        }
        options.remote.or(config.default_repo)
    };
    let mut model = Model::new(local, initial_remote, options.demo);
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        previous_hook(info);
    }));
    let _session = Session::enter(options.mouse)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))
        .context("Cannot initialize terminal renderer")?;
    let (sender, receiver) = mpsc::channel();
    let mut backend = Backend::new(sender, options.demo);
    let mut last_click: Option<(Instant, u16, u16)> = None;
    while !model.should_quit {
        for request in std::mem::take(&mut model.requests) {
            backend.dispatch(request);
        }
        for message in receiver.try_iter().take(256) {
            model.handle_message(message);
        }
        backend.pump();
        let mut hits = HitMap::default();
        terminal.draw(|frame| {
            hits = ui::draw(frame, &model);
        })?;
        if event::poll(Duration::from_millis(60))? {
            match event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => model.handle_key(key),
                Event::Paste(text) => model.paste(&text),
                Event::Mouse(mouse) if model.overlay.is_none() => {
                    handle_mouse(&mut model, mouse, &hits, &mut last_click)
                }
                _ => {}
            }
        }
        model.tick = model.tick.wrapping_add(1);
    }
    // Deliver cancellations requested by the final keypress before joining transfer workers.
    for request in std::mem::take(&mut model.requests) {
        backend.dispatch(request);
    }
    drop(backend);
    Ok(())
}

struct Session;
impl Session {
    fn enter(mouse: bool) -> Result<Self> {
        enable_raw_mode().context("Cannot enter raw terminal mode")?;
        let guard = Self;
        execute!(
            io::stdout(),
            EnterAlternateScreen,
            EnableBracketedPaste,
            Hide
        )?;
        if mouse {
            execute!(io::stdout(), EnableMouseCapture)?;
        }
        Ok(guard)
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        restore();
    }
}

fn restore() {
    let _ = execute!(
        io::stdout(),
        ResetColor,
        Show,
        DisableBracketedPaste,
        DisableMouseCapture,
        LeaveAlternateScreen
    );
    let _ = disable_raw_mode();
}

fn handle_mouse(
    model: &mut Model,
    mouse: MouseEvent,
    hits: &HitMap,
    last_click: &mut Option<(Instant, u16, u16)>,
) {
    let point = Position::new(mouse.column, mouse.row);
    match mouse.kind {
        MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
            if hits.libraries.contains(point) {
                model.focus = Focus::Libraries;
            } else if hits.files.contains(point) {
                model.focus = Focus::Files;
            } else {
                return;
            }
            model.move_cursor(if mouse.kind == MouseEventKind::ScrollUp {
                -3
            } else {
                3
            });
        }
        MouseEventKind::Down(MouseButton::Left) => {
            let double = last_click.is_some_and(|(time, x, y)| {
                time.elapsed() < Duration::from_millis(400) && x == mouse.column && y == mouse.row
            });
            *last_click = Some((Instant::now(), mouse.column, mouse.row));
            for (rect, workspace) in &hits.tabs {
                if rect.contains(point) {
                    model.switch(*workspace);
                    return;
                }
            }
            if hits.command.contains(point) {
                model.open_palette();
                return;
            }
            if hits.libraries.contains(point) {
                model.focus = Focus::Libraries;
                model.library_cursor = (hits.library_start
                    + (mouse.row - hits.libraries.y) as usize)
                    .min(model.repos.len().saturating_sub(1));
                if double {
                    model.open_library();
                }
            } else if hits.files.contains(point) {
                let row = mouse.row - hits.files.y;
                if row < hits.file_header || hits.row_height == 0 {
                    return;
                }
                let index = hits.list_start + ((row - hits.file_header) / hits.row_height) as usize;
                model.focus = Focus::Files;
                match model.workspace {
                    Workspace::Files => {
                        model.cursor = index.min(model.file_indices().len().saturating_sub(1))
                    }
                    Workspace::Transfers => {
                        model.transfer_cursor = index.min(model.transfers.len().saturating_sub(1))
                    }
                    Workspace::Shares => {
                        model.share_cursor =
                            index.min(model.share_indices().len().saturating_sub(1))
                    }
                }
                if double {
                    model.enter();
                }
            }
        }
        _ => {}
    }
}
