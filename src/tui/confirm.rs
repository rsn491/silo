//! Inline Yes / No confirmation dialog built on ratatui + crossterm.

use std::io;

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal, TerminalOptions, Viewport,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

/// Show a full-screen Yes / No confirmation dialog.
///
/// Returns `true` when the user confirms, `false` when they decline or cancel.
///
/// # Errors
///
/// Returns an error if terminal setup or event polling fails.
pub fn run_confirm(prompt: &str, default_yes: bool) -> Result<bool, Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    const HEIGHT: u16 = 7;
    let mut terminal = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Inline(HEIGHT),
        },
    )?;
    // Capture viewport top row before the first draw so we can restore cursor on exit.
    let viewport_top = cursor::position().map(|(_, r)| r).unwrap_or(0);

    let mut yes_focused = default_yes;

    let result = run_confirm_loop(&mut terminal, prompt, &mut yes_focused, &mut event::read);

    disable_raw_mode()?;
    terminal.show_cursor()?;
    // Move to the last row of the viewport then newline so the shell prompt
    // appears below the widget instead of overwriting its bottom line.
    let _ = execute!(
        io::stdout(),
        cursor::MoveTo(0, viewport_top.saturating_add(HEIGHT - 1))
    );
    println!();

    result
}

/// Inner event loop for [`run_confirm`].
fn run_confirm_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    prompt: &str,
    yes_focused: &mut bool,
    next_event: &mut impl FnMut() -> io::Result<Event>,
) -> Result<bool, Box<dyn std::error::Error>>
where
    B::Error: 'static,
{
    loop {
        terminal.draw(|f| draw_confirm(f, prompt, *yes_focused))?;

        if let Event::Key(key) = next_event()? {
            match (key.code, key.modifiers) {
                (KeyCode::Char('c'), KeyModifiers::CONTROL) | (KeyCode::Esc, _) => {
                    return Ok(false);
                }
                (KeyCode::Enter, _) => {
                    return Ok(*yes_focused);
                }
                (KeyCode::Left, _)
                | (KeyCode::Right, _)
                | (KeyCode::Tab, _)
                | (KeyCode::Char('h'), _)
                | (KeyCode::Char('l'), _) => {
                    *yes_focused = !*yes_focused;
                }
                (KeyCode::Char('y'), _) | (KeyCode::Char('Y'), _) => {
                    return Ok(true);
                }
                (KeyCode::Char('n'), _) | (KeyCode::Char('N'), _) => {
                    return Ok(false);
                }
                _ => {}
            }
        }
    }
}

/// Render the confirmation dialog into the current frame.
fn draw_confirm(f: &mut ratatui::Frame<'_>, prompt: &str, yes_focused: bool) {
    let area = f.area();

    // Centre a small dialog box
    let dialog_height = 7u16;
    let dialog_width = (area.width / 2).max(50).min(area.width);
    let dialog_x = (area.width.saturating_sub(dialog_width)) / 2;
    let dialog_y = area.y + (area.height.saturating_sub(dialog_height)) / 2;
    let dialog_area = Rect::new(dialog_x, dialog_y, dialog_width, dialog_height);

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Min(1),    // prompt
            Constraint::Length(1), // spacer
            Constraint::Length(1), // buttons
            Constraint::Length(1), // hint
        ])
        .split(dialog_area);

    // Outer box
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));
    f.render_widget(block, dialog_area);

    // Prompt text
    let prompt_widget = Paragraph::new(prompt)
        .alignment(Alignment::Center)
        .style(Style::default().add_modifier(Modifier::BOLD));
    f.render_widget(prompt_widget, inner[0]);

    // Buttons
    let yes_style = if yes_focused {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let no_style = if !yes_focused {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let buttons = Paragraph::new(Line::from(vec![
        Span::styled("  [ Yes ]  ", yes_style),
        Span::raw("  "),
        Span::styled("  [ No ]  ", no_style),
    ]))
    .alignment(Alignment::Center);
    f.render_widget(buttons, inner[2]);

    // Hint
    let hint = Paragraph::new("←/→ switch  Enter confirm  y/n quick select")
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(hint, inner[3]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::test_support::{ctrl, event_source, key, screen_text, test_terminal};

    /// Runs the confirm loop over `events` and returns the result plus final focus.
    fn run(default_yes: bool, events: Vec<Event>) -> (bool, bool, String) {
        let mut terminal = test_terminal(60, 7);
        let mut yes_focused = default_yes;
        let result = run_confirm_loop(
            &mut terminal,
            "Proceed?",
            &mut yes_focused,
            &mut event_source(events),
        )
        .unwrap();
        (result, yes_focused, screen_text(&terminal))
    }

    #[test]
    fn enter_returns_default_choice() {
        // Given
        // (no setup needed)
        // When
        let actual_1 = run(true, vec![key(KeyCode::Enter)]).0;
        let actual_2 = !run(false, vec![key(KeyCode::Enter)]).0;
        // Then
        assert!(actual_1);
        assert!(actual_2);
    }

    #[test]
    fn navigation_keys_toggle_focus() {
        // Given
        let keys = [
            KeyCode::Left,
            KeyCode::Right,
            KeyCode::Tab,
            KeyCode::Char('h'),
            KeyCode::Char('l'),
        ];

        // When
        let outcomes: Vec<_> = keys
            .iter()
            .map(|code| run(false, vec![key(*code), key(KeyCode::Enter)]))
            .collect();

        // Then
        for (code, (result, focused, _)) in keys.iter().zip(outcomes) {
            assert!(result, "{:?} should move focus to Yes", code);
            assert!(focused);
        }
    }

    #[test]
    fn quick_select_keys() {
        // Given
        // (no setup needed)
        // When
        let actual_1 = run(false, vec![key(KeyCode::Char('y'))]).0;
        let actual_2 = run(false, vec![key(KeyCode::Char('Y'))]).0;
        let actual_3 = !run(true, vec![key(KeyCode::Char('n'))]).0;
        let actual_4 = !run(true, vec![key(KeyCode::Char('N'))]).0;
        // Then
        assert!(actual_1);
        assert!(actual_2);
        assert!(actual_3);
        assert!(actual_4);
    }

    #[test]
    fn cancel_keys_decline() {
        // Given
        // (no setup needed)
        // When
        let actual_1 = !run(true, vec![key(KeyCode::Esc)]).0;
        let actual_2 = !run(true, vec![ctrl('c')]).0;
        // Then
        assert!(actual_1);
        assert!(actual_2);
    }

    #[test]
    fn ignores_unrelated_events() {
        // Given
        let events = vec![
            Event::FocusGained,
            key(KeyCode::Char('x')),
            key(KeyCode::Enter),
        ];
        // When
        let actual = run(true, events).0;
        // Then
        assert!(actual);
    }

    #[test]
    fn renders_prompt_and_buttons() {
        // Given
        // (no setup needed)
        // When
        let (_, _, screen) = run(true, vec![key(KeyCode::Enter)]);
        // Then
        assert!(screen.contains("Proceed?"));
        assert!(screen.contains("[ Yes ]"));
        assert!(screen.contains("[ No ]"));
        assert!(screen.contains("y/n quick select"));
    }

    #[test]
    fn propagates_event_errors() {
        // Given
        let mut terminal = test_terminal(60, 7);
        let mut yes_focused = true;
        // When
        let result = run_confirm_loop(
            &mut terminal,
            "Proceed?",
            &mut yes_focused,
            &mut event_source(vec![]),
        );
        // Then
        assert!(result.is_err());
    }
}
