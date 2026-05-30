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

    let result = run_confirm_loop(&mut terminal, prompt, &mut yes_focused);

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
) -> Result<bool, Box<dyn std::error::Error>> {
    loop {
        terminal.draw(|f| draw_confirm(f, prompt, *yes_focused))?;

        if let Event::Key(key) = event::read()? {
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
