//! Inline interactive item selector built on ratatui + crossterm.

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
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph},
};

/// A single item shown in the selector list.
pub struct SelectItem {
    /// Primary label (shown in the list).
    pub label: String,
    /// Optional detail line shown below the selected item.
    pub detail: Option<String>,
}

/// Show an interactive full-screen selector and return the chosen index.
///
/// Returns `None` if the user cancels with `Esc` or `q`.
///
/// # Errors
///
/// Returns an error if terminal setup or event polling fails.
pub fn run_select(
    prompt: &str,
    items: &[SelectItem],
    default: usize,
) -> Result<Option<usize>, Box<dyn std::error::Error>> {
    if items.is_empty() {
        return Ok(None);
    }

    enable_raw_mode()?;
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let height = (items.len().min(10) + 8) as u16;
    let mut terminal = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Inline(height),
        },
    )?;
    let viewport_top = cursor::position().map(|(_, r)| r).unwrap_or(0);

    let mut state = ListState::default();
    state.select(Some(default.min(items.len().saturating_sub(1))));

    let result = run_select_loop(&mut terminal, items, prompt, &mut state, &mut event::read);

    disable_raw_mode()?;
    terminal.show_cursor()?;
    let _ = execute!(
        io::stdout(),
        cursor::MoveTo(0, viewport_top.saturating_add(height - 1))
    );
    println!();

    result
}

/// Inner event loop for [`run_select`].
fn run_select_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    items: &[SelectItem],
    prompt: &str,
    state: &mut ListState,
    next_event: &mut impl FnMut() -> io::Result<Event>,
) -> Result<Option<usize>, Box<dyn std::error::Error>>
where
    B::Error: 'static,
{
    loop {
        terminal.draw(|f| draw_select(f, items, prompt, state))?;

        if let Event::Key(key) = next_event()? {
            match (key.code, key.modifiers) {
                (KeyCode::Char('c'), KeyModifiers::CONTROL)
                | (KeyCode::Esc, _)
                | (KeyCode::Char('q'), _) => return Ok(None),

                (KeyCode::Enter, _) => {
                    return Ok(state.selected());
                }

                (KeyCode::Up, _) | (KeyCode::Char('k'), _) => {
                    let i = state.selected().unwrap_or(0);
                    if i > 0 {
                        state.select(Some(i - 1));
                    }
                }

                (KeyCode::Down, _) | (KeyCode::Char('j'), _) => {
                    let i = state.selected().unwrap_or(0);
                    if i + 1 < items.len() {
                        state.select(Some(i + 1));
                    }
                }

                (KeyCode::Home, _) | (KeyCode::Char('g'), _) => {
                    state.select(Some(0));
                }

                (KeyCode::End, _) | (KeyCode::Char('G'), _) => {
                    state.select(Some(items.len().saturating_sub(1)));
                }

                _ => {}
            }
        }
    }
}

/// Render the full selector UI into the current frame.
fn draw_select(
    f: &mut ratatui::Frame<'_>,
    items: &[SelectItem],
    prompt: &str,
    state: &mut ListState,
) {
    let area = f.area();

    // Layout: prompt row, list, footer hint
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // prompt box
            Constraint::Min(3),    // list
            Constraint::Length(1), // footer hint
        ])
        .split(area);

    draw_prompt(f, chunks[0], prompt);
    draw_list(f, chunks[1], items, state);
    draw_footer(f, chunks[2]);
}

/// Render the prompt label at the top of the selector.
fn draw_prompt(f: &mut ratatui::Frame<'_>, area: Rect, prompt: &str) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(prompt)
        .block(block)
        .style(Style::default().add_modifier(Modifier::BOLD));

    f.render_widget(paragraph, area);
}

/// Render the scrollable item list with the current selection highlighted.
fn draw_list(f: &mut ratatui::Frame<'_>, area: Rect, items: &[SelectItem], state: &mut ListState) {
    let list_items: Vec<ListItem> = items
        .iter()
        .map(|item| ListItem::new(Line::from(item.label.as_str())))
        .collect();

    let list = List::new(list_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("› ");

    f.render_stateful_widget(list, area, state);

    // Show detail line for selected item inside the list area (bottom of block).
    if let Some(idx) = state.selected()
        && let Some(detail) = items[idx].detail.as_deref()
    {
        let detail_area = Rect {
            x: area.x + 2,
            y: area.y + area.height.saturating_sub(2),
            width: area.width.saturating_sub(4),
            height: 1,
        };
        let detail_widget = Paragraph::new(detail).style(Style::default().fg(Color::DarkGray));
        f.render_widget(detail_widget, detail_area);
    }
}

/// Render the keyboard hint footer.
fn draw_footer(f: &mut ratatui::Frame<'_>, area: Rect) {
    let hint = Paragraph::new(Line::from(vec![
        Span::styled("↑/↓", Style::default().fg(Color::DarkGray)),
        Span::raw(" navigate  "),
        Span::styled("Enter", Style::default().fg(Color::DarkGray)),
        Span::raw(" select  "),
        Span::styled("Esc/q", Style::default().fg(Color::DarkGray)),
        Span::raw(" cancel"),
    ]))
    .alignment(Alignment::Center);
    f.render_widget(hint, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::test_support::{ctrl, event_source, key, screen_text, test_terminal};

    /// Three items; only the second has a detail line.
    fn items() -> Vec<SelectItem> {
        vec![
            SelectItem {
                label: "alpha".to_string(),
                detail: None,
            },
            SelectItem {
                label: "beta".to_string(),
                detail: Some("beta details".to_string()),
            },
            SelectItem {
                label: "gamma".to_string(),
                detail: None,
            },
        ]
    }

    /// Runs the select loop starting at `start` over `events`.
    fn run(start: usize, events: Vec<Event>) -> (Option<usize>, String) {
        let mut terminal = test_terminal(60, 14);
        let mut state = ListState::default();
        state.select(Some(start));
        let result = run_select_loop(
            &mut terminal,
            &items(),
            "Pick one",
            &mut state,
            &mut event_source(events),
        )
        .unwrap();
        (result, screen_text(&terminal))
    }

    #[test]
    fn enter_returns_current_selection() {
        // Given
        // (no setup needed)

        // When
        let actual = run(1, vec![key(KeyCode::Enter)]).0;

        // Then
        assert_eq!(actual, Some(1));
    }

    #[test]
    fn down_moves_selection_and_stops_at_last_item() {
        // Given
        let events = vec![
            key(KeyCode::Down),
            key(KeyCode::Char('j')),
            key(KeyCode::Down),
            key(KeyCode::Enter),
        ];

        // When
        let (selected, _) = run(0, events);

        // Then
        assert_eq!(selected, Some(2));
    }

    #[test]
    fn up_moves_selection_and_stops_at_first_item() {
        // Given
        let events = vec![
            key(KeyCode::Up),
            key(KeyCode::Char('k')),
            key(KeyCode::Up),
            key(KeyCode::Enter),
        ];

        // When
        let (selected, _) = run(2, events);

        // Then
        assert_eq!(selected, Some(0));
    }

    #[test]
    fn home_and_end_jump() {
        // Given
        // (no setup needed)

        // When
        let actual_1 = run(1, vec![key(KeyCode::End), key(KeyCode::Enter)]).0;
        let actual_2 = run(1, vec![key(KeyCode::Char('G')), key(KeyCode::Enter)]).0;
        let actual_3 = run(1, vec![key(KeyCode::Home), key(KeyCode::Enter)]).0;
        let actual_4 = run(1, vec![key(KeyCode::Char('g')), key(KeyCode::Enter)]).0;

        // Then
        assert_eq!(actual_1, Some(2));
        assert_eq!(actual_2, Some(2));
        assert_eq!(actual_3, Some(0));
        assert_eq!(actual_4, Some(0));
    }

    #[test]
    fn cancel_keys_return_none() {
        // Given
        // (no setup needed)

        // When
        let actual_1 = run(0, vec![key(KeyCode::Esc)]).0;
        let actual_2 = run(0, vec![key(KeyCode::Char('q'))]).0;
        let actual_3 = run(0, vec![ctrl('c')]).0;

        // Then
        assert_eq!(actual_1, None);
        assert_eq!(actual_2, None);
        assert_eq!(actual_3, None);
    }

    #[test]
    fn ignores_unrelated_events() {
        // Given
        let events = vec![Event::FocusGained, key(KeyCode::Tab), key(KeyCode::Enter)];

        // When
        let actual = run(2, events).0;

        // Then
        assert_eq!(actual, Some(2));
    }

    #[test]
    fn renders_prompt_items_and_selected_detail() {
        // Given
        // (no setup needed)

        // When
        let (_, screen) = run(1, vec![key(KeyCode::Enter)]);

        // Then
        assert!(screen.contains("Pick one"));
        assert!(screen.contains("alpha"));
        assert!(screen.contains("› beta"));
        assert!(screen.contains("beta details"));
        assert!(screen.contains("navigate"));
    }

    #[test]
    fn empty_items_return_none_without_terminal() {
        // Given
        // (no setup needed)

        // When
        let actual = run_select("Pick", &[], 0).unwrap();

        // Then
        assert_eq!(actual, None);
    }

    #[test]
    fn propagates_event_errors() {
        // Given
        let mut terminal = test_terminal(60, 14);
        let mut state = ListState::default();

        // When
        let result = run_select_loop(
            &mut terminal,
            &items(),
            "Pick",
            &mut state,
            &mut event_source(vec![]),
        );

        // Then
        assert!(result.is_err());
    }
}
