//! Full-screen text input widget built on ratatui + crossterm.

use std::io;

use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, Paragraph},
};

/// Show a full-screen single-line text input prompt.
///
/// Returns the entered string, or `None` if the user presses `Esc`.
///
/// # Errors
///
/// Returns an error if terminal setup or event polling fails.
pub fn run_input(
    prompt: &str,
    initial: Option<&str>,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut value: Vec<char> = initial.unwrap_or("").chars().collect();
    let mut cursor = value.len();

    let result = run_input_loop(&mut terminal, prompt, &mut value, &mut cursor);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

/// Inner event loop for [`run_input`].
#[allow(clippy::too_many_lines)]
fn run_input_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    prompt: &str,
    value: &mut Vec<char>,
    cursor: &mut usize,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    loop {
        terminal.draw(|f| draw_input(f, prompt, value, *cursor))?;

        if let Event::Key(key) = event::read()? {
            match (key.code, key.modifiers) {
                (KeyCode::Char('c'), KeyModifiers::CONTROL) | (KeyCode::Esc, _) => {
                    return Ok(None);
                }

                (KeyCode::Enter, _) => {
                    return Ok(Some(value.iter().collect()));
                }

                (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
                    value.drain(..*cursor);
                    *cursor = 0;
                }

                (KeyCode::Char('k'), KeyModifiers::CONTROL) => {
                    value.truncate(*cursor);
                }

                (KeyCode::Char(c), _) => {
                    value.insert(*cursor, c);
                    *cursor += 1;
                }

                (KeyCode::Backspace, _) => {
                    if *cursor > 0 {
                        *cursor -= 1;
                        value.remove(*cursor);
                    }
                }

                (KeyCode::Delete, _) => {
                    if *cursor < value.len() {
                        value.remove(*cursor);
                    }
                }

                (KeyCode::Left, _) => {
                    if *cursor > 0 {
                        *cursor -= 1;
                    }
                }

                (KeyCode::Right, _) => {
                    if *cursor < value.len() {
                        *cursor += 1;
                    }
                }

                (KeyCode::Home, _) => {
                    *cursor = 0;
                }

                (KeyCode::End, _) => {
                    *cursor = value.len();
                }

                _ => {}
            }
        }
    }
}

/// Render the text input dialog into the current frame.
fn draw_input(f: &mut ratatui::Frame<'_>, prompt: &str, value: &[char], cursor: usize) {
    let area = f.area();

    let dialog_height = 7u16;
    let dialog_width = (area.width * 2 / 3).max(60).min(area.width);
    let dialog_x = (area.width.saturating_sub(dialog_width)) / 2;
    let dialog_y = (area.height.saturating_sub(dialog_height)) / 2;
    let dialog_area = Rect::new(dialog_x, dialog_y, dialog_width, dialog_height);

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1), // prompt label
            Constraint::Length(1), // spacer
            Constraint::Length(1), // text field
            Constraint::Length(1), // hint
        ])
        .split(dialog_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));
    f.render_widget(block, dialog_area);

    let prompt_widget = Paragraph::new(prompt)
        .style(Style::default().add_modifier(Modifier::BOLD))
        .alignment(Alignment::Left);
    f.render_widget(prompt_widget, inner[0]);

    // Build text with cursor block
    let text: String = value.iter().collect();
    let before_cursor: String = value[..cursor].iter().collect();
    let cursor_char: String = value
        .get(cursor)
        .map(|c| c.to_string())
        .unwrap_or_else(|| " ".to_string());
    let after_cursor: String = if cursor < value.len() {
        value[cursor + 1..].iter().collect()
    } else {
        String::new()
    };

    use ratatui::text::{Line, Span};
    let _ = text; // suppress unused warning
    let field_line = Line::from(vec![
        Span::raw(before_cursor),
        Span::styled(
            cursor_char,
            Style::default().bg(Color::Cyan).fg(Color::Black),
        ),
        Span::raw(after_cursor),
    ]);
    let field = Paragraph::new(field_line).style(Style::default()).block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    f.render_widget(field, inner[2]);

    let hint = Paragraph::new("Enter confirm  Esc cancel  Ctrl+U clear")
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(hint, inner[3]);
}
