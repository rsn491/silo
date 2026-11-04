//! Helpers for driving TUI event loops in tests.

use std::collections::VecDeque;
use std::io;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};

/// A key press event without modifiers.
pub fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

/// A `Ctrl+<c>` key press event.
pub fn ctrl(c: char) -> Event {
    Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL))
}

/// Key press events for every character of `text`.
pub fn typed(text: &str) -> Vec<Event> {
    text.chars().map(|c| key(KeyCode::Char(c))).collect()
}

/// Returns an event source that yields `events` in order, then fails.
pub fn event_source(events: Vec<Event>) -> impl FnMut() -> io::Result<Event> {
    let mut queue: VecDeque<Event> = events.into();
    move || {
        queue
            .pop_front()
            .ok_or_else(|| io::Error::other("no more test events"))
    }
}

/// Creates an in-memory terminal of the given size.
pub fn test_terminal(width: u16, height: u16) -> Terminal<TestBackend> {
    Terminal::new(TestBackend::new(width, height)).expect("test terminal")
}

/// Returns all text currently rendered in the terminal's buffer, row by row.
pub fn screen_text(terminal: &Terminal<TestBackend>) -> String {
    let buffer = terminal.backend().buffer();
    let area = buffer.area;
    let mut text = String::new();
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            text.push_str(buffer[(x, y)].symbol());
        }
        text.push('\n');
    }
    text
}
