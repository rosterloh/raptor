//! Semantic colour slots (tui-design skill §4). No hex/ANSI literal appears
//! outside this file — panel code asks for `theme.accent`, never a colour.

use ratatui::style::Color;

pub struct Theme {
    pub fg: Color,
    pub fg_muted: Color,
    pub fg_emphasis: Color,
    pub selection_bg: Color,
    pub accent: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub info: Color,
}

impl Theme {
    /// `NO_COLOR` collapses everything to the terminal default plus reverse
    /// video for selection — still fully readable, per the skill's tier-0
    /// requirement. Otherwise use portable ANSI names; they inherit the
    /// user's terminal theme rather than fighting it with fixed RGB.
    pub fn detect() -> Self {
        if std::env::var_os("NO_COLOR").is_some() {
            return Self {
                fg: Color::Reset,
                fg_muted: Color::Reset,
                fg_emphasis: Color::Reset,
                selection_bg: Color::Reset,
                accent: Color::Reset,
                success: Color::Reset,
                warning: Color::Reset,
                error: Color::Reset,
                info: Color::Reset,
            };
        }
        Self {
            fg: Color::Reset,
            fg_muted: Color::DarkGray,
            fg_emphasis: Color::White,
            selection_bg: Color::Blue,
            accent: Color::Cyan,
            success: Color::Green,
            warning: Color::Yellow,
            error: Color::Red,
            info: Color::Cyan,
        }
    }
}

/// Status glyph + colour slot for a target's `updateStatus`. Colour is never
/// the only signal — the glyph carries meaning in monochrome mode too.
pub fn status_glyph(status: &str) -> &'static str {
    match status {
        "in_sync" => "●",
        "pending" => "◐",
        "error" => "✕",
        _ => "○", // registered / unknown
    }
}

pub fn status_color(theme: &Theme, status: &str) -> Color {
    match status {
        "in_sync" => theme.success,
        "pending" => theme.warning,
        "error" => theme.error,
        _ => theme.fg_muted,
    }
}
