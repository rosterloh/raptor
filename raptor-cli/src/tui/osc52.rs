//! Clipboard writes via the OSC 52 terminal escape. The sequence is
//! interpreted by whatever terminal emulator the operator is sitting in
//! front of, so a yank from a raptorctl running over SSH lands in the *local*
//! clipboard — no X/Wayland connection on the server needed.
//!
//! Written between draws, when ratatui has already flushed its own buffer,
//! so the escape can never be spliced into a half-written frame.

use anyhow::Result;
use base64::Engine;
use std::io::Write;

/// Not every terminal honours OSC 52 (and some only with an opt-in setting),
/// and none of them reply, so a successful write is all we can report.
pub fn copy(text: &str) -> Result<()> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(text);
    let mut out = std::io::stdout();
    write!(out, "\x1b]52;c;{encoded}\x07")?;
    out.flush()?;
    Ok(())
}
