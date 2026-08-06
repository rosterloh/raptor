//! Render functions. Pure — reads `App` state, writes to the frame, never
//! mutates and never blocks (tui-design skill: layout stays fixed, only
//! content changes).

use super::app::{App, Mode};
use super::theme::{Theme, status_color, status_glyph};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};

const MIN_WIDTH: u16 = 80;
const MIN_HEIGHT: u16 = 24;
/// Below this, the detail column is dropped so the targets list keeps a
/// usable width instead of both columns going illegibly narrow.
const DETAIL_BREAKPOINT: u16 = 100;

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
    let stats = app
        .stats
        .as_ref()
        .map(|s| {
            let by = &s.targets_by_status;
            format!(
                "targets {} · in_sync {} · pending {} · error {}",
                s.total_targets,
                by.get("in_sync").copied().unwrap_or(0),
                by.get("pending").copied().unwrap_or(0),
                by.get("error").copied().unwrap_or(0),
            )
        })
        .unwrap_or_default();
    let line = Line::from(vec![
        Span::styled(
            " raptor ",
            Style::default()
                .fg(theme.fg_emphasis)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(spinner, Style::default().fg(theme.accent)),
        Span::raw(stats),
    ]);
    f.render_widget(Paragraph::new(line), area);
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
    let items: Vec<ListItem> = app
        .targets
        .iter()
        .map(|t| {
            let glyph = status_glyph(&t.update_status);
            let color = status_color(theme, &t.update_status);
            let ds = t
                .assigned_ds
                .as_ref()
                .map(|d| format!("{}:{}", d.name, d.version))
                .unwrap_or_else(|| "-".into());
            Line::from(vec![
                Span::styled(format!("{glyph} "), Style::default().fg(color)),
                Span::styled(
                    format!("{:<14}", t.controller_id),
                    Style::default().fg(theme.fg),
                ),
                Span::styled(
                    format!("{:<11}", t.update_status),
                    Style::default().fg(color),
                ),
                Span::styled(ds, Style::default().fg(theme.fg_muted)),
            ])
            .into()
        })
        .collect();
    let title = format!(" Targets ({}) ", app.targets.len());
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(theme.accent)),
        )
        .highlight_style(
            Style::default()
                .bg(theme.selection_bg)
                .add_modifier(Modifier::BOLD),
        );
    let mut state = ratatui::widgets::ListState::default();
    if !app.targets.is_empty() {
        state.select(Some(app.selected));
    }
    f.render_stateful_widget(list, area, &mut state);
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
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Rollouts ")
            .border_style(Style::default().fg(theme.fg_muted)),
    );
    f.render_widget(list, area);
}

fn draw_detail(f: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let Some(t) = app.selected_target() else {
        f.render_widget(
            Block::default()
                .borders(Borders::ALL)
                .title(" (no targets) "),
            area,
        );
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
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} ", t.controller_id))
        .border_style(Style::default().fg(theme.accent));
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
    let hist_block = Block::default()
        .borders(Borders::ALL)
        .title(" History ")
        .border_style(Style::default().fg(theme.fg_muted));
    f.render_widget(Paragraph::new(hist_lines).block(hist_block), history_area);
}

fn draw_footer(f: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let text = if let Some((msg, _)) = &app.status {
        msg.clone()
    } else {
        "[q]uit [/]search [a]ssign [c]ancel [f]orce [t]ag [r]efresh [?]help".to_string()
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
                Style::default().bg(theme.selection_bg)
            } else {
                Style::default()
            };
            ListItem::new(format!("{} {}:{}", d.id, d.name, d.version)).style(style)
        })
        .collect();
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" assign distribution set · filter: {filter} "))
        .border_style(Style::default().fg(theme.accent));
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
        "c          cancel the active action",
        "f          force the active action",
        "r          refresh now",
        "q / Esc    quit",
        "",
        "press any key to close",
    ]
    .join("\n");
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" help ")
        .border_style(Style::default().fg(theme.accent));
    f.render_widget(Paragraph::new(lines).block(block), rect);
}

fn draw_prompt(f: &mut Frame, theme: &Theme, area: Rect, title: &str, input: &str) {
    let rect = centered(area, 60, 3);
    f.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {title} "))
        .border_style(Style::default().fg(theme.accent));
    f.render_widget(Paragraph::new(format!("{input}_")).block(block), rect);
}

fn draw_confirm(f: &mut Frame, theme: &Theme, area: Rect, text: &str) {
    let rect = centered(area, (text.len() as u16 + 4).max(30), 3);
    f.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.warning));
    f.render_widget(Paragraph::new(text).block(block), rect);
}
