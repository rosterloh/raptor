//! Keybindings and the mutations they trigger (assign / cancel / force /
//! tag), including the inline confirmation state machine for the two
//! moderate-severity actions (tui-design skill §3, "Dialogs & Confirmation").

use super::app::{App, Mode, Msg};
use super::osc52;
use crate::api;
use crossterm::event::{KeyCode, KeyEvent};
use raptor_api_types::DsAssignment;

pub fn handle_key(app: &mut App, key: KeyEvent) {
    match &mut app.mode {
        Mode::Normal => handle_normal(app, key),
        Mode::Search { .. } => handle_search(app, key),
        Mode::Assign { .. } => handle_assign(app, key),
        Mode::TagInput { .. } => handle_tag_input(app, key),
        Mode::ConfirmCancel { .. } | Mode::ConfirmForce { .. } => handle_confirm(app, key),
        Mode::Help => app.mode = Mode::Normal,
    }
}

fn handle_normal(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
        KeyCode::Down | KeyCode::Char('j') => app.select_next(),
        KeyCode::Up | KeyCode::Char('k') => app.select_prev(),
        KeyCode::Char('g') => app.select_first(),
        KeyCode::Char('G') => app.select_last(),
        KeyCode::Char('r') => app.refresh_all(),
        KeyCode::Char('?') => app.mode = Mode::Help,
        KeyCode::Char('y') => yank_selected(app),
        KeyCode::Char('/') => {
            app.mode = Mode::Search {
                input: String::new(),
            }
        }
        KeyCode::Char('a') => {
            if let Some(cid) = app.selected_cid() {
                app.set_status("loading distribution sets…");
                app.fetch_ds_list(cid);
            }
        }
        KeyCode::Char('t') => {
            if let Some(cid) = app.selected_cid() {
                app.mode = Mode::TagInput {
                    cid,
                    input: String::new(),
                };
            }
        }
        KeyCode::Char('c') => match pending_action(app) {
            Some((cid, aid)) => app.mode = Mode::ConfirmCancel { cid, aid },
            None => app.set_status("no active action to cancel"),
        },
        KeyCode::Char('f') => match pending_action(app) {
            Some((cid, aid)) => app.mode = Mode::ConfirmForce { cid, aid },
            None => app.set_status("no active action to force"),
        },
        _ => {}
    }
}

fn pending_action(app: &App) -> Option<(String, i64)> {
    let aid = app
        .detail_actions
        .iter()
        .find(|a| a.status == "pending")?
        .id;
    Some((app.selected_cid()?, aid))
}

/// `y` confirms, `n`/`Esc` cancel, anything else is swallowed so a stray key
/// neither fires nor silently dismisses the prompt.
fn handle_confirm(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('y') => match std::mem::replace(&mut app.mode, Mode::Normal) {
            Mode::ConfirmCancel { cid, aid } => spawn_cancel(app, cid, aid),
            Mode::ConfirmForce { cid, aid } => spawn_force(app, cid, aid),
            _ => {}
        },
        KeyCode::Char('n') | KeyCode::Esc => app.mode = Mode::Normal,
        _ => {}
    }
}

/// Yanks the controller ID — the one string an operator retypes constantly,
/// into `raptorctl target show`, a `q=` filter, or a ticket.
fn yank_selected(app: &mut App) {
    let Some(cid) = app.selected_cid() else {
        return;
    };
    match osc52::copy(&cid) {
        Ok(()) => app.set_status(format!("copied {cid}")),
        Err(e) => app.set_status(format!("copy failed: {e}")),
    }
}

fn handle_search(app: &mut App, key: KeyEvent) {
    let Mode::Search { input } = &mut app.mode else {
        return;
    };
    match key.code {
        KeyCode::Esc => app.mode = Mode::Normal,
        KeyCode::Enter => {
            app.query = if input.is_empty() {
                None
            } else {
                Some(input.clone())
            };
            app.mode = Mode::Normal;
            app.fetch_targets();
        }
        KeyCode::Backspace => {
            input.pop();
        }
        KeyCode::Char(c) => input.push(c),
        _ => {}
    }
}

fn handle_tag_input(app: &mut App, key: KeyEvent) {
    let Mode::TagInput { cid, input } = &mut app.mode else {
        return;
    };
    match key.code {
        KeyCode::Esc => app.mode = Mode::Normal,
        KeyCode::Enter => {
            let (cid, tag) = (cid.clone(), input.clone());
            app.mode = Mode::Normal;
            if !tag.is_empty() {
                spawn_tag(app, cid, tag);
            }
        }
        KeyCode::Backspace => {
            input.pop();
        }
        KeyCode::Char(c) => input.push(c),
        _ => {}
    }
}

fn handle_assign(app: &mut App, key: KeyEvent) {
    let Mode::Assign {
        cid,
        filter,
        items,
        selected,
    } = &mut app.mode
    else {
        return;
    };
    let filtered: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, d)| {
            filter.is_empty() || d.name.to_lowercase().contains(&filter.to_lowercase())
        })
        .map(|(i, _)| i)
        .collect();
    match key.code {
        KeyCode::Esc => app.mode = Mode::Normal,
        KeyCode::Down => *selected = (*selected + 1).min(filtered.len().saturating_sub(1)),
        KeyCode::Up => *selected = selected.saturating_sub(1),
        KeyCode::Backspace => {
            filter.pop();
            *selected = 0;
        }
        KeyCode::Char(c) => {
            filter.push(c);
            *selected = 0;
        }
        KeyCode::Enter => {
            if let Some(&idx) = filtered.get(*selected) {
                let ds = &items[idx];
                let (cid, ds_id) = (cid.clone(), ds.id);
                let label = format!("{}:{}", ds.name, ds.version);
                app.mode = Mode::Normal;
                app.set_status(format!("assigning {label} to {cid}…"));
                spawn_assign(app, cid, ds_id);
            }
        }
        _ => {}
    }
}

fn spawn_assign(app: &App, cid: String, ds_id: i64) {
    let client = app.client.clone();
    let body = DsAssignment {
        id: ds_id,
        assign_type: None,
        forcetime: None,
        maintenance_window: None,
    };
    app.spawn(
        async move {
            let res = api::actions::assign(&client, &cid, &body).await?;
            Ok(format!(
                "assigned {} (already assigned {})",
                res.assigned, res.already_assigned
            ))
        },
        Msg::Done,
    );
}

fn spawn_cancel(app: &App, cid: String, action_id: i64) {
    let client = app.client.clone();
    app.spawn(
        async move {
            api::actions::cancel(&client, &cid, action_id, false).await?;
            Ok(format!("cancelled action {action_id} on {cid}"))
        },
        Msg::Done,
    );
}

fn spawn_force(app: &App, cid: String, action_id: i64) {
    let client = app.client.clone();
    app.spawn(
        async move {
            api::actions::force(&client, &cid, action_id).await?;
            Ok(format!("forced action {action_id} on {cid}"))
        },
        Msg::Done,
    );
}

fn spawn_tag(app: &App, cid: String, tag: String) {
    let client = app.client.clone();
    app.spawn(
        async move {
            let id = api::tags::find_id(&client, api::tags::Kind::Target, &tag).await?;
            api::tags::assign(&client, api::tags::Kind::Target, id, &cid).await?;
            Ok(format!("tagged {cid} with {tag}"))
        },
        Msg::Done,
    );
}
