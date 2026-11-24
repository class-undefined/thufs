use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, BorderType, Borders, Cell, Clear, Gauge, Paragraph, Row, Table, TableState, Wrap,
    },
};
use unicode_width::UnicodeWidthStr;

use crate::{config::DEFAULT_USER_AGENT, transfer::format_size};

use super::{
    editor::Editor,
    i18n::Language,
    model::{
        COMMANDS, Focus, Form, FormKind, Model, Overlay, Picker, Setting, SettingsCategory,
        TransferStatus, UpdateState, Workspace, palette_matches_in,
    },
};

#[derive(Clone, Copy)]
struct Theme {
    language: Language,
    bg: Color,
    panel: Color,
    text: Color,
    muted: Color,
    border: Color,
    accent: Color,
    mint: Color,
    red: Color,
    selected: Color,
}

impl Theme {
    fn new(language: Language) -> Self {
        if std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty()) {
            Self {
                language,
                bg: Color::Reset,
                panel: Color::Reset,
                text: Color::Reset,
                muted: Color::Reset,
                border: Color::Reset,
                accent: Color::Reset,
                mint: Color::Reset,
                red: Color::Reset,
                selected: Color::Reset,
            }
        } else {
            Self {
                language,
                bg: Color::Rgb(16, 18, 26),
                panel: Color::Rgb(21, 24, 34),
                text: Color::Rgb(222, 226, 237),
                muted: Color::Rgb(133, 144, 166),
                border: Color::Rgb(49, 56, 74),
                accent: Color::Rgb(190, 168, 255),
                mint: Color::Rgb(132, 218, 190),
                red: Color::Rgb(255, 148, 155),
                selected: Color::Rgb(49, 42, 70),
            }
        }
    }
    fn plain(self) -> Style {
        Style::default().fg(self.text).bg(self.bg)
    }
    fn muted(self) -> Style {
        let style = Style::default().fg(self.muted);
        if self.accent == Color::Reset {
            style.add_modifier(Modifier::DIM)
        } else {
            style
        }
    }
    fn accent(self) -> Style {
        Style::default()
            .fg(self.accent)
            .add_modifier(Modifier::BOLD)
    }
    fn selected(self) -> Style {
        if self.accent == Color::Reset {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default().bg(self.selected).fg(self.text)
        }
    }
}

#[derive(Default)]
pub struct HitMap {
    pub check_updates: Rect,
    pub search: Rect,
    pub libraries: Rect,
    pub files: Rect,
    pub file_header: u16,
    pub row_height: u16,
    pub list_start: usize,
    pub library_start: usize,
    pub tabs: Vec<(Rect, Workspace)>,
    pub settings_categories: Vec<(Rect, SettingsCategory)>,
    pub dialog: DialogHits,
}

#[derive(Default)]
pub struct DialogHits {
    pub fields: Vec<(Rect, usize)>,
    pub browse: Vec<(Rect, usize)>,
    pub picker: Option<PickerHits>,
}

pub struct PickerHits {
    pub input: Rect,
    pub browse: Rect,
    pub parent: Rect,
    pub list: Rect,
    pub start: usize,
    pub rows: usize,
    pub select: Rect,
    pub cancel: Rect,
}

pub fn draw(frame: &mut Frame, model: &Model) -> HitMap {
    let theme = Theme::new(model.language());
    let area = frame.area();
    frame.render_widget(Block::default().style(theme.plain()), area);
    if area.width < 60 || area.height < 18 {
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled("thufs", theme.accent()),
                Line::raw(""),
                Line::raw(
                    theme
                        .language
                        .tr("This workspace needs at least 60 × 18 cells."),
                ),
                Line::raw(
                    theme
                        .language
                        .tr("Resize your terminal, or press q to quit."),
                ),
            ])
            .style(theme.plain())
            .wrap(Wrap { trim: false }),
            area.inner(Margin::new(2, 2)),
        );
        return HitMap::default();
    }
    let root = area.inner(Margin::new(1, 0));
    let regions = Layout::vertical([
        Constraint::Length(4),
        Constraint::Min(8),
        Constraint::Length(3),
    ])
    .split(root);
    let mut hits = HitMap::default();
    header(frame, model, regions[0], theme, &mut hits);
    match model.workspace {
        Workspace::Files => files_workspace(frame, model, regions[1], theme, &mut hits),
        Workspace::Transfers => transfers(frame, model, regions[1], theme, &mut hits),
        Workspace::Shares => shares(frame, model, regions[1], theme, &mut hits),
        Workspace::Settings => settings(frame, model, regions[1], theme, &mut hits),
        Workspace::About => about(frame, model, regions[1], theme, &mut hits),
    }
    footer(frame, model, regions[2], theme, &mut hits);
    if let Some(overlay) = &model.overlay {
        draw_overlay(frame, model, overlay, theme, &mut hits.dialog);
    }
    hits
}

fn header(frame: &mut Frame, model: &Model, area: Rect, t: Theme, hits: &mut HitMap) {
    let top = Layout::horizontal([
        Constraint::Min(24),
        Constraint::Length(if area.width >= 96 { 41 } else { 24 }),
    ])
    .split(Rect {
        y: area.y + 1,
        height: 1,
        ..area
    });
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" ▰ ", t.accent()),
            Span::styled(
                "thufs",
                Style::default().fg(t.text).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  /  {}", t.language.tr("cloud workspace")),
                t.muted(),
            ),
        ])),
        top[0],
    );
    let (status, color) = if model.demo {
        ("DEMO · offline", t.accent)
    } else if !model.signed_in {
        ("Not signed in", t.muted)
    } else if model.connected {
        ("● connected", t.mint)
    } else if model.repos_loading {
        ("◌ connecting", t.muted)
    } else {
        ("○ disconnected", t.red)
    };
    let mut right = vec![Span::styled(
        t.language.tr(status),
        Style::default().fg(color),
    )];
    if area.width >= 96 {
        right.insert(0, Span::styled("cloud.tsinghua.edu.cn  ", t.muted()));
    }
    frame.render_widget(
        Paragraph::new(Line::from(right)).alignment(Alignment::Right),
        top[1],
    );
    let labels = [
        ("Files", Workspace::Files),
        ("Transfers", Workspace::Transfers),
        ("Shares", Workspace::Shares),
        ("Settings", Workspace::Settings),
        ("About", Workspace::About),
    ];
    let mut x = area.x + 1;
    for (index, (label, workspace)) in labels.into_iter().enumerate() {
        let compact = area.width < 80;
        let label = format!(
            "{}{}{}",
            index + 1,
            if compact { " " } else { "  " },
            t.language.tr(label)
        );
        let padding = if compact { " " } else { "  " };
        let width = cell_width(&label) as u16 + padding.len() as u16 * 2;
        let rect = Rect::new(x, area.y + 3, width, 1);
        let style = if model.workspace == workspace {
            t.selected().fg(t.accent).add_modifier(Modifier::BOLD)
        } else {
            t.muted()
        };
        frame.render_widget(
            Paragraph::new(format!("{padding}{label}{padding}")).style(style),
            rect,
        );
        hits.tabs.push((rect, workspace));
        x += width + 1;
    }
    let active = model.active_transfers();
    if active > 0 && area.width > 95 {
        frame.render_widget(
            Paragraph::new(format!(
                "{} {}  ",
                spinner(model.tick),
                t.language.format("{0} active", &[active.to_string()])
            ))
            .style(Style::default().fg(t.mint))
            .alignment(Alignment::Right),
            Rect::new(area.x + area.width - 22, area.y + 3, 22, 1),
        );
    }
}

fn files_workspace(frame: &mut Frame, model: &Model, area: Rect, t: Theme, hits: &mut HitMap) {
    let area = Rect {
        y: area.y + 1,
        height: area.height.saturating_sub(1),
        ..area
    };
    let sidebar = area.width >= 88;
    if !sidebar && model.focus == Focus::Libraries {
        libraries(frame, model, area, t, hits);
        return;
    }
    let inspect = model.inspector && area.width >= 116;
    let regions = Layout::horizontal([
        Constraint::Length(if sidebar { 23 } else { 0 }),
        Constraint::Min(30),
        Constraint::Length(if inspect { 29 } else { 0 }),
    ])
    .split(area);
    if sidebar {
        libraries(frame, model, regions[0], t, hits);
    }
    files(frame, model, regions[1], t, hits);
    if inspect {
        inspector(frame, model, regions[2], t);
    }
}

fn libraries(frame: &mut Frame, model: &Model, area: Rect, t: Theme, hits: &mut HitMap) {
    let area = area.inner(Margin::new(1, 0));
    let header = Line::from(vec![
        Span::styled(
            t.language.tr("LIBRARIES"),
            if model.focus == Focus::Libraries {
                t.accent()
            } else {
                t.muted()
            },
        ),
        Span::styled(format!("  {:02}", model.repos.len()), t.muted()),
    ]);
    frame.render_widget(Paragraph::new(header), Rect { height: 1, ..area });
    let footer_height = if area.height >= 17 { 8 } else { 3 };
    let list = Rect::new(
        area.x,
        area.y + 2,
        area.width,
        area.height.saturating_sub(footer_height + 2),
    );
    hits.libraries = list;
    let start = scroll_start(
        model.library_cursor,
        list.height as usize,
        model.repos.len(),
    );
    hits.library_start = start;
    for (row, repo) in model
        .repos
        .iter()
        .enumerate()
        .skip(start)
        .take(list.height as usize)
    {
        let current = model
            .location
            .as_ref()
            .is_some_and(|l| l.repo_id == repo.id);
        let focused = model.focus == Focus::Libraries && row == model.library_cursor;
        let line = Line::from(vec![
            Span::styled(
                if current { "▰  " } else { "▱  " },
                Style::default().fg(if current { t.accent } else { t.muted }),
            ),
            Span::raw(safe(&repo.name)),
        ]);
        frame.render_widget(
            Paragraph::new(line).style(if focused {
                t.selected().add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(if current { t.text } else { t.muted })
            }),
            Rect::new(list.x, list.y + (row - start) as u16, list.width, 1),
        );
    }
    if model.repos_loading && model.repos.is_empty() {
        frame.render_widget(
            Paragraph::new(format!(
                "{} {}",
                spinner(model.tick),
                t.language.tr("Loading libraries")
            ))
            .style(t.muted()),
            list,
        );
    } else if model.repos.is_empty() {
        frame.render_widget(
            Paragraph::new(t.language.tr("No libraries yet")).style(t.muted()),
            list,
        );
    }
    let bottom = Rect::new(
        area.x,
        area.bottom().saturating_sub(footer_height),
        area.width,
        footer_height,
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("N", t.accent()),
            Span::styled(t.language.tr("  New library"), t.muted()),
        ])),
        Rect {
            height: 1,
            ..bottom
        },
    );
    if footer_height >= 8 {
        let name = model
            .account
            .as_ref()
            .and_then(|a| a.name.as_deref())
            .unwrap_or("THU Cloud Drive");
        frame.render_widget(
            Paragraph::new(safe(name)).style(Style::default().fg(t.text)),
            Rect::new(bottom.x, bottom.y + 3, bottom.width, 1),
        );
        if let Some((usage, total)) = model
            .account
            .as_ref()
            .and_then(|a| a.usage.zip(a.total))
            .filter(|(_, total)| *total > 0)
        {
            frame.render_widget(
                Gauge::default()
                    .ratio((usage as f64 / total as f64).clamp(0.0, 1.0))
                    .gauge_style(Style::default().fg(t.mint).bg(t.border))
                    .label(""),
                Rect::new(bottom.x, bottom.y + 5, bottom.width.saturating_sub(2), 1),
            );
            frame.render_widget(
                Paragraph::new(format!("{} / {}", format_size(usage), format_size(total)))
                    .style(t.muted()),
                Rect::new(bottom.x, bottom.y + 6, bottom.width, 1),
            );
        } else {
            frame.render_widget(
                Paragraph::new(t.language.tr("Your files, in the terminal."))
                    .style(t.muted())
                    .wrap(Wrap { trim: false }),
                Rect::new(bottom.x, bottom.y + 5, bottom.width, 2),
            );
        }
    }
}

fn panel(title: impl Into<String>, active: bool, t: Theme) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if active { t.accent } else { t.border }))
        .style(Style::default().bg(t.panel).fg(t.text))
        .title(t.language.tr(&title.into()).into_owned())
        .title_style(if active { t.accent() } else { t.muted() })
}

fn files(frame: &mut Frame, model: &Model, area: Rect, t: Theme, hits: &mut HitMap) {
    let block = panel(" FILES ", model.focus == Focus::Files, t);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let inner = inner.inner(Margin::new(1, 0));
    let compact = inner.height < 17;
    let minimal = inner.height < 9;
    if let Some(location) = &model.location {
        let title = Line::from(vec![
            Span::styled(safe(&location.repo), t.accent()),
            Span::styled(
                if location.path == "/" {
                    "  /".into()
                } else {
                    format!(
                        "  /  {}",
                        safe(location.path.trim_start_matches('/')).replace('/', " / ")
                    )
                },
                Style::default().fg(t.text),
            ),
        ]);
        frame.render_widget(
            Paragraph::new(title),
            Rect {
                y: inner.y + u16::from(!compact),
                height: 1,
                ..inner
            },
        );
    }
    let filtered = model.file_indices();
    let search_y = inner.y + if compact { 1 } else { 2 };
    search_field(
        frame,
        model,
        Rect::new(inner.x, search_y, inner.width, 1),
        t,
        hits,
    );
    let count = if model.filter.text.is_empty() {
        t.language
            .format("{0} items", &[model.file_entries().len().to_string()])
    } else {
        t.language.format(
            "{0} of {1} items",
            &[
                filtered.len().to_string(),
                model.file_entries().len().to_string(),
            ],
        )
    };
    if !minimal {
        frame.render_widget(
            Paragraph::new(format!("{count}  ·  {}", t.language.tr(model.sort.label())))
                .style(t.muted()),
            Rect::new(inner.x, search_y + 1, inner.width, 1),
        );
    }
    let top = if minimal {
        3
    } else if compact {
        4
    } else {
        5
    };
    let body = Rect::new(
        inner.x,
        inner.y + top,
        inner.width,
        inner
            .height
            .saturating_sub(top + if compact { 1 } else { 2 }),
    );
    hits.files = body;
    hits.file_header = if compact { 1 } else { 2 };
    hits.row_height = 1;
    let rows_height = body.height.saturating_sub(hits.file_header) as usize;
    let start = scroll_start(model.cursor, rows_height, filtered.len());
    hits.list_start = start;
    if model.files_loading() {
        empty(
            frame,
            body,
            spinner(model.tick),
            if model.searching_library() {
                "Searching the entire library…"
            } else {
                "Loading your files"
            },
            "You can keep using commands while this loads.",
            t,
        );
        return;
    }
    if let Some(error) = model.files_error() {
        empty(
            frame,
            body,
            "!",
            "Could not load this directory",
            &format!("{}\n\nr  Retry    Tab  Libraries    ?  Help", safe(error)),
            t,
        );
        return;
    }
    if model.location.is_none() {
        empty(
            frame,
            body,
            "◇",
            if model.signed_in {
                "Your cloud workspace"
            } else {
                "Not signed in"
            },
            if model.signed_in {
                "Tab to choose a library, or N to create one.\nCtrl+P opens commands."
            } else {
                "Sign in to browse your cloud drive.\n4  Settings    Ctrl+P  Commands"
            },
            t,
        );
        return;
    }
    if filtered.is_empty() {
        empty(
            frame,
            body,
            "◇",
            if model.filter.text.is_empty() {
                "A little room for what's next"
            } else {
                "No matching files"
            },
            if model.filter.text.is_empty() {
                "u  Upload a file    n  New folder"
            } else {
                "Change your filter, or press Esc to clear it."
            },
            t,
        );
        return;
    }
    let rows = filtered.iter().skip(start).take(rows_height).map(|&index| {
        let file = &model.file_entries()[index];
        let dir = file.kind == "dir";
        let selected = model.selected.contains(&file.path);
        Row::new(vec![
            Cell::from(if selected {
                "●"
            } else if dir {
                "▸"
            } else {
                "·"
            })
            .style(Style::default().fg(if selected {
                t.mint
            } else if dir {
                t.accent
            } else {
                t.muted
            })),
            Cell::from(format!(
                "{}{}",
                safe(if model.searching_library() {
                    file.path.trim_start_matches('/')
                } else {
                    &file.name
                }),
                if dir { "/" } else { "" }
            ))
            .style(Style::default().fg(if dir { t.accent } else { t.text })),
            Cell::from(file.size.map(format_size).unwrap_or_else(|| "—".into())).style(t.muted()),
        ])
    });
    let header = Row::new(["", "NAME", "SIZE"].map(|key| t.language.tr(key)))
        .style(t.muted())
        .bottom_margin(u16::from(!compact));
    let table = Table::new(
        rows,
        [
            Constraint::Length(2),
            Constraint::Min(8),
            Constraint::Length(10),
        ],
    )
    .header(header)
    .column_spacing(1)
    .row_highlight_style(t.selected())
    .highlight_symbol("› ");
    let mut state = TableState::default().with_selected(Some(model.cursor.saturating_sub(start)));
    frame.render_stateful_widget(table, body, &mut state);
    let foot = if model.selected.is_empty() {
        t.language
            .tr("Space select items · a select all")
            .into_owned()
    } else {
        t.language.format(
            "{0} selected · d download · Esc clear",
            &[model.selected.len().to_string()],
        )
    };
    let summary = Layout::horizontal([Constraint::Min(10), Constraint::Length(9)]).split(
        Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
    );
    frame.render_widget(Paragraph::new(foot).style(t.muted()), summary[0]);
    frame.render_widget(
        Paragraph::new(format!("{}/{}", model.cursor + 1, filtered.len()))
            .style(t.muted())
            .alignment(Alignment::Right),
        summary[1],
    );
}

fn inspector(frame: &mut Frame, model: &Model, area: Rect, t: Theme) {
    let area = area.inner(Margin::new(2, 0));
    frame.render_widget(
        Paragraph::new(t.language.tr("INSPECTOR")).style(t.muted()),
        Rect { height: 1, ..area },
    );
    let Some(file) = model.current_file() else {
        frame.render_widget(
            Paragraph::new(t.language.tr("Select a file to see its details."))
                .style(t.muted())
                .wrap(Wrap { trim: true }),
            Rect::new(area.x, area.y + 3, area.width, 3),
        );
        return;
    };
    let dir = file.kind == "dir";
    let kind = if dir {
        t.language.tr("FOLDER").into_owned()
    } else {
        file.name
            .rsplit_once('.')
            .map(|(_, ext)| format!("{} {}", ext.to_uppercase(), t.language.tr("FILE")))
            .unwrap_or_else(|| t.language.tr("FILE").into_owned())
    };
    let mut lines = vec![
        Line::raw(""),
        Line::styled(safe(&kind), t.accent()),
        Line::raw(""),
        Line::styled(
            safe(&file.name),
            Style::default().fg(t.text).add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
        Line::styled(t.language.tr("SIZE"), t.muted()),
        Line::raw(file.size.map(format_size).unwrap_or_else(|| "—".into())),
        Line::raw(""),
        Line::styled(t.language.tr("MODIFIED"), t.muted()),
        Line::raw(safe(&crate::datetime::display(
            file.updated_at.as_deref().unwrap_or("—"),
        ))),
        Line::raw(""),
        Line::styled(t.language.tr("LOCATION"), t.muted()),
        Line::raw(
            model
                .location
                .as_ref()
                .map(|l| safe(&l.repo))
                .unwrap_or_default(),
        ),
        Line::styled(safe(&file.path), t.muted()),
        Line::raw(""),
        Line::styled(t.language.tr("ACTIONS"), t.muted()),
    ];
    lines.push(compact_hint(
        if dir { "enter" } else { "d" },
        if dir { "Open folder" } else { "Download" },
        t,
    ));
    if dir {
        lines.push(compact_hint("d", "Download folder", t));
    }
    lines.push(compact_hint("s", "Create share link", t));
    lines.push(compact_hint("y", "Copy remote path", t));
    if area.height < 24 {
        lines.retain(|line| line.width() > 0);
    }
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().fg(t.text))
            .wrap(Wrap { trim: false }),
        Rect::new(
            area.x,
            area.y + 1,
            area.width,
            area.height.saturating_sub(1),
        ),
    );
}

fn transfers(frame: &mut Frame, model: &Model, area: Rect, t: Theme, hits: &mut HitMap) {
    let area = Rect {
        y: area.y + 1,
        height: area.height.saturating_sub(1),
        ..area
    };
    let block = panel(" TRANSFERS ", true, t);
    let inner = block.inner(area).inner(Margin::new(2, 0));
    frame.render_widget(block, area);
    let active = model.active_transfers();
    let complete = model
        .transfers
        .iter()
        .filter(|t| t.status == TransferStatus::Done)
        .count();
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                t.language.format("{0} active", &[active.to_string()]),
                t.accent(),
            ),
            Span::styled(
                format!(
                    "  ·  {}  ·  {}",
                    t.language.format("{0} completed", &[complete.to_string()]),
                    t.language
                        .format("{0} total", &[model.transfers.len().to_string()])
                ),
                t.muted(),
            ),
        ])),
        Rect {
            y: inner.y + 1,
            height: 1,
            ..inner
        },
    );
    let body = Rect::new(
        inner.x,
        inner.y + 3,
        inner.width,
        inner.height.saturating_sub(5),
    );
    hits.files = body;
    hits.row_height = 4;
    hits.file_header = 0;
    let visible = (body.height / 4) as usize;
    let start = scroll_start(model.transfer_cursor, visible, model.transfers.len());
    hits.list_start = start;
    if model.transfers.is_empty() {
        empty(
            frame,
            body,
            "⇅",
            "A clear runway",
            "Upload or download a file from the Files workspace.\nTransfers keep moving while you browse.",
            t,
        );
    }
    for (index, transfer) in model.transfers.iter().enumerate().skip(start).take(visible) {
        let row = Rect::new(body.x, body.y + ((index - start) * 4) as u16, body.width, 3);
        if index == model.transfer_cursor {
            frame.render_widget(Block::default().style(t.selected()), row);
        }
        let color = match transfer.status {
            TransferStatus::Done => t.mint,
            TransferStatus::Failed => t.red,
            TransferStatus::Running => t.accent,
            _ => t.muted,
        };
        let symbol = match transfer.status {
            TransferStatus::Done => "✓",
            TransferStatus::Failed => "!",
            TransferStatus::Running => spinner(model.tick),
            TransferStatus::Cancelled => "×",
            TransferStatus::Queued => "·",
        };
        let columns = Layout::horizontal([Constraint::Min(20), Constraint::Length(24)])
            .split(Rect { height: 1, ..row });
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(format!(" {symbol}  "), Style::default().fg(color)),
                Span::styled(
                    safe(&transfer.spec.name()),
                    Style::default().fg(t.text).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {}", t.language.tr(transfer.spec.direction())),
                    t.muted(),
                ),
            ])),
            columns[0],
        );
        frame.render_widget(
            Paragraph::new(format!(
                "{}  {:>3.0}% ",
                t.language.tr(transfer.status.label()),
                transfer.ratio() * 100.0
            ))
            .style(Style::default().fg(color))
            .alignment(Alignment::Right),
            columns[1],
        );
        let destination = transfer
            .final_path
            .clone()
            .unwrap_or_else(|| transfer.spec.destination());
        frame.render_widget(
            Paragraph::new(format!("    → {}", safe(&destination))).style(t.muted()),
            Rect::new(row.x, row.y + 1, row.width, 1),
        );
        let progress = Layout::horizontal([Constraint::Percentage(44), Constraint::Min(12)]).split(
            Rect::new(row.x + 4, row.y + 2, row.width.saturating_sub(5), 1),
        );
        frame.render_widget(
            Gauge::default()
                .ratio(transfer.ratio())
                .label("")
                .gauge_style(Style::default().fg(color).bg(t.border))
                .use_unicode(true),
            progress[0],
        );
        let speed = if transfer.status == TransferStatus::Running
            && transfer.elapsed().as_secs_f64() > 0.1
        {
            format!(
                "  ·  {}/s",
                format_size(
                    (transfer.transferred as f64 / transfer.elapsed().as_secs_f64()) as u64
                )
            )
        } else {
            String::new()
        };
        let detail = if transfer.status == TransferStatus::Failed {
            safe(&transfer.detail)
        } else {
            format!(
                "{} / {}{speed}",
                format_size(transfer.transferred),
                transfer
                    .total
                    .map(format_size)
                    .unwrap_or_else(|| "?".into())
            )
        };
        frame.render_widget(
            Paragraph::new(format!("  {detail}")).style(Style::default().fg(
                if transfer.status == TransferStatus::Failed {
                    t.red
                } else {
                    t.muted
                },
            )),
            progress[1],
        );
    }
}

fn search_field(frame: &mut Frame, model: &Model, area: Rect, t: Theme, hits: &mut HitMap) {
    hits.search = area;
    frame.render_widget(Block::default().style(t.selected()), area);
    let prefix = format!(
        "{} › ",
        t.language.tr(if model.workspace == Workspace::Shares {
            "Search shares"
        } else {
            "Search library"
        })
    );
    let prefix_width = cell_width(&prefix) as u16;
    frame.render_widget(Paragraph::new(prefix).style(t.accent()), area);
    let input = Rect::new(
        area.x + prefix_width,
        area.y,
        area.width.saturating_sub(prefix_width),
        1,
    );
    if model.filtering {
        editor(frame, &model.filter, input, false, t);
    } else if model.filter.text.is_empty() {
        frame.render_widget(
            Paragraph::new(t.language.tr(if model.workspace == Workspace::Shares {
                "Name, link or code · Ctrl+F"
            } else {
                "Name or path · Ctrl+F"
            }))
            .style(t.muted()),
            input,
        );
    } else {
        frame.render_widget(
            Paragraph::new(safe(&model.filter.text)).style(Style::default().fg(t.text)),
            input,
        );
    }
}

fn shares(frame: &mut Frame, model: &Model, area: Rect, t: Theme, hits: &mut HitMap) {
    let area = Rect {
        y: area.y + 1,
        height: area.height.saturating_sub(1),
        ..area
    };
    let block = panel(" SHARE LINKS ", true, t);
    let inner = block.inner(area).inner(Margin::new(2, 0));
    frame.render_widget(block, area);
    let indices = model.share_indices();
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(t.language.tr("Shared from your drive"), t.accent()),
            Span::styled(
                format!(
                    "  ·  {}",
                    if model.filter.text.trim().is_empty() {
                        t.language
                            .format("{0} links", &[model.shares.len().to_string()])
                    } else {
                        t.language.format(
                            "{0} of {1} links",
                            &[indices.len().to_string(), model.shares.len().to_string()],
                        )
                    }
                ),
                t.muted(),
            ),
        ])),
        Rect {
            y: inner.y + 1,
            height: 1,
            ..inner
        },
    );
    search_field(
        frame,
        model,
        Rect::new(inner.x, inner.y + 2, inner.width, 1),
        t,
        hits,
    );
    let body = Rect::new(
        inner.x,
        inner.y + 4,
        inner.width,
        inner.height.saturating_sub(4),
    );
    hits.files = body;
    hits.file_header = 2;
    hits.row_height = 2;
    if model.shares_loading {
        empty(
            frame,
            body,
            spinner(model.tick),
            "Loading share links",
            "",
            t,
        );
        return;
    }
    if let Some(error) = &model.shares_error {
        empty(
            frame,
            body,
            "!",
            "Could not load share links",
            &format!("{}\n\nr  Retry", safe(error)),
            t,
        );
        return;
    }
    let count = body.height.saturating_sub(2) as usize / 2;
    let start = scroll_start(model.share_cursor, count, indices.len());
    hits.list_start = start;
    if indices.is_empty() {
        empty(
            frame,
            body,
            "↗",
            if model.filter.text.is_empty() {
                "Good work is worth sharing"
            } else {
                "No matching links"
            },
            "Select a file or folder and press s to create a link.",
            t,
        );
        return;
    }
    let rows = indices.iter().skip(start).take(count).map(|&i| {
        let share = &model.shares[i];
        Row::new(vec![
            Cell::from(Text::from(vec![
                Line::styled(
                    safe(share.path.as_deref().unwrap_or("Shared item")),
                    Style::default().fg(t.text),
                ),
                Line::styled(safe(&share.url), t.muted()),
            ])),
            Cell::from(safe(share.repo.as_deref().unwrap_or("—"))).style(t.muted()),
            Cell::from(
                share
                    .view_count
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "—".into()),
            )
            .style(t.muted()),
            Cell::from(
                share
                    .expire_date
                    .as_deref()
                    .map(|date| safe(&crate::datetime::display(date)))
                    .unwrap_or_else(|| t.language.tr("No expiry").into_owned()),
            )
            .style(t.muted()),
        ])
        .height(2)
    });
    let widths = if body.width >= 90 {
        vec![
            Constraint::Min(28),
            Constraint::Length(18),
            Constraint::Length(7),
            Constraint::Length(20),
        ]
    } else {
        vec![
            Constraint::Min(20),
            Constraint::Length(11),
            Constraint::Length(5),
            Constraint::Length(12),
        ]
    };
    let table = Table::new(rows, widths)
        .header(
            Row::new(["FILE / LINK", "LIBRARY", "VIEWS", "EXPIRES"].map(|key| t.language.tr(key)))
                .style(t.muted())
                .bottom_margin(1),
        )
        .column_spacing(2)
        .highlight_symbol("› ")
        .row_highlight_style(t.selected());
    frame.render_stateful_widget(
        table,
        body,
        &mut TableState::default().with_selected(Some(model.share_cursor.saturating_sub(start))),
    );
}

fn footer(frame: &mut Frame, model: &Model, area: Rect, t: Theme, _hits: &mut HitMap) {
    let notice = model
        .notice
        .as_ref()
        .filter(|n| n.since.elapsed().as_secs() < 9);
    let text = if let Some(notice) = notice {
        format!(
            "{} {}",
            if notice.error { "!" } else { "✓" },
            t.language.message(&safe(&notice.text))
        )
    } else if model.quitting {
        t.language
            .tr("Cancelling active transfers before exit…")
            .into_owned()
    } else if model.demo {
        t.language
            .tr("Offline demo · cloud operations are disabled")
            .into_owned()
    } else {
        format!(
            "{}  {}",
            t.language.tr("Local"),
            safe(&model.local.display().to_string())
        )
    };
    let color = if notice.is_some_and(|n| n.error) {
        t.red
    } else if notice.is_some() {
        t.mint
    } else {
        t.muted
    };
    let columns = Layout::horizontal([
        Constraint::Min(10),
        Constraint::Length(if area.width >= 90 { 32 } else { 22 }),
    ])
    .split(Rect { height: 1, ..area });
    frame.render_widget(
        Paragraph::new(text).style(Style::default().fg(color)),
        columns[0],
    );
    let shortcut = if cfg!(target_os = "macos") {
        "Ctrl/⌘+P"
    } else {
        "Ctrl+P"
    };
    frame.render_widget(
        Paragraph::new(format!("{shortcut}  {}", t.language.tr("Commands")))
            .style(t.muted())
            .alignment(Alignment::Right),
        columns[1],
    );
    let hints = match model.workspace {
        Workspace::Files if model.focus == Focus::Libraries => {
            "↑↓ navigate   enter open library   tab files   N new library   4 settings   ? help   q quit"
        }
        Workspace::Files => {
            if area.width >= 100 {
                "↑↓ navigate   enter open   tab libraries   u upload   d download   s share   f find   4 settings   ? help"
            } else {
                "↑↓ move  enter open  tab libraries  u upload  4 settings  ? help"
            }
        }
        Workspace::Transfers => "↑↓ x cancel  r retry  enter details  Shift+C clear  ? help",
        Workspace::Shares => "d download  y copy  x revoke  f search  r refresh  ? help",
        Workspace::Settings => "tab category  ↑↓ option  enter change  esc back  q quit",
        Workspace::About => "u check updates   i details   esc back   q quit",
    };
    frame.render_widget(
        Paragraph::new(t.language.tr(if model.filtering {
            "Enter keeps results · Esc clears search"
        } else {
            hints
        }))
        .style(t.muted()),
        Rect::new(area.x, area.y + 1, area.width, 1),
    );
}

const ABOUT_LOGO: [(&str, &str); 5] = [
    (r" _____   _   _   _   _ ", r"  _____   ____  "),
    (r"|_   _| | | | | | | | |", r" |  ___| / ___| "),
    (r"  | |   | |_| | | | | |", r" | |_    \___ \ "),
    (r"  | |   |  _  | | |_| |", r" |  _|    ___) |"),
    (r"  |_|   |_| |_|  \___/ ", r" |_|     |____/ "),
];

const ABOUT_LOGO_COMPACT: [(&str, &str); 3] = [
    (r"___  | |  | |  ", r" __   __ "),
    (r" |   |_|  | |  ", r"|_   (__ "),
    (r" |   | |  |_|  ", r"|     __)"),
];

fn about(frame: &mut Frame, model: &Model, area: Rect, t: Theme, hits: &mut HitMap) {
    let block = panel(" ABOUT ", true, t);
    let inner = block.inner(area).inner(Margin::new(2, 1));
    frame.render_widget(block, area);
    let compact = inner.height < ABOUT_LOGO.len() as u16 + 9;
    let logo: &[(&str, &str)] = if compact {
        &ABOUT_LOGO_COMPACT
    } else {
        &ABOUT_LOGO
    };
    let mut lines: Vec<_> = logo
        .iter()
        .map(|&(thu, fs)| {
            Line::from(vec![
                Span::styled(thu, t.accent()),
                Span::styled(fs, Style::default().fg(t.mint).add_modifier(Modifier::BOLD)),
            ])
        })
        .collect();
    if !compact {
        lines.push(Line::raw(""));
    }
    let version = format!("thufs  v{}   ", env!("CARGO_PKG_VERSION"));
    let label = format!(" u {} ", t.language.tr("Check for updates"));
    let version_row = lines.len() as u16;
    let version_width = cell_width(&version) as u16;
    let button_width = cell_width(&label) as u16;
    lines.push(Line::from(vec![
        Span::styled(version, t.muted()),
        Span::styled(label, t.selected()),
    ]));
    let (status, style) = match &model.update {
        UpdateState::NotChecked => (t.language.tr("Updates not checked").into_owned(), t.muted()),
        UpdateState::Checking => (
            format!(
                "{} {}",
                spinner(model.tick),
                t.language.tr("Checking for updates…")
            ),
            t.muted(),
        ),
        UpdateState::Failed(_) => (
            t.language
                .tr("Check unavailable · press u to retry")
                .into_owned(),
            Style::default().fg(t.red),
        ),
        UpdateState::Checked(result) if result.update_available => (
            t.language.format(
                "{0} available · Enter for details",
                &[result.latest_version.clone().unwrap_or_default()],
            ),
            Style::default().fg(t.mint),
        ),
        UpdateState::Checked(result) if result.latest_version.is_none() => (
            t.language
                .tr("No stable release published yet")
                .into_owned(),
            t.muted(),
        ),
        UpdateState::Checked(_) => (
            t.language.tr("No newer release found.").into_owned(),
            t.muted(),
        ),
    };
    lines.push(Line::styled(status, style));
    if compact {
        lines.push(Line::from(vec![
            Span::styled(format!("{}  ", t.language.tr("Author")), t.muted()),
            Span::raw(env!("CARGO_PKG_AUTHORS")),
        ]));
        lines.push(Line::from(vec![
            Span::styled(format!("{}  ", t.language.tr("Repository")), t.muted()),
            Span::raw(env!("CARGO_PKG_REPOSITORY")),
        ]));
    } else {
        lines.extend([
            Line::raw(""),
            Line::styled(t.language.tr("Author"), t.muted()),
            Line::raw(env!("CARGO_PKG_AUTHORS")),
            Line::raw(""),
            Line::styled(t.language.tr("Repository"), t.muted()),
            Line::raw(env!("CARGO_PKG_REPOSITORY")),
        ]);
    }
    let content = Rect::new(
        inner.x,
        inner.y + inner.height.saturating_sub(lines.len() as u16) / 2,
        inner.width,
        lines.len() as u16,
    );
    hits.check_updates = Rect::new(
        content.x + content.width.saturating_sub(version_width + button_width) / 2 + version_width,
        content.y + version_row,
        button_width,
        1,
    );
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().fg(t.text))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false }),
        content,
    );
}

fn current_account_name(model: &Model, t: Theme) -> String {
    if !model.signed_in {
        return t.language.tr("Not signed in").into_owned();
    }
    let identity = model
        .account
        .as_ref()
        .and_then(|account| {
            account
                .name
                .as_deref()
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .or_else(|| (!account.email.trim().is_empty()).then(|| account.email.trim()))
        })
        .or_else(|| {
            model
                .settings
                .accounts
                .iter()
                .find(|account| {
                    Some(&account.key) == model.settings.active_account.as_ref()
                        && !account.key.starts_with('@')
                })
                .and_then(|account| {
                    (!account.label.trim().is_empty())
                        .then(|| account.label.trim())
                        .or_else(|| {
                            (!account.detail.trim().is_empty()).then(|| account.detail.trim())
                        })
                })
        });
    identity
        .map(safe)
        .unwrap_or_else(|| t.language.tr("Loading account…").into_owned())
}

fn settings(frame: &mut Frame, model: &Model, area: Rect, t: Theme, hits: &mut HitMap) {
    let block = panel(" SETTINGS ", true, t);
    let inner = block.inner(area).inner(Margin::new(2, 1));
    frame.render_widget(block, area);
    let mut x = inner.x;
    for category in SettingsCategory::ALL {
        let label = format!(" {} ", t.language.tr(category.label()));
        let width = cell_width(&label) as u16;
        let rect = Rect::new(x, inner.y, width, 1);
        let style = if category == model.settings_category {
            t.selected().fg(t.accent).add_modifier(Modifier::BOLD)
        } else {
            t.muted()
        };
        frame.render_widget(Paragraph::new(label).style(style), rect);
        hits.settings_categories.push((rect, category));
        x += width + 1;
    }
    let description = match model.settings_category {
        SettingsCategory::Interface => t.language.tr("Choose the interface language").into_owned(),
        SettingsCategory::Network => t
            .language
            .tr("Proxy and HTTP request settings")
            .into_owned(),
        SettingsCategory::Updates => t
            .language
            .tr("Version checks and release information")
            .into_owned(),
        SettingsCategory::Accounts => model
            .account
            .as_ref()
            .filter(|_| model.signed_in)
            .map(|account| {
                safe(&format!(
                    "{} · {}",
                    account.name.as_deref().unwrap_or(&account.email),
                    account.email
                ))
            })
            .unwrap_or_else(|| t.language.tr("Not signed in").into_owned()),
    };
    frame.render_widget(
        Paragraph::new(description).style(t.muted()),
        Rect::new(inner.x, inner.y + 2, inner.width, 1),
    );
    let items = model.settings_category.items();
    let row_height = if inner.height < 10 { 1 } else { 2 };
    let body = Rect::new(
        inner.x,
        inner.y + 4,
        inner.width,
        inner.height.saturating_sub(4),
    );
    let visible = (body.height / row_height) as usize;
    let start = scroll_start(model.settings_cursor, visible, items.len());
    hits.files = Rect {
        height: items.len().saturating_sub(start).min(visible) as u16 * row_height,
        ..body
    };
    hits.row_height = row_height;
    hits.list_start = start;
    for (index, setting) in items.iter().enumerate().skip(start).take(visible) {
        let (label, value) = match setting {
            Setting::Language => ("Language", model.language().label().to_string()),
            Setting::Proxy => (
                "Proxy",
                t.language
                    .tr(&model.settings.proxy.description())
                    .into_owned(),
            ),
            Setting::UserAgent => (
                "User-Agent",
                model
                    .settings
                    .user_agent
                    .as_deref()
                    .unwrap_or(DEFAULT_USER_AGENT)
                    .to_string(),
            ),
            Setting::AutomaticUpdates => (
                "Automatic update checks",
                t.language
                    .tr(if model.settings.check_updates {
                        "On"
                    } else {
                        "Off"
                    })
                    .into_owned(),
            ),
            Setting::CheckUpdates => (
                "Check for updates",
                match &model.update {
                    UpdateState::NotChecked => t.language.tr("Updates not checked").into_owned(),
                    UpdateState::Checking => t.language.tr("Checking for updates…").into_owned(),
                    UpdateState::Failed(_) => t.language.tr("Check failed").into_owned(),
                    UpdateState::Checked(result) => result
                        .latest_version
                        .as_ref()
                        .map(|version| format!("v{version}"))
                        .unwrap_or_else(|| "—".into()),
                },
            ),
            Setting::SwitchAccount => ("Switch account", current_account_name(model, t)),
            Setting::SignIn => (
                if model.signed_in {
                    "Add account"
                } else {
                    "Sign in with token"
                },
                String::new(),
            ),
            Setting::SignOut => ("Sign out", String::new()),
        };
        let selected = index == model.settings_cursor;
        let style = if selected {
            t.selected()
        } else if *setting == Setting::SignOut && !model.signed_in {
            t.muted()
        } else {
            Style::default().fg(t.text)
        };
        frame.render_widget(
            Paragraph::new(format!(
                "{} {}    {}",
                if selected { "›" } else { " " },
                t.language.tr(label),
                value
            ))
            .style(style),
            Rect::new(
                body.x,
                body.y + (index - start) as u16 * row_height,
                body.width,
                1,
            ),
        );
    }
}

fn empty(frame: &mut Frame, area: Rect, symbol: &str, title: &str, description: &str, t: Theme) {
    let y = area.y + (area.height / 3).saturating_sub(1);
    let body = Rect::new(
        area.x + 1,
        y,
        area.width.saturating_sub(2),
        area.bottom().saturating_sub(y),
    );
    let mut lines = vec![
        Line::styled(symbol.to_string(), t.accent()),
        Line::raw(""),
        Line::styled(
            t.language.tr(title).into_owned(),
            Style::default().fg(t.text).add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
    ];
    lines.extend(
        description
            .lines()
            .map(|line| Line::styled(t.language.tr(line).into_owned(), t.muted())),
    );
    frame.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false }),
        body,
    );
}

fn draw_overlay(
    frame: &mut Frame,
    model: &Model,
    overlay: &Overlay,
    t: Theme,
    hits: &mut DialogHits,
) {
    match overlay {
        Overlay::Accounts { selected } => {
            let area = popup(
                frame,
                76,
                (model.settings.accounts.len() as u16 * 2 + 8).min(26),
                " SAVED ACCOUNTS ",
                t,
                false,
            );
            let inner = area.inner(Margin::new(2, 1));
            let visible = inner.height.saturating_sub(3) as usize / 2;
            let start = scroll_start(*selected, visible, model.settings.accounts.len() + 1);
            for (row, index) in (start..model.settings.accounts.len() + 1)
                .take(visible)
                .enumerate()
            {
                let (name, detail) = model
                    .settings
                    .accounts
                    .get(index)
                    .map(|account| {
                        (
                            if model.settings.active_account.as_ref() == Some(&account.key)
                                && model.account.is_some()
                                && model.signed_in
                            {
                                current_account_name(model, t)
                            } else if account.key.starts_with('@') {
                                t.language.tr(&account.label).into_owned()
                            } else {
                                safe(&account.label)
                            },
                            format!(
                                "{}{}",
                                if model.settings.active_account.as_ref() == Some(&account.key) {
                                    "● "
                                } else {
                                    ""
                                },
                                account.detail
                            ),
                        )
                    })
                    .unwrap_or_else(|| {
                        (format!("+ {}", t.language.tr("Add account")), String::new())
                    });
                let y = inner.y + row as u16 * 2;
                frame.render_widget(
                    Paragraph::new(format!(
                        "{} {name}",
                        if index == *selected { "›" } else { " " }
                    ))
                    .style(if index == *selected {
                        t.selected()
                    } else {
                        Style::default().fg(t.text)
                    }),
                    Rect::new(inner.x, y, inner.width, 1),
                );
                frame.render_widget(
                    Paragraph::new(safe(&detail)).style(t.muted()),
                    Rect::new(inner.x + 2, y + 1, inner.width.saturating_sub(2), 1),
                );
            }
            frame.render_widget(
                Paragraph::new(t.language.tr("↑↓ choose   enter switch   esc close"))
                    .style(t.muted()),
                Rect::new(inner.x, area.bottom().saturating_sub(2), inner.width, 1),
            );
        }
        Overlay::Palette { input, selected } => {
            let matches = palette_matches_in(&input.text, model.language());
            let rows = matches.len().min(8);
            let area = popup(frame, 84, rows as u16 + 7, " COMMANDS ", t, true);
            let inner = area.inner(Margin::new(2, 1));
            frame.render_widget(
                Paragraph::new(t.language.tr("Find an action")).style(t.muted()),
                Rect { height: 1, ..inner },
            );
            frame.render_widget(
                Paragraph::new("❯ ").style(t.accent()),
                Rect::new(inner.x, inner.y + 1, 2, 1),
            );
            editor(
                frame,
                input,
                Rect::new(inner.x + 2, inner.y + 1, inner.width.saturating_sub(2), 1),
                false,
                t,
            );
            let menu = Rect::new(inner.x, inner.y + 3, inner.width, rows as u16);
            let start = scroll_start(*selected, rows, matches.len());
            let data = matches.iter().skip(start).take(rows).map(|&i| {
                let cmd = &COMMANDS[i];
                Row::new([
                    Cell::from(t.language.tr(cmd.name)).style(t.accent()),
                    Cell::from(t.language.tr(cmd.description)).style(t.muted()),
                    Cell::from(cmd.key).style(t.muted()),
                ])
            });
            let table = Table::new(
                data,
                [
                    Constraint::Length(13),
                    Constraint::Min(10),
                    Constraint::Length(6),
                ],
            )
            .column_spacing(1)
            .row_highlight_style(t.selected())
            .highlight_symbol("› ");
            frame.render_stateful_widget(
                table,
                menu,
                &mut TableState::default().with_selected(Some(selected.saturating_sub(start))),
            );
            if matches.is_empty() {
                frame.render_widget(
                    Paragraph::new(t.language.tr("No matching commands")).style(t.muted()),
                    menu,
                );
            }
            frame.render_widget(
                Paragraph::new(
                    t.language
                        .tr("↑↓ choose   enter run   tab complete   esc close"),
                )
                .style(t.muted()),
                Rect::new(inner.x, area.bottom().saturating_sub(2), inner.width, 1),
            );
        }
        Overlay::Form(form) => draw_form(frame, form, t, hits),
        Overlay::Picker(picker) => draw_picker(frame, model, picker, t, hits),
        Overlay::Busy { form, .. } => {
            let area = popup(frame, 64, 8, " WORKING ", t, false);
            frame.render_widget(
                Paragraph::new(vec![
                    Line::styled(
                        format!("{} {}", spinner(model.tick), t.language.tr(form.title)),
                        t.accent(),
                    ),
                    Line::raw(""),
                    Line::styled(t.language.tr("Waiting for the cloud drive…"), t.muted()),
                ])
                .wrap(Wrap { trim: false }),
                area.inner(Margin::new(3, 2)),
            );
        }
        Overlay::ShareReady(share) => {
            let area = popup(frame, 80, 14, " SHARE LINK READY ", t, false);
            let lines = vec![
                Line::styled(
                    t.language.tr("↗  Ready to share"),
                    Style::default().fg(t.mint).add_modifier(Modifier::BOLD),
                ),
                Line::raw(""),
                Line::styled(safe(&share.remote_path), t.muted()),
                Line::raw(""),
                Line::styled(safe(&share.url), t.accent()),
                Line::raw(""),
                Line::raw(format!(
                    "{}  ·  {}",
                    t.language.tr(if share.password_protected {
                        "Password protected"
                    } else {
                        "No password"
                    }),
                    share
                        .expires_in_days
                        .map(|d| t.language.format("Expires in {0} days", &[d.to_string()]))
                        .unwrap_or_else(|| t.language.tr("No expiry").into_owned())
                )),
                Line::raw(""),
                hint("y / enter", "Copy link to clipboard", t),
                hint("esc", "Back to files", t),
            ];
            frame.render_widget(
                Paragraph::new(lines)
                    .style(Style::default().fg(t.text))
                    .wrap(Wrap { trim: false }),
                area.inner(Margin::new(3, 1)),
            );
        }
        Overlay::Revoke { path, .. } => {
            let area = popup(frame, 70, 11, " REVOKE SHARE LINK ", t, false);
            frame.render_widget(
                Paragraph::new(vec![
                    Line::styled(t.language.tr("Revoke this link?"),
                        Style::default().fg(t.red).add_modifier(Modifier::BOLD),
                    ),
                    Line::raw(""),
                    Line::raw(safe(path)),
                    Line::raw(""),
                    Line::styled(t.language.tr("People using this link will lose access. Your file stays in the drive."),
                        t.muted(),
                    ),
                    Line::raw(""),
                    hint("enter", "Revoke link", t),
                    hint("esc", "Keep link", t),
                ])
                .wrap(Wrap { trim: false }),
                area.inner(Margin::new(3, 1)),
            );
        }
        Overlay::Quit => {
            let area = popup(frame, 66, 10, " TRANSFERS ARE STILL RUNNING ", t, false);
            frame.render_widget(
                Paragraph::new(vec![
                    Line::styled(
                        t.language.format(
                            "{0} active transfer(s)",
                            &[model.active_transfers().to_string()],
                        ),
                        t.accent(),
                    ),
                    Line::raw(""),
                    Line::raw(t.language.tr("Quit and cancel the remaining transfers?")),
                    Line::styled(
                        t.language.tr("Partial downloads can resume when retried."),
                        t.muted(),
                    ),
                    Line::raw(""),
                    hint("enter", "Cancel transfers and quit", t),
                    hint("esc", "Keep working", t),
                ])
                .wrap(Wrap { trim: false }),
                area.inner(Margin::new(3, 1)),
            );
        }
        Overlay::Details { title, lines } => {
            let area = popup(frame, 84, 16, " DETAILS ", t, false);
            let mut text = vec![Line::styled(safe(title), t.accent()), Line::raw("")];
            text.extend(
                lines
                    .iter()
                    .map(|s| Line::raw(t.language.message(&safe(s)))),
            );
            text.push(Line::raw(""));
            text.push(Line::styled(t.language.tr("esc / enter  Close"), t.muted()));
            frame.render_widget(
                Paragraph::new(text)
                    .style(Style::default().fg(t.text))
                    .wrap(Wrap { trim: false }),
                area.inner(Margin::new(3, 1)),
            );
        }
        Overlay::Help { scroll } => {
            let area = popup(frame, 88, 24, " MAKE YOURSELF AT HOME ", t, false);
            let lines = vec![
                Line::styled(t.language.tr("Your cloud drive. One keyboard."), t.accent()),
                Line::raw(""),
                hint(
                    "1/2/3/4/5",
                    "Files / Transfers / Shares / Settings / About",
                    t,
                ),
                hint("Ctrl/⌘ P", "Search commands; Tab completes", t),
                hint("↑↓ · j k", "Move selection; PgUp/PgDn page", t),
                hint("enter · l", "Open library, folder, or details", t),
                hint("← · h · ⌫", "Parent folder; at root: libraries", t),
                hint("tab", "Switch between libraries and files", t),
                hint("Ctrl+F / f", "Search the entire library or share links", t),
                hint("space · a", "Select file / toggle all visible files", t),
                hint("u / d / s", "Upload / download / create share link", t),
                hint("n / N / g", "New folder / new library / go to path", t),
                hint("o / i / y", "Sort / toggle inspector / copy path", t),
                hint("r", "Refresh; in Transfers: retry", t),
                hint("x", "Cancel transfer / confirm share revoke", t),
                hint("Shift+C", "Clear completed transfers", t),
                hint("ctrl O", "Cycle conflict policy in transfer forms", t),
                hint("esc / q", "Close overlay / quit workspace", t),
                Line::raw(""),
                Line::styled(
                    t.language
                        .tr("Default conflict policy: uniquify. Folder uploads are not supported."),
                    t.muted(),
                ),
                Line::styled(
                    t.language
                        .tr("Mouse: click to select, double-click to open, wheel to scroll."),
                    t.muted(),
                ),
            ];
            let inner = area.inner(Margin::new(2, 1));
            frame.render_widget(
                Paragraph::new(lines)
                    .style(Style::default().fg(t.text))
                    .wrap(Wrap { trim: false })
                    .scroll((*scroll, 0)),
                Rect {
                    height: inner.height.saturating_sub(1),
                    ..inner
                },
            );
            frame.render_widget(
                Paragraph::new(t.language.tr("↑↓ scroll   esc close")).style(t.muted()),
                Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
            );
        }
    }
}

fn draw_form(frame: &mut Frame, form: &Form, t: Theme, hits: &mut DialogHits) {
    let height = 9 + form.fields.len() as u16 * 3 + if form.is_transfer() { 2 } else { 0 };
    let area = popup(
        frame,
        80,
        height,
        &format!(" {} ", form.title.to_uppercase()),
        t,
        false,
    );
    let inner = area.inner(Margin::new(3, 1));
    let subtitle = match &form.kind {
        FormKind::UploadMany(files) => t.language.format(
            "{0} file(s) will be added to your transfer queue.",
            &[files.len().to_string()],
        ),
        FormKind::Login => t
            .language
            .tr("Paste a cloud drive API token. It will be saved privately on this device.")
            .into_owned(),
        FormKind::UserAgent => t
            .language
            .format("Default: {0}", &[DEFAULT_USER_AGENT.into()]),
        FormKind::Proxy => t
            .language
            .tr("Auto uses environment/system proxies. Manual accepts a proxy URL.")
            .into_owned(),
        FormKind::Share(remote) => safe(remote),
        FormKind::Upload => "Choose the destination and conflict policy before uploading.".into(),
        FormKind::Download(remotes) => t.language.format(
            "{0} item(s) will be added to your transfer queue.",
            &[remotes.len().to_string()],
        ),
        FormKind::Jump => "Library name or repo:<library>/<directory>".into(),
        FormKind::Mkdir => "Creates a folder inside the current directory.".into(),
        FormKind::NewLibrary => "A new home for a collection of files.".into(),
    };
    frame.render_widget(
        Paragraph::new(t.language.tr(&subtitle).into_owned()).style(t.muted()),
        Rect { height: 1, ..inner },
    );
    for (index, field) in form.fields.iter().enumerate() {
        let y = inner.y + 2 + index as u16 * 3;
        if y + 2 >= area.bottom().saturating_sub(3) {
            break;
        }
        let active = form.active == index;
        frame.render_widget(
            Paragraph::new(t.language.tr(field.label)).style(if active {
                t.accent()
            } else {
                t.muted()
            }),
            Rect::new(inner.x, y, inner.width, 1),
        );
        let input = Rect::new(inner.x, y + 1, inner.width, 1);
        frame.render_widget(
            Block::default().style(Style::default().bg(if active { t.selected } else { t.bg })),
            input,
        );
        let input = if form.can_browse(index) {
            let width = button_width("Browse…", t);
            let browse = Rect::new(input.right().saturating_sub(width), input.y, width, 1);
            button(frame, "Browse…", browse, t, active);
            hits.browse.push((browse, index));
            Rect {
                width: input.width.saturating_sub(width + 1),
                ..input
            }
        } else {
            input
        };
        let field_area = Rect::new(input.x + 1, input.y, input.width.saturating_sub(2), 1);
        hits.fields.push((field_area, index));
        if matches!(form.kind, FormKind::Proxy) && index == 0 {
            frame.render_widget(
                Paragraph::new(format!("← {} →", t.language.tr(&field.editor.text)))
                    .style(t.accent()),
                field_area,
            );
            continue;
        }
        let secret = if matches!(form.kind, FormKind::Proxy) {
            field.editor.text.contains('@')
        } else {
            field.secret
        };
        if active {
            editor(frame, &field.editor, field_area, secret, t);
        } else {
            let (text, _) = field.editor.viewport(field_area.width as usize, secret);
            frame.render_widget(
                Paragraph::new(text).style(Style::default().fg(t.text)),
                field_area,
            );
        }
    }
    let bottom = area.bottom().saturating_sub(4);
    if matches!(form.kind, FormKind::Login) {
        frame.render_widget(
            Paragraph::new(format!(
                "{}: https://cloud.tsinghua.edu.cn/profile/#get-auth-token",
                t.language.tr("Get token")
            ))
            .style(t.muted())
            .wrap(Wrap { trim: false }),
            Rect::new(inner.x, inner.y + 8, inner.width, 2),
        );
    }
    if form.is_transfer() {
        let y = inner.y + 2 + form.fields.len() as u16 * 3;
        if y < bottom {
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(
                        format!(
                            "{}: {}",
                            t.language.tr("Conflict"),
                            t.language.tr(form.policy())
                        ),
                        t.accent(),
                    ),
                    Span::styled(
                        format!("  ·  {}", t.language.tr("Ctrl O changes")),
                        t.muted(),
                    ),
                ])),
                Rect::new(inner.x, y, inner.width, 1),
            );
        }
    }
    if let Some(error) = &form.error {
        frame.render_widget(
            Paragraph::new(t.language.message(&safe(error)))
                .style(Style::default().fg(t.red))
                .wrap(Wrap { trim: false }),
            Rect::new(inner.x, bottom, inner.width, 2),
        );
    }
    frame.render_widget(
        Paragraph::new(t.language.tr(if form.can_browse(form.active) {
            "tab complete   ↑↓ fields   Ctrl+B browse   enter confirm   esc cancel"
        } else {
            "tab next field   enter confirm   esc cancel"
        }))
        .style(t.muted()),
        Rect::new(inner.x, area.bottom().saturating_sub(2), inner.width, 1),
    );
}

fn draw_picker(frame: &mut Frame, model: &Model, picker: &Picker, t: Theme, hits: &mut DialogHits) {
    let directory_mode = picker.directories_only();
    let area = popup(
        frame,
        96,
        27,
        if directory_mode {
            " DOWNLOAD · CHOOSE A DIRECTORY "
        } else {
            " UPLOAD · CHOOSE A LOCAL FILE "
        },
        t,
        false,
    );
    let inner = area.inner(Margin::new(2, 1));
    frame.render_widget(
        Paragraph::new(t.language.tr("Path")).style(t.muted()),
        Rect {
            height: 1,
            width: inner.width.saturating_sub(6),
            ..inner
        },
    );
    let parent = Rect::new(inner.right().saturating_sub(5), inner.y, 5, 1);
    button(frame, "↑", parent, t, false);
    let browse_width = button_width("Browse…", t);
    let browse = Rect::new(
        inner.right().saturating_sub(browse_width),
        inner.y + 1,
        browse_width,
        1,
    );
    let input_area = Rect::new(
        inner.x + 2,
        inner.y + 1,
        inner.width.saturating_sub(browse_width + 4),
        1,
    );
    frame.render_widget(
        Block::default().style(t.selected()),
        Rect::new(inner.x, input_area.y, inner.width, 1),
    );
    button(frame, "Browse…", browse, t, true);
    frame.render_widget(
        Paragraph::new("› ").style(t.accent()),
        Rect::new(inner.x, input_area.y, 2, 1),
    );
    if picker.input.text.is_empty() {
        frame.render_widget(
            Paragraph::new(t.language.tr(if directory_mode {
                "Type a directory path. Tab completes; Enter opens or selects."
            } else {
                "Type a path, file name, or pattern. Tab completes; Enter opens or selects."
            }))
            .style(t.muted()),
            input_area,
        );
    } else {
        editor(frame, &picker.input, input_area, false, t);
    }
    let count = if directory_mode {
        t.language.format(
            "{0} directories",
            &[picker
                .entries
                .iter()
                .filter(|entry| !picker.is_current_directory(entry))
                .count()
                .to_string()],
        )
    } else if picker.chosen.is_empty() {
        t.language
            .format("{0} items", &[picker.entries.len().to_string()])
    } else {
        t.language
            .format("{0} files selected", &[picker.chosen.len().to_string()])
    };
    frame.render_widget(
        Paragraph::new(format!(
            "{} · {}{}",
            t.language.tr("Matching paths"),
            count,
            if picker.truncated {
                t.language.tr(" · first 2000 matches").into_owned()
            } else {
                String::new()
            }
        ))
        .style(t.muted()),
        Rect::new(inner.x, inner.y + 3, inner.width, 1),
    );
    let body = Rect::new(
        inner.x,
        inner.y + 5,
        inner.width,
        inner.height.saturating_sub(9),
    );
    if picker.loading {
        frame.render_widget(
            Paragraph::new(format!(
                "{} {}",
                spinner(model.tick),
                t.language.tr("Loading local files")
            ))
            .style(t.muted()),
            body,
        );
    } else if let Some(error) = &picker.error {
        frame.render_widget(
            Paragraph::new(t.language.message(&safe(error)))
                .style(Style::default().fg(t.red))
                .wrap(Wrap { trim: false }),
            body,
        );
    } else {
        let start = scroll_start(picker.selected, body.height as usize, picker.entries.len());
        for (index, entry) in picker
            .entries
            .iter()
            .enumerate()
            .skip(start)
            .take(body.height as usize)
        {
            let size = entry.size.map(format_size).unwrap_or_default();
            let columns = Layout::horizontal([Constraint::Min(10), Constraint::Length(10)]).split(
                Rect::new(body.x, body.y + (index - start) as u16, body.width, 1),
            );
            let selected = index == picker.selected;
            let style = if selected {
                t.selected()
            } else {
                Style::default()
            };
            let display = if picker.is_current_directory(entry) {
                t.language.tr("Use this directory").into_owned()
            } else {
                entry
                    .path
                    .strip_prefix(&picker.match_root)
                    .unwrap_or(&entry.path)
                    .display()
                    .to_string()
            };
            let marker = if picker.is_current_directory(entry) {
                "✓"
            } else if picker.chosen.contains(&entry.path) {
                "●"
            } else if entry.is_dir {
                "▸"
            } else {
                "·"
            };
            frame.render_widget(
                Paragraph::new(format!(
                    "{} {marker} {}",
                    if selected { "›" } else { " " },
                    safe(&display)
                ))
                .style(style.fg(if entry.is_dir { t.accent } else { t.text })),
                columns[0],
            );
            frame.render_widget(Paragraph::new(size).style(style.fg(t.muted)), columns[1]);
        }
        if picker.entries.is_empty() {
            frame.render_widget(
                Paragraph::new(t.language.tr("No matching paths")).style(t.muted()),
                body,
            );
        }
    }
    let select_label = if directory_mode {
        "Use directory"
    } else {
        "Select"
    };
    let cancel_width = button_width("Cancel", t);
    let select_width = button_width(select_label, t);
    let cancel = Rect::new(
        inner.right().saturating_sub(cancel_width),
        area.bottom().saturating_sub(4),
        cancel_width,
        1,
    );
    let select = Rect::new(
        cancel.x.saturating_sub(select_width + 2),
        cancel.y,
        select_width,
        1,
    );
    button(frame, select_label, select, t, true);
    button(frame, "Cancel", cancel, t, false);
    hits.picker = Some(PickerHits {
        input: input_area,
        browse,
        parent,
        list: body,
        start: scroll_start(picker.selected, body.height as usize, picker.entries.len()),
        rows: if picker.loading || picker.error.is_some() {
            0
        } else {
            picker.entries.len().min(body.height as usize)
        },
        select,
        cancel,
    });
    frame.render_widget(
        Paragraph::new(t.language.tr(if directory_mode {
            "↑↓ select  tab complete  enter open/use  esc cancel"
        } else if picker.allows_multiple() {
            "↑↓ select  tab complete  enter choose  ctrl+a all matches  esc cancel"
        } else {
            "↑↓ select  tab complete  enter open/select  esc cancel"
        }))
        .style(t.muted()),
        Rect::new(inner.x, area.bottom().saturating_sub(3), inner.width, 1),
    );
    frame.render_widget(
        Paragraph::new(t.language.tr(if directory_mode {
            "Ctrl+D use directory · Alt+↑ parent · Alt+. hidden · Ctrl+L clear"
        } else if picker.allows_multiple() {
            "Ctrl+L clear path · Alt+↑ parent · Alt+. hidden · Ctrl+Space select"
        } else {
            "Ctrl+B browse · Alt+↑ parent · Alt+. hidden · Ctrl+L clear"
        }))
        .style(t.muted()),
        Rect::new(inner.x, area.bottom().saturating_sub(2), inner.width, 1),
    );
    // The path field remains the input target while arrows move through matches.
    let (_, cursor) = picker.input.viewport(input_area.width as usize, false);
    frame.set_cursor_position((input_area.x + cursor, input_area.y));
}

fn button_width(label: &str, t: Theme) -> u16 {
    cell_width(&t.language.tr(label)) as u16 + 4
}

fn button(frame: &mut Frame, label: &str, area: Rect, t: Theme, primary: bool) {
    frame.render_widget(
        Paragraph::new(format!("[ {} ]", t.language.tr(label)))
            .alignment(Alignment::Center)
            .style(if primary {
                t.selected().fg(t.accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.accent).bg(t.panel)
            }),
        area,
    );
}

fn popup(frame: &mut Frame, width: u16, height: u16, title: &str, t: Theme, bottom: bool) -> Rect {
    let screen = frame.area();
    let width = width.min(screen.width.saturating_sub(4));
    let height = height.min(screen.height.saturating_sub(2));
    let area = Rect::new(
        screen.x + (screen.width - width) / 2,
        if bottom {
            screen.bottom().saturating_sub(height + 4).max(screen.y + 1)
        } else {
            screen.y + (screen.height - height) / 2
        },
        width,
        height,
    );
    let shadow = Rect::new(
        area.x + 1,
        area.y + 1,
        area.width.min(screen.right().saturating_sub(area.x + 1)),
        area.height.min(screen.bottom().saturating_sub(area.y + 1)),
    );
    frame.render_widget(
        Block::default().style(Style::default().bg(Color::Black)),
        shadow,
    );
    frame.render_widget(Clear, area);
    frame.render_widget(panel(title.to_string(), true, t), area);
    area
}

fn editor(frame: &mut Frame, input: &Editor, area: Rect, secret: bool, t: Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let (text, cursor) = input.viewport(area.width as usize, secret);
    frame.render_widget(
        Paragraph::new(text).style(Style::default().fg(t.text)),
        area,
    );
    frame.set_cursor_position((area.x + cursor, area.y));
}

fn hint(key: &str, description: &str, t: Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{key:<12}"), t.accent()),
        Span::styled(
            t.language.tr(description).into_owned(),
            Style::default().fg(t.text),
        ),
    ])
}

fn compact_hint(key: &str, description: &str, t: Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{key:<7}"), t.accent()),
        Span::styled(
            t.language.tr(description).into_owned(),
            Style::default().fg(t.text),
        ),
    ])
}

pub fn spinner(tick: u64) -> &'static str {
    ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"][(tick / 2) as usize % 10]
}

pub fn scroll_start(cursor: usize, height: usize, total: usize) -> usize {
    cursor
        .saturating_sub(height.saturating_sub(1))
        .min(total.saturating_sub(height))
}

/// Filenames and server errors are data, never terminal instructions.
pub fn safe(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).collect()
}

pub fn cell_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}
