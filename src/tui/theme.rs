//! Shared style constants for the Silo TUI.

use ratatui::style::{Color, Modifier, Style};

pub const HEADER: Style = Style::new()
    .fg(Color::Cyan)
    .add_modifier(Modifier::BOLD);

pub const SELECTED: Style = Style::new()
    .bg(Color::DarkGray)
    .add_modifier(Modifier::BOLD);

pub const SUCCESS: Style = Style::new().fg(Color::Green);

pub const WARNING: Style = Style::new().fg(Color::Yellow);

pub const ERROR: Style = Style::new().fg(Color::Red);

pub const DIM: Style = Style::new().fg(Color::DarkGray);

pub const BORDER: Style = Style::new().fg(Color::Cyan);

pub const NORMAL: Style = Style::new();
