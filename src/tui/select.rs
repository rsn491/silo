//! Full-screen interactive item selector built on ratatui + crossterm.

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
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = ListState::default();
    state.select(Some(default.min(items.len().saturating_sub(1))));

    let result = run_select_loop(&mut terminal, items, prompt, &mut state);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_select_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    items: &[SelectItem],
    prompt: &str,
    state: &mut ListState,
) -> Result<Option<usize>, Box<dyn std::error::Error>> {
    loop {
        terminal.draw(|f| draw_select(f, items, prompt, state))?;

        if let Event::Key(key) = event::read()? {
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
            Constraint::Length(3),  // prompt box
            Constraint::Min(3),     // list
            Constraint::Length(1),  // footer hint
        ])
        .split(area);

    draw_prompt(f, chunks[0], prompt);
    draw_list(f, chunks[1], items, state);
    draw_footer(f, chunks[2]);
}

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

fn draw_list(
    f: &mut ratatui::Frame<'_>,
    area: Rect,
    items: &[SelectItem],
    state: &mut ListState,
) {
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

    // Show detail line for selected item inside the list area (bottom of block)
    if let Some(idx) = state.selected() {
        if let Some(detail) = items[idx].detail.as_deref() {
            let detail_area = Rect {
                x: area.x + 2,
                y: area.y + area.height.saturating_sub(2),
                width: area.width.saturating_sub(4),
                height: 1,
            };
            let detail_widget =
                Paragraph::new(detail).style(Style::default().fg(Color::DarkGray));
            f.render_widget(detail_widget, detail_area);
        }
    }
}

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
