//! Keybindings and the mutations they trigger (assign / cancel / force /
//! tag), including the inline confirmation state machine for the two
//! moderate-severity actions (tui-design skill §3, "Dialogs & Confirmation").

use super::app::{App, Mode, Msg};
use crate::api;
use crossterm::event::{KeyCode, KeyEvent};
use raptor_api_types::DsAssignment;

pub fn handle_key(app: &mut App, key: KeyEvent) {
    match &mut app.mode {
        Mode::Normal => handle_normal(app, key),
        Mode::Search { .. } => handle_search(app, key),
        Mode::Assign { .. } => handle_assign(app, key),
        Mode::TagInput { .. } => handle_tag_input(app, key),
        Mode::ConfirmCancel(aid) => {
            let aid = *aid;
            if key.code == KeyCode::Char('y') {
                spawn_cancel(app, aid);
            }
            app.mode = Mode::Normal;
        }
        Mode::ConfirmForce(aid) => {
            let aid = *aid;
            if key.code == KeyCode::Char('y') {
                spawn_force(app, aid);
            }
            app.mode = Mode::Normal;
        }
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
        KeyCode::Char('/') => {
            app.mode = Mode::Search {
                input: String::new(),
            }
        }
        KeyCode::Char('a') => {
            if app.selected_target().is_some() {
                app.fetch_ds_list();
            }
        }
        KeyCode::Char('t') => {
            if app.selected_target().is_some() {
                app.mode = Mode::TagInput {
                    input: String::new(),
                };
            }
        }
        KeyCode::Char('c') => {
            if let Some(a) = app.detail_actions.iter().find(|a| a.status == "pending") {
                app.mode = Mode::ConfirmCancel(a.id);
            } else {
                app.set_status("no active action to cancel");
            }
        }
        KeyCode::Char('f') => {
            if let Some(a) = app.detail_actions.iter().find(|a| a.status == "pending") {
                app.mode = Mode::ConfirmForce(a.id);
            } else {
                app.set_status("no active action to force");
            }
        }
        _ => {}
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
    let Mode::TagInput { input } = &mut app.mode else {
        return;
    };
    match key.code {
        KeyCode::Esc => app.mode = Mode::Normal,
        KeyCode::Enter => {
            let tag = input.clone();
            app.mode = Mode::Normal;
            if let Some(cid) = app.selected_target().map(|t| t.controller_id.clone())
                && !tag.is_empty()
            {
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
                let ds_id = items[idx].id;
                let cid = app.selected_target().map(|t| t.controller_id.clone());
                app.mode = Mode::Normal;
                if let Some(cid) = cid {
                    spawn_assign(app, cid, ds_id);
                }
            }
        }
        _ => {}
    }
}

fn spawn_assign(app: &App, cid: String, ds_id: i64) {
    let client = app.client.clone();
    let tx = app.tx.clone();
    tokio::spawn(async move {
        let body = DsAssignment {
            id: ds_id,
            assign_type: None,
            forcetime: None,
        };
        let r = api::actions::assign(&client, &cid, &body).await.map(|res| {
            format!(
                "assigned {} (already assigned {})",
                res.assigned, res.already_assigned
            )
        });
        let _ = tx.send(Msg::Done(r));
    });
}

fn spawn_cancel(app: &App, action_id: i64) {
    let Some(cid) = app.selected_target().map(|t| t.controller_id.clone()) else {
        return;
    };
    let client = app.client.clone();
    let tx = app.tx.clone();
    tokio::spawn(async move {
        let r = api::actions::cancel(&client, &cid, action_id, false)
            .await
            .map(|_| format!("cancelled action {action_id}"));
        let _ = tx.send(Msg::Done(r));
    });
}

fn spawn_force(app: &App, action_id: i64) {
    let Some(cid) = app.selected_target().map(|t| t.controller_id.clone()) else {
        return;
    };
    let client = app.client.clone();
    let tx = app.tx.clone();
    tokio::spawn(async move {
        let r = api::actions::force(&client, &cid, action_id)
            .await
            .map(|_| format!("forced action {action_id}"));
        let _ = tx.send(Msg::Done(r));
    });
}

fn spawn_tag(app: &App, cid: String, tag: String) {
    let client = app.client.clone();
    let tx = app.tx.clone();
    tokio::spawn(async move {
        let r = async {
            let id = api::tags::find_id(&client, &tag).await?;
            api::tags::assign(&client, id, &cid).await?;
            Ok(format!("tagged {cid} with {tag}"))
        }
        .await;
        let _ = tx.send(Msg::Done(r));
    });
}
