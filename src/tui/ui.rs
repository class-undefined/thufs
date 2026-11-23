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

use crate::transfer::format_size;

use super::{
    editor::Editor,
    model::{
        COMMANDS, Focus, Form, FormKind, Model, Overlay, Picker, TransferStatus, Workspace,
        palette_matches,
    },
};

#[derive(Clone, Copy)]
struct Theme {
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
    fn new() -> Self {
        if std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty()) {
            Self {
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
    pub libraries: Rect,
    pub files: Rect,
    pub file_header: u16,
    pub row_height: u16,
    pub list_start: usize,
    pub library_start: usize,
    pub tabs: Vec<(Rect, Workspace)>,
    pub command: Rect,
}

pub fn draw(frame: &mut Frame, model: &Model) -> HitMap {
    let theme = Theme::new();
    let area = frame.area();
    frame.render_widget(Block::default().style(theme.plain()), area);
    if area.width < 60 || area.height < 18 {
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled("thufs", theme.accent()),
                Line::raw(""),
                Line::raw("This workspace needs at least 60 × 18 cells."),
                Line::raw("Resize your terminal, or press q to quit."),
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
        Constraint::Length(if area.height < 26 { 4 } else { 5 }),
    ])
    .split(root);
    let mut hits = HitMap::default();
    header(frame, model, regions[0], theme, &mut hits);
    match model.workspace {
        Workspace::Files => files_workspace(frame, model, regions[1], theme, &mut hits),
        Workspace::Transfers => transfers(frame, model, regions[1], theme, &mut hits),
        Workspace::Shares => shares(frame, model, regions[1], theme, &mut hits),
    }
    footer(frame, model, regions[2], theme, &mut hits);
    if let Some(overlay) = &model.overlay {
        draw_overlay(frame, model, overlay, theme);
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
            Span::styled("  /  cloud workspace", t.muted()),
        ])),
        top[0],
    );
    let (status, color) = if model.demo {
        ("DEMO · offline", t.accent)
    } else if model.connected {
        ("● connected", t.mint)
    } else if model.repos_loading {
        ("◌ connecting", t.muted)
    } else {
        ("○ disconnected", t.red)
    };
    let mut right = vec![Span::styled(status, Style::default().fg(color))];
    if area.width >= 96 {
        right.insert(0, Span::styled("cloud.tsinghua.edu.cn  ", t.muted()));
    }
    frame.render_widget(
        Paragraph::new(Line::from(right)).alignment(Alignment::Right),
        top[1],
    );
    let labels = [
        ("1  Files", Workspace::Files),
        ("2  Transfers", Workspace::Transfers),
        ("3  Shares", Workspace::Shares),
    ];
    let mut x = area.x + 1;
    for (label, workspace) in labels {
        let width = label.len() as u16 + 4;
        let rect = Rect::new(x, area.y + 3, width, 1);
        let style = if model.workspace == workspace {
            t.selected().fg(t.accent).add_modifier(Modifier::BOLD)
        } else {
            t.muted()
        };
        frame.render_widget(Paragraph::new(format!("  {label}  ")).style(style), rect);
        hits.tabs.push((rect, workspace));
        x += width + 1;
    }
    let active = model.active_transfers();
    if active > 0 && area.width > 95 {
        frame.render_widget(
            Paragraph::new(format!("{} {active} active  ", spinner(model.tick)))
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
            "LIBRARIES",
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
            Paragraph::new(format!("{} Loading libraries", spinner(model.tick))).style(t.muted()),
            list,
        );
    } else if model.repos.is_empty() {
        frame.render_widget(Paragraph::new("No libraries yet").style(t.muted()), list);
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
            Span::styled("  New library", t.muted()),
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
                Paragraph::new("Your files, in the terminal.")
                    .style(t.muted())
                    .wrap(Wrap { trim: false }),
                Rect::new(bottom.x, bottom.y + 5, bottom.width, 2),
            );
        }
    }
}

fn panel<'a>(title: impl Into<Line<'a>>, active: bool, t: Theme) -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if active { t.accent } else { t.border }))
        .style(Style::default().bg(t.panel).fg(t.text))
        .title(title)
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
    let count = if model.filter.text.is_empty() {
        format!("{} items", model.entries.len())
    } else {
        format!(
            "{} of {} items · filter: {}",
            filtered.len(),
            model.entries.len(),
            safe(&model.filter.text)
        )
    };
    if !minimal {
        frame.render_widget(
            Paragraph::new(format!("{count}  ·  {}", model.sort.label())).style(t.muted()),
            Rect::new(
                inner.x,
                inner.y + if compact { 1 } else { 2 },
                inner.width,
                1,
            ),
        );
    }
    let top = if minimal {
        2
    } else if compact {
        3
    } else {
        4
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
    if model.loading {
        empty(
            frame,
            body,
            spinner(model.tick),
            "Loading your files",
            "You can keep using commands while this loads.",
            t,
        );
        return;
    }
    if let Some(error) = &model.load_error {
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
            "Your cloud workspace",
            "Tab to choose a library, or N to create one.\nPress / to discover commands.",
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
        let file = &model.entries[index];
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
                safe(&file.name),
                if dir { "/" } else { "" }
            ))
            .style(Style::default().fg(if dir { t.accent } else { t.text })),
            Cell::from(file.size.map(format_size).unwrap_or_else(|| "—".into())).style(t.muted()),
        ])
    });
    let header = Row::new(["", "NAME", "SIZE"])
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
        "Space select files · a select all".into()
    } else {
        format!("{} selected · d download · Esc clear", model.selected.len())
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
        Paragraph::new("INSPECTOR").style(t.muted()),
        Rect { height: 1, ..area },
    );
    let Some(file) = model.current_file() else {
        frame.render_widget(
            Paragraph::new("Select a file to see its details.")
                .style(t.muted())
                .wrap(Wrap { trim: true }),
            Rect::new(area.x, area.y + 3, area.width, 3),
        );
        return;
    };
    let dir = file.kind == "dir";
    let kind = if dir {
        "FOLDER".into()
    } else {
        file.name
            .rsplit_once('.')
            .map(|(_, ext)| format!("{} FILE", ext.to_uppercase()))
            .unwrap_or_else(|| "FILE".into())
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
        Line::styled("SIZE", t.muted()),
        Line::raw(file.size.map(format_size).unwrap_or_else(|| "—".into())),
        Line::raw(""),
        Line::styled("LOCATION", t.muted()),
        Line::raw(
            model
                .location
                .as_ref()
                .map(|l| safe(&l.repo))
                .unwrap_or_default(),
        ),
        Line::styled(safe(&file.path), t.muted()),
        Line::raw(""),
        Line::styled("ACTIONS", t.muted()),
    ];
    lines.push(compact_hint(
        if dir { "enter" } else { "d" },
        if dir { "Open folder" } else { "Download" },
        t,
    ));
    lines.push(compact_hint("s", "Create share link", t));
    lines.push(compact_hint("y", "Copy remote path", t));
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
            Span::styled(format!("{active} active"), t.accent()),
            Span::styled(
                format!(
                    "  ·  {complete} completed  ·  {} total",
                    model.transfers.len()
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
                Span::styled(format!("  {}", transfer.spec.direction()), t.muted()),
            ])),
            columns[0],
        );
        frame.render_widget(
            Paragraph::new(format!(
                "{}  {:>3.0}% ",
                transfer.status.label(),
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
    frame.render_widget(
        Paragraph::new(
            "x cancel  ·  r retry failed/cancelled  ·  enter details  ·  C clear finished",
        )
        .style(t.muted()),
        Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
    );
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
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Shared from your drive", t.accent()),
            Span::styled(format!("  ·  {} links", model.shares.len()), t.muted()),
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
    let indices = model.share_indices();
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
            Cell::from(safe(share.expire_date.as_deref().unwrap_or("No expiry"))).style(t.muted()),
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
            Row::new(["FILE / LINK", "LIBRARY", "VIEWS", "EXPIRES"])
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
    frame.render_widget(
        Paragraph::new("y copy link  ·  enter details  ·  x revoke  ·  f filter  ·  r refresh")
            .style(t.muted()),
        Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
    );
}

fn footer(frame: &mut Frame, model: &Model, area: Rect, t: Theme, hits: &mut HitMap) {
    let compact = area.height < 5;
    let notice = model
        .notice
        .as_ref()
        .filter(|n| n.since.elapsed().as_secs() < 9);
    let (text, color) = if let Some(notice) = notice {
        (
            format!(
                " {} {}",
                if notice.error { "!" } else { "✓" },
                safe(&notice.text)
            ),
            if notice.error { t.red } else { t.mint },
        )
    } else if model.quitting {
        (" Cancelling active transfers before exit…".into(), t.muted)
    } else if model.active_transfers() > 0 {
        (
            format!(
                " {} {} transfer(s) active · press 2 to view progress",
                spinner(model.tick),
                model.active_transfers()
            ),
            t.mint,
        )
    } else if model.demo {
        (
            " Offline demo · explore freely · cloud operations are disabled".into(),
            t.muted,
        )
    } else {
        (
            format!(" Local  {}", safe(&model.local.display().to_string())),
            t.muted,
        )
    };
    if !compact {
        frame.render_widget(
            Paragraph::new(text.clone()).style(Style::default().fg(color)),
            Rect { height: 1, ..area },
        );
    }
    let command = Rect::new(area.x, area.y + u16::from(!compact), area.width, 3);
    hits.command = command;
    let block = panel("", model.filtering, t);
    let inner = block.inner(command).inner(Margin::new(1, 0));
    frame.render_widget(block, command);
    if model.filtering {
        let prefix = "find › ";
        frame.render_widget(Paragraph::new(prefix).style(t.accent()), inner);
        let prefix_width = cell_width(prefix) as u16;
        let input_area = Rect::new(
            inner.x + prefix_width,
            inner.y,
            inner.width.saturating_sub(prefix_width),
            1,
        );
        editor(frame, &model.filter, input_area, false, t);
    } else {
        let mut spans = vec![
            Span::styled("❯  ", t.accent()),
            Span::styled("/", Style::default().fg(t.text)),
            Span::styled(" for commands", t.muted()),
        ];
        if !model.filter.text.is_empty() {
            spans.push(Span::styled(
                format!("    filter: {}  ·  Esc clears", safe(&model.filter.text)),
                Style::default().fg(t.mint),
            ));
        }
        let columns = Layout::horizontal([
            Constraint::Min(20),
            Constraint::Length(if inner.width > 82 { 24 } else { 0 }),
        ])
        .split(inner);
        frame.render_widget(Paragraph::new(Line::from(spans)), columns[0]);
        if inner.width > 82 {
            frame.render_widget(
                Paragraph::new("Ctrl P  command palette")
                    .style(t.muted())
                    .alignment(Alignment::Right),
                columns[1],
            );
        }
    }
    let hints = match model.workspace {
        Workspace::Files if model.focus == Focus::Libraries => {
            "↑↓ navigate   enter open library   tab files   N new library   ? help   q quit"
        }
        Workspace::Files => {
            if area.width >= 100 {
                "↑↓ navigate   enter open   tab libraries   u upload   d download   s share   f find   ? help   q quit"
            } else {
                "↑↓ move  enter open  tab libraries  u upload  d download  ? help"
            }
        }
        Workspace::Transfers => {
            "↑↓ navigate   x cancel   r retry   enter details   1 files   ? help   q quit"
        }
        Workspace::Shares => {
            "↑↓ navigate   y copy   x revoke   f find   r refresh   1 files   ? help   q quit"
        }
    };
    frame.render_widget(
        Paragraph::new(if compact && notice.is_some() {
            text
        } else {
            format!(" {hints}")
        })
        .style(if compact && notice.is_some() {
            Style::default().fg(color)
        } else {
            t.muted()
        }),
        Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1),
    );
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
            title.to_string(),
            Style::default().fg(t.text).add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
    ];
    lines.extend(
        description
            .lines()
            .map(|line| Line::styled(line.to_string(), t.muted())),
    );
    frame.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false }),
        body,
    );
}

fn draw_overlay(frame: &mut Frame, model: &Model, overlay: &Overlay, t: Theme) {
    match overlay {
        Overlay::Palette { input, selected } => {
            let matches = palette_matches(&input.text);
            let rows = matches.len().min(8);
            let area = popup(frame, 84, rows as u16 + 7, " COMMANDS ", t, true);
            let inner = area.inner(Margin::new(2, 1));
            frame.render_widget(
                Paragraph::new("Find an action").style(t.muted()),
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
                    Cell::from(cmd.name).style(t.accent()),
                    Cell::from(cmd.description).style(t.muted()),
                    Cell::from(cmd.key).style(t.muted()),
                ])
            });
            let table = Table::new(
                data,
                [
                    Constraint::Length(13),
                    Constraint::Min(10),
                    Constraint::Length(5),
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
                    Paragraph::new("No matching commands").style(t.muted()),
                    menu,
                );
            }
            frame.render_widget(
                Paragraph::new("↑↓ choose   enter run   tab complete   esc close").style(t.muted()),
                Rect::new(inner.x, area.bottom().saturating_sub(2), inner.width, 1),
            );
        }
        Overlay::Form(form) => draw_form(frame, form, t),
        Overlay::Picker(picker) => draw_picker(frame, model, picker, t),
        Overlay::Busy { form, .. } => {
            let area = popup(frame, 64, 8, " WORKING ", t, false);
            frame.render_widget(
                Paragraph::new(vec![
                    Line::styled(
                        format!("{} {}", spinner(model.tick), form.title),
                        t.accent(),
                    ),
                    Line::raw(""),
                    Line::styled("Waiting for the cloud drive…", t.muted()),
                ])
                .wrap(Wrap { trim: false }),
                area.inner(Margin::new(3, 2)),
            );
        }
        Overlay::ShareReady(share) => {
            let area = popup(frame, 80, 14, " SHARE LINK READY ", t, false);
            let lines = vec![
                Line::styled(
                    "↗  Ready to share",
                    Style::default().fg(t.mint).add_modifier(Modifier::BOLD),
                ),
                Line::raw(""),
                Line::styled(safe(&share.remote_path), t.muted()),
                Line::raw(""),
                Line::styled(safe(&share.url), t.accent()),
                Line::raw(""),
                Line::raw(format!(
                    "{}  ·  {}",
                    if share.password_protected {
                        "Password protected"
                    } else {
                        "No password"
                    },
                    share
                        .expires_in_days
                        .map(|d| format!("Expires in {d} days"))
                        .unwrap_or_else(|| "No expiry".into())
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
                    Line::styled(
                        "Revoke this link?",
                        Style::default().fg(t.red).add_modifier(Modifier::BOLD),
                    ),
                    Line::raw(""),
                    Line::raw(safe(path)),
                    Line::raw(""),
                    Line::styled(
                        "People using this link will lose access. Your file stays in the drive.",
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
                        format!("{} active transfer(s)", model.active_transfers()),
                        t.accent(),
                    ),
                    Line::raw(""),
                    Line::raw("Quit and cancel the remaining transfers?"),
                    Line::styled("Partial downloads can resume when retried.", t.muted()),
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
            text.extend(lines.iter().map(|s| Line::raw(safe(s))));
            text.push(Line::raw(""));
            text.push(Line::styled("esc / enter  Close", t.muted()));
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
                Line::styled("Your cloud drive. One keyboard.", t.accent()),
                Line::raw(""),
                hint("1 / 2 / 3", "Files / Transfers / Shares", t),
                hint("/ · Ctrl P", "Search commands; Tab completes", t),
                hint("↑↓ · j k", "Move selection; PgUp/PgDn page", t),
                hint("enter · l", "Open library, folder, or details", t),
                hint("← · h · ⌫", "Parent folder; at root: libraries", t),
                hint("tab", "Switch between libraries and files", t),
                hint("f", "Fuzzy filter; Enter keeps, Esc clears", t),
                hint("space · a", "Select file / toggle all visible files", t),
                hint("u / d / s", "Upload / download / create share link", t),
                hint("n / N / g", "New folder / new library / go to path", t),
                hint("o / i / y", "Sort / toggle inspector / copy path", t),
                hint("r", "Refresh; in Transfers: retry", t),
                hint("x", "Cancel transfer / confirm share revoke", t),
                hint("C", "Clear completed transfers", t),
                hint("ctrl O", "Cycle conflict policy in transfer forms", t),
                hint("esc / q", "Close overlay / quit workspace", t),
                Line::raw(""),
                Line::styled(
                    "Default conflict policy: uniquify. Folder transfers are not supported.",
                    t.muted(),
                ),
                Line::styled(
                    "Mouse: click to select, double-click to open, wheel to scroll.",
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
                Paragraph::new("↑↓ scroll   esc close").style(t.muted()),
                Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1),
            );
        }
    }
}

fn draw_form(frame: &mut Frame, form: &Form, t: Theme) {
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
        FormKind::Share(remote) => safe(remote),
        FormKind::Upload => "Choose the destination and conflict policy before uploading.".into(),
        FormKind::Download(remotes) => format!(
            "{} file(s) will be added to your transfer queue.",
            remotes.len()
        ),
        FormKind::Jump => "Library name or repo:<library>/<directory>".into(),
        FormKind::Mkdir => "Creates a folder inside the current directory.".into(),
        FormKind::NewLibrary => "A new home for a collection of files.".into(),
    };
    frame.render_widget(
        Paragraph::new(subtitle).style(t.muted()),
        Rect { height: 1, ..inner },
    );
    for (index, field) in form.fields.iter().enumerate() {
        let y = inner.y + 2 + index as u16 * 3;
        if y + 2 >= area.bottom().saturating_sub(3) {
            break;
        }
        let active = form.active == index;
        frame.render_widget(
            Paragraph::new(field.label).style(if active { t.accent() } else { t.muted() }),
            Rect::new(inner.x, y, inner.width, 1),
        );
        let input = Rect::new(inner.x, y + 1, inner.width, 1);
        frame.render_widget(
            Block::default().style(Style::default().bg(if active { t.selected } else { t.bg })),
            input,
        );
        let field_area = Rect::new(input.x + 1, input.y, input.width.saturating_sub(2), 1);
        if active {
            editor(frame, &field.editor, field_area, field.secret, t);
        } else {
            let (text, _) = field
                .editor
                .viewport(field_area.width as usize, field.secret);
            frame.render_widget(
                Paragraph::new(text).style(Style::default().fg(t.text)),
                field_area,
            );
        }
    }
    let bottom = area.bottom().saturating_sub(4);
    if form.is_transfer() {
        let y = inner.y + 2 + form.fields.len() as u16 * 3;
        if y < bottom {
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(format!("Conflict: {}", form.policy()), t.accent()),
                    Span::styled("  ·  Ctrl O changes", t.muted()),
                ])),
                Rect::new(inner.x, y, inner.width, 1),
            );
        }
    }
    if let Some(error) = &form.error {
        frame.render_widget(
            Paragraph::new(safe(error))
                .style(Style::default().fg(t.red))
                .wrap(Wrap { trim: false }),
            Rect::new(inner.x, bottom, inner.width, 2),
        );
    }
    frame.render_widget(
        Paragraph::new("tab next field   enter confirm   esc cancel").style(t.muted()),
        Rect::new(inner.x, area.bottom().saturating_sub(2), inner.width, 1),
    );
}

fn draw_picker(frame: &mut Frame, model: &Model, picker: &Picker, t: Theme) {
    let area = popup(frame, 92, 26, " UPLOAD · CHOOSE A LOCAL FILE ", t, false);
    let inner = area.inner(Margin::new(2, 1));
    frame.render_widget(
        Paragraph::new(safe(&picker.cwd.display().to_string())).style(t.accent()),
        Rect { height: 1, ..inner },
    );
    frame.render_widget(
        Paragraph::new("Folders open with Enter. Select a file to review its upload destination.")
            .style(t.muted()),
        Rect::new(inner.x, inner.y + 1, inner.width, 1),
    );
    let body = Rect::new(
        inner.x,
        inner.y + 3,
        inner.width,
        inner.height.saturating_sub(6),
    );
    if picker.loading {
        frame.render_widget(
            Paragraph::new(format!("{} Loading local files", spinner(model.tick))).style(t.muted()),
            body,
        );
    } else if let Some(error) = &picker.error {
        frame.render_widget(
            Paragraph::new(safe(error))
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
            frame.render_widget(
                Paragraph::new(format!(
                    "{} {} {}",
                    if selected { "›" } else { " " },
                    if entry.is_dir { "▸" } else { "·" },
                    safe(&entry.name)
                ))
                .style(style.fg(if entry.is_dir { t.accent } else { t.text })),
                columns[0],
            );
            frame.render_widget(Paragraph::new(size).style(style.fg(t.muted)), columns[1]);
        }
        if picker.entries.is_empty() {
            frame.render_widget(
                Paragraph::new("No files in this folder. Press . to show hidden files.")
                    .style(t.muted()),
                body,
            );
        }
    }
    frame.render_widget(
        Paragraph::new(format!(
            "↑↓ move   enter choose   h parent   . hidden ({})   esc cancel",
            if picker.hidden { "shown" } else { "hidden" }
        ))
        .style(t.muted()),
        Rect::new(inner.x, area.bottom().saturating_sub(2), inner.width, 1),
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
        Span::styled(description.to_string(), Style::default().fg(t.text)),
    ])
}

fn compact_hint(key: &str, description: &str, t: Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{key:<7}"), t.accent()),
        Span::styled(description.to_string(), Style::default().fg(t.text)),
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
