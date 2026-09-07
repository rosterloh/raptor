//! Terminal setup/teardown and the event loop. `ratatui::init`/`restore`
//! handle raw mode, the alternate screen, and a panic hook that restores the
//! terminal even if a render panics — no hand-rolled guard type needed.

mod actions;
mod app;
mod osc52;
mod panels;
mod theme;

use crate::client::Client;
use anyhow::Result;
use app::{App, Msg};
use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use futures_util::StreamExt;
use std::time::Duration;
use theme::Theme;

/// Cadence for time-driven state (status-message expiry, the auto-refresh
/// timer). Nothing is drawn on this clock alone — a tick only redraws
/// because it may have changed state.
const TICK: Duration = Duration::from_millis(250);

pub async fn run(client: Client, refresh_secs: u64) -> Result<()> {
    let mut terminal = ratatui::init();
    let theme = Theme::detect();
    let (mut app, mut rx) = App::new(client, refresh_secs);
    app.refresh_all();

    let result = event_loop(&mut terminal, &mut app, &mut rx, &theme).await;
    ratatui::restore();
    result
}

/// Event-driven: the loop blocks until a keypress, a completed request, or a
/// tick actually happens, then redraws once. Nothing repaints on an idle
/// screen, and a keypress is handled the moment it arrives rather than at the
/// next poll boundary.
async fn event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<Msg>,
    theme: &Theme,
) -> Result<()> {
    let mut events = EventStream::new();
    let mut ticker = tokio::time::interval(TICK);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    terminal.draw(|f| panels::draw(f, app, theme))?;
    loop {
        tokio::select! {
            Some(msg) = rx.recv() => app.handle_msg(msg),
            _ = ticker.tick() => app.tick(),
            event = events.next() => match event {
                Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                    if key.modifiers.contains(KeyModifiers::CONTROL)
                        && key.code == KeyCode::Char('c')
                    {
                        app.should_quit = true;
                    } else {
                        actions::handle_key(app, key);
                    }
                }
                // A resize needs no state change, just the redraw below.
                Some(Ok(Event::Resize(..))) => {}
                Some(Err(e)) => return Err(e.into()),
                // Key releases, mouse, focus: nothing to draw.
                Some(Ok(_)) => continue,
                None => return Ok(()),
            },
        }

        if app.should_quit {
            return Ok(());
        }
        terminal.draw(|f| panels::draw(f, app, theme))?;
    }
}
