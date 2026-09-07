//! Render functions. Pure — reads `App` state, writes to the frame, never
//! mutates and never blocks (tui-design skill: layout stays fixed, only
//! content changes).

use super::app::{App, Mode};
use super::theme::{Theme, status_color, status_glyph};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Cell, Clear, List, ListItem, Paragraph, Row, Table, TableState,
};
use std::fmt::Display;

const MIN_WIDTH: u16 = 80;
const MIN_HEIGHT: u16 = 24;
/// Below this, the detail column is dropped so the targets list keeps a
/// usable width instead of both columns going illegibly narrow.
const DETAIL_BREAKPOINT: u16 = 100;

/// Every bordered panel goes through here, so rounded corners and the title
/// treatment stay consistent. `border` distinguishes the panel carrying the
/// selection from its supporting panels.
fn panel<'a>(theme: &Theme, title: impl Display, border: Color) -> Block<'a> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border))
        .title(Span::styled(
            format!(" {title} "),
            Style::default()
                .fg(theme.fg_emphasis)
                .add_modifier(Modifier::BOLD),
        ))
}

/// A label/value chip. Reverse-video on the value half binds the pair into
/// one unit at a glance — the reason a chip row scans faster than the same
/// numbers joined into a sentence. Reverse video also survives `NO_COLOR`,
/// so the grouping never depends on colour alone.
fn chip(label: &str, value: impl Display, color: Color) -> [Span<'static>; 2] {
    [
        Span::styled(format!(" {label} "), Style::default().fg(color)),
        Span::styled(
            format!(" {value} "),
            Style::default().fg(color).add_modifier(Modifier::REVERSED),
        ),
    ]
}

pub fn draw(f: &mut Frame, app: &App, theme: &Theme) {
    let area = f.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        let msg = format!("terminal too small — need at least {MIN_WIDTH}x{MIN_HEIGHT}");
        f.render_widget(
            Paragraph::new(msg).style(Style::default().fg(theme.error)),
            area,
        );
        return;
    }

    let [header, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);

    draw_header(f, app, theme, header);
    draw_body(f, app, theme, body);
    draw_footer(f, app, theme, footer);

    match &app.mode {
        Mode::Assign { .. } => draw_assign_modal(f, app, theme, area),
        Mode::Help => draw_help_modal(f, theme, area),
        Mode::TagInput { input } => draw_prompt(f, theme, area, "tag name", input),
        Mode::Search { input } => draw_prompt(f, theme, area, "filter (FIQL q=)", input),
        Mode::ConfirmCancel(id) => {
            draw_confirm(f, theme, area, &format!("cancel action {id}? (y/n)"))
        }
        Mode::ConfirmForce(id) => {
            draw_confirm(f, theme, area, &format!("force action {id}? (y/n)"))
        }
        Mode::Normal => {}
    }
}

fn draw_header(f: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let spinner = if app.loading { "⠋ " } else { "" };
    let mut spans = vec![
        Span::styled(
            " raptor ",
            Style::default()
                .fg(theme.fg_emphasis)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(spinner, Style::default().fg(theme.accent)),
    ];
    if let Some(s) = app.stats.as_ref() {
        let by = &s.targets_by_status;
        let errors = by.get("error").copied().unwrap_or(0);
        spans.extend(chip("targets", s.total_targets, theme.accent));
        spans.extend(chip(
            "in sync",
            by.get("in_sync").copied().unwrap_or(0),
            theme.success,
        ));
        spans.extend(chip(
            "pending",
            by.get("pending").copied().unwrap_or(0),
            theme.warning,
        ));
        // A zero error count shouldn't shout in red alongside the real ones.
        spans.extend(chip(
            "error",
            errors,
            if errors > 0 {
                theme.error
            } else {
                theme.fg_muted
            },
        ));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_body(f: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    if area.width < DETAIL_BREAKPOINT {
        draw_left_column(f, app, theme, area);
        return;
    }
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)]).areas(area);
    draw_left_column(f, app, theme, left);
    draw_detail(f, app, theme, right);
}

fn draw_left_column(f: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let [targets_area, rollouts_area] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(5)]).areas(area);
    draw_targets(f, app, theme, targets_area);
    draw_rollouts(f, app, theme, rollouts_area);
}

fn draw_targets(f: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let rows: Vec<Row> = app
        .targets
        .iter()
        .map(|t| {
            let color = status_color(theme, &t.update_status);
            let ds = t
                .assigned_ds
                .as_ref()
                .map(|d| format!("{}:{}", d.name, d.version))
                .unwrap_or_else(|| "-".into());
            Row::new(vec![
                Cell::from(status_glyph(&t.update_status)).style(Style::default().fg(color)),
                Cell::from(t.controller_id.clone()).style(Style::default().fg(theme.fg)),
                Cell::from(t.update_status.clone()).style(Style::default().fg(color)),
                Cell::from(ds).style(Style::default().fg(theme.fg_muted)),
            ])
        })
        .collect();
    // A `Table` (rather than a `List` of pre-padded strings) is what keeps
    // the columns aligned: it truncates an over-long controller ID to its
    // column instead of letting that row shove the rest to the right.
    // ponytail: truncation is silent — no ellipsis marks a clipped ID. Add a
    // `…` if operators start mistaking a clipped ID for the whole thing; `y`
    // yanks the full value in the meantime.
    let widths = [
        Constraint::Length(1),
        Constraint::Min(14),
        Constraint::Length(11),
        Constraint::Min(10),
    ];
    let table = Table::new(rows, widths)
        .header(
            Row::new(vec!["", "target", "status", "distribution set"]).style(
                Style::default()
                    .fg(theme.fg_muted)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .block(panel(
            theme,
            format!("Targets ({})", app.targets.len()),
            theme.accent,
        ))
        .row_highlight_style(theme.selection.add_modifier(Modifier::BOLD));
    let mut state = TableState::default();
    if !app.targets.is_empty() {
        state.select(Some(app.selected));
    }
    f.render_stateful_widget(table, area, &mut state);
}

fn draw_rollouts(f: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let items: Vec<ListItem> = app
        .rollouts
        .iter()
        .map(|r| {
            let glyph = match r.status.as_str() {
                "running" => "▶",
                "paused" => "‖",
                "finished" => "✓",
                "stopping" | "stopped" => "✕",
                _ => "○",
            };
            let s = &r.total_targets_per_status;
            let done = s.finished + s.error;
            Line::from(format!(
                "{glyph} {} {} — {}/{} · {} err",
                r.name, r.status, done, r.total_targets, s.error
            ))
            .into()
        })
        .collect();
    let list = List::new(items).block(panel(theme, "Rollouts", theme.fg_muted));
    f.render_widget(list, area);
}

fn draw_detail(f: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let Some(t) = app.selected_target() else {
        f.render_widget(panel(theme, "(no targets)", theme.fg_muted), area);
        return;
    };
    let [summary, history_area] =
        Layout::vertical([Constraint::Length(7), Constraint::Min(0)]).areas(area);

    let assigned = t
        .assigned_ds
        .as_ref()
        .map(|d| format!("{}:{}", d.name, d.version))
        .unwrap_or_else(|| "-".into());
    let installed = t
        .installed_ds
        .as_ref()
        .map(|d| format!("{}:{}", d.name, d.version))
        .unwrap_or_else(|| "-".into());
    let tags = if t.tags.is_empty() {
        "-".into()
    } else {
        t.tags
            .iter()
            .map(|tg| tg.name.clone())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let lines = vec![
        Line::from(vec![
            Span::styled("status    ", Style::default().fg(theme.fg_muted)),
            Span::styled(
                &t.update_status,
                Style::default().fg(status_color(theme, &t.update_status)),
            ),
        ]),
        Line::from(format!("installed {installed}")),
        Line::from(format!("assigned  {assigned}")),
        Line::from(format!(
            "address   {}",
            t.address.clone().unwrap_or_else(|| "-".into())
        )),
        Line::from(format!("tags      {tags}")),
    ];
    let block = panel(theme, &t.controller_id, theme.accent);
    f.render_widget(Paragraph::new(lines).block(block), summary);

    let mut hist_lines: Vec<Line> = Vec::new();
    if let Some(a) = app.detail_actions.first() {
        hist_lines.push(Line::from(vec![Span::styled(
            format!("Action {} · {} · {}", a.id, a.force_type, a.detail_status),
            Style::default()
                .fg(theme.fg_emphasis)
                .add_modifier(Modifier::BOLD),
        )]));
    }
    for s in app.detail_history.iter().rev().take(20) {
        hist_lines.push(Line::from(vec![
            Span::styled(
                format!("{:<14}", s.reported_at),
                Style::default().fg(theme.fg_muted),
            ),
            Span::styled(
                format!("{:<12}", s.status_type),
                Style::default().fg(theme.info),
            ),
            Span::raw(s.messages.join("; ")),
        ]));
    }
    if hist_lines.is_empty() {
        hist_lines.push(Line::from(Span::styled(
            "no actions",
            Style::default().fg(theme.fg_muted),
        )));
    }
    let hist_block = panel(theme, "History", theme.fg_muted);
    f.render_widget(Paragraph::new(hist_lines).block(hist_block), history_area);
}

fn draw_footer(f: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let text = if let Some((msg, _)) = &app.status {
        msg.clone()
    } else {
        "[q]uit [/]search [a]ssign [c]ancel [f]orce [t]ag [y]ank [r]efresh [?]help".to_string()
    };
    f.render_widget(
        Paragraph::new(text).style(Style::default().fg(theme.fg_muted)),
        area,
    );
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width, height)
}

fn draw_assign_modal(f: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let Mode::Assign {
        filter,
        items,
        selected,
    } = &app.mode
    else {
        return;
    };
    let rect = centered(area, 60, 16);
    f.render_widget(Clear, rect);
    let filtered: Vec<&raptor_api_types::DsRest> = items
        .iter()
        .filter(|d| filter.is_empty() || d.name.to_lowercase().contains(&filter.to_lowercase()))
        .collect();
    let rows: Vec<ListItem> = filtered
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let style = if i == *selected {
                theme.selection
            } else {
                Style::default()
            };
            ListItem::new(format!("{} {}:{}", d.id, d.name, d.version)).style(style)
        })
        .collect();
    let block = panel(
        theme,
        format!("assign distribution set · filter: {filter}"),
        theme.accent,
    );
    f.render_widget(List::new(rows).block(block), rect);
}

fn draw_help_modal(f: &mut Frame, theme: &Theme, area: Rect) {
    let rect = centered(area, 50, 16);
    f.render_widget(Clear, rect);
    let lines = [
        "↑↓ / j k   move selection",
        "g / G      first / last",
        "/          filter targets (server-side FIQL)",
        "a          assign a distribution set",
        "t          tag the selected target",
        "y          copy controller ID to the clipboard",
        "c          cancel the active action",
        "f          force the active action",
        "r          refresh now",
        "q / Esc    quit",
        "",
        "press any key to close",
    ]
    .join("\n");
    let block = panel(theme, "help", theme.accent);
    f.render_widget(Paragraph::new(lines).block(block), rect);
}

fn draw_prompt(f: &mut Frame, theme: &Theme, area: Rect, title: &str, input: &str) {
    let rect = centered(area, 60, 3);
    f.render_widget(Clear, rect);
    let block = panel(theme, title, theme.accent);
    f.render_widget(Paragraph::new(format!("{input}_")).block(block), rect);
}

fn draw_confirm(f: &mut Frame, theme: &Theme, area: Rect, text: &str) {
    let rect = centered(area, (text.len() as u16 + 4).max(30), 3);
    f.render_widget(Clear, rect);
    // Untitled — the question is the content, so it gets no panel() title.
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.warning));
    f.render_widget(Paragraph::new(text).block(block), rect);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::Client;
    use crate::config::Resolved;
    use raptor_api_types::TargetRest;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn target(controller_id: &str, update_status: &str) -> TargetRest {
        serde_json::from_value(serde_json::json!({
            "controllerId": controller_id,
            "name": controller_id,
            "description": null,
            "updateStatus": update_status,
            "securityToken": "t",
            "createdAt": 0,
            "lastModifiedAt": 0,
            "address": null,
            "ipAddress": null,
            "lastControllerRequestAt": null,
            "pollStatus": null,
        }))
        .unwrap()
    }

    fn row_text(buf: &ratatui::buffer::Buffer, y: u16) -> String {
        (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect()
    }

    /// The reason the targets panel is a `Table` and not a `List` of padded
    /// strings: a controller ID wider than its column used to push that row's
    /// remaining cells right, breaking alignment for that row only.
    #[tokio::test]
    async fn long_controller_id_does_not_shift_the_status_column() {
        let cfg = Resolved {
            url: "http://127.0.0.1:1".into(),
            user: "u".into(),
            pass: "p".into(),
        };
        let (mut app, _rx) = App::new(Client::new(&cfg), 0);
        app.targets = vec![
            target("short", "in_sync"),
            target("a-controller-id-far-wider-than-its-column", "error"),
        ];

        let mut terminal = Terminal::new(TestBackend::new(50, 6)).unwrap();
        let theme = Theme::detect();
        terminal
            .draw(|f| draw_targets(f, &app, &theme, f.area()))
            .unwrap();
        let buf = terminal.backend().buffer();

        // Row 0 is the top border, row 1 the header, rows 2-3 the targets.
        let short_row = row_text(buf, 2);
        let long_row = row_text(buf, 3);
        assert_eq!(
            short_row.find("in_sync"),
            long_row.find("error"),
            "status column moved:\n{short_row}\n{long_row}"
        );
        assert!(
            !long_row.contains("far-wider-than-its-column"),
            "over-long ID should be truncated to its column: {long_row}"
        );
    }
}
