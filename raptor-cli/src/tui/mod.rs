//! Terminal setup/teardown and the event loop. `ratatui::init`/`restore`
//! handle raw mode, the alternate screen, and a panic hook that restores the
//! terminal even if a render panics — no hand-rolled guard type needed.

mod actions;
mod app;
mod panels;
mod theme;

use crate::client::Client;
use anyhow::Result;
use app::{App, Msg};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use std::time::Duration;
use theme::Theme;

/// How often to poll for a keypress between redraws. Short enough that
/// input feels instant, long enough not to spin the CPU.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

pub async fn run(client: Client, refresh_secs: u64) -> Result<()> {
    let mut terminal = ratatui::init();
    let theme = Theme::detect();
    let (mut app, mut rx) = App::new(client, refresh_secs);
    app.refresh_all();

    let result = event_loop(&mut terminal, &mut app, &mut rx, &theme).await;
    ratatui::restore();
    result
}

async fn event_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<Msg>,
    theme: &Theme,
) -> Result<()> {
    loop {
        while let Ok(msg) = rx.try_recv() {
            app.handle_msg(msg);
        }
        app.tick();

        terminal.draw(|f| panels::draw(f, app, theme))?;

        if event::poll(POLL_INTERVAL)?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                app.should_quit = true;
            } else {
                actions::handle_key(app, key);
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}
