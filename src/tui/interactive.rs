//! Inline interactive agent launcher TUI built on ratatui.

use std::io;

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::{Frame, Terminal, backend::CrosstermBackend};
use strum::IntoEnumIterator;

use crate::infra::agent::{Agent, AgentMode};

/// The result returned when the TUI event loop exits.
#[derive(Debug)]
pub enum AppOutcome {
    /// User pressed Enter: launch the agent with the given settings.
    Launch {
        /// The agent selected by the user.
        agent: Agent,
        /// The mode selected by the user.
        mode: AgentMode,
        /// The prompt typed by the user.
        prompt: String,
    },
    /// User pressed Esc or Ctrl+C: exit without launching.
    Quit,
}

/// Holds the full runtime state of the interactive TUI.
struct App {
    agent: Agent,
    agents: Vec<Agent>,
    agent_index: usize,
    mode: AgentMode,
    prompt: String,
    cursor: usize,
    outcome: Option<AppOutcome>,
}

impl App {
    fn new(default_agent: Agent) -> Self {
        let agents: Vec<Agent> = Agent::iter().collect();
        let agent_index = agents.iter().position(|a| a == &default_agent).unwrap_or(0);
        Self {
            agent: default_agent,
            agents,
            agent_index,
            mode: AgentMode::Code,
            prompt: String::new(),
            cursor: 0,
            outcome: None,
        }
    }

    fn next_agent(&mut self) {
        self.agent_index = (self.agent_index + 1) % self.agents.len();
        self.agent = self.agents[self.agent_index].clone();
    }

    fn toggle_mode(&mut self) {
        self.mode = self.mode.toggle();
    }

    fn insert_char(&mut self, c: char) {
        let byte_pos = self.char_to_byte_pos(self.cursor);
        self.prompt.insert(byte_pos, c);
        self.cursor += 1;
    }

    fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let byte_pos = self.char_to_byte_pos(self.cursor);
        if let Some((char_start, _)) = self.prompt[..byte_pos].char_indices().next_back() {
            self.prompt.drain(char_start..byte_pos);
            self.cursor -= 1;
        }
    }

    fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    fn move_right(&mut self) {
        if self.cursor < self.prompt.chars().count() {
            self.cursor += 1;
        }
    }

    fn char_to_byte_pos(&self, char_pos: usize) -> usize {
        self.prompt
            .char_indices()
            .nth(char_pos)
            .map(|(i, _)| i)
            .unwrap_or(self.prompt.len())
    }

    fn launch(&mut self) {
        self.outcome = Some(AppOutcome::Launch {
            agent: self.agent.clone(),
            mode: self.mode,
            prompt: self.prompt.clone(),
        });
    }

    fn quit(&mut self) {
        self.outcome = Some(AppOutcome::Quit);
    }

    fn should_exit(&self) -> bool {
        self.outcome.is_some()
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Tab => self.toggle_mode(),
            KeyCode::Char('.') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.next_agent();
            }
            KeyCode::Enter => self.launch(),
            KeyCode::Esc => self.quit(),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => self.quit(),
            KeyCode::Char(c) => self.insert_char(c),
            KeyCode::Backspace => self.backspace(),
            KeyCode::Left => self.move_left(),
            KeyCode::Right => self.move_right(),
            _ => {}
        }
    }
}

fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(area);

    let title = Paragraph::new("silo — interactive agent launcher")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(title, chunks[0]);

    let mode_color = match app.mode {
        AgentMode::Code => Color::Green,
        AgentMode::Plan => Color::Yellow,
    };
    let status_line = Line::from(vec![
        Span::raw("  Agent: "),
        Span::styled(
            app.agent.to_string(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("        Mode: "),
        Span::styled(
            format!("[{}]", app.mode.display_name()),
            Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
        ),
    ]);
    let status =
        Paragraph::new(status_line).block(Block::default().borders(Borders::ALL).title("Settings"));
    frame.render_widget(status, chunks[1]);

    let prompt_chars: Vec<char> = app.prompt.chars().collect();
    let before_cursor: String = prompt_chars[..app.cursor].iter().collect();
    let at_cursor: String = prompt_chars
        .get(app.cursor)
        .map(|c| c.to_string())
        .unwrap_or_else(|| " ".to_string());
    let after_cursor: String = if app.cursor < prompt_chars.len() {
        prompt_chars[app.cursor + 1..].iter().collect()
    } else {
        String::new()
    };

    let input_line = Line::from(vec![
        Span::raw("> "),
        Span::raw(before_cursor),
        Span::styled(
            at_cursor,
            Style::default().bg(Color::White).fg(Color::Black),
        ),
        Span::raw(after_cursor),
    ]);
    let prompt_widget = Paragraph::new(vec![Line::from(""), input_line])
        .block(Block::default().borders(Borders::ALL).title("Prompt"));
    frame.render_widget(prompt_widget, chunks[2]);

    let help_line = Line::from(vec![
        Span::styled(" [Tab]", Style::default().fg(Color::Yellow)),
        Span::raw(" mode  "),
        Span::styled("[Ctrl+.]", Style::default().fg(Color::Yellow)),
        Span::raw(" agent  "),
        Span::styled("[Enter]", Style::default().fg(Color::Green)),
        Span::raw(" launch  "),
        Span::styled("[Esc]", Style::default().fg(Color::Red)),
        Span::raw(" quit "),
    ]);
    let help = Paragraph::new(help_line).block(Block::default().borders(Borders::ALL));
    frame.render_widget(help, chunks[3]);
}

/// Runs the interactive TUI event loop and returns the user's choice.
///
/// # Errors
///
/// Returns an [`io::Error`] if the terminal cannot be initialised or if a
/// read/write error occurs during event processing.
pub fn run(default_agent: Agent) -> io::Result<AppOutcome> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(default_agent);
    let loop_result = run_event_loop(&mut terminal, &mut app);

    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    loop_result?;

    Ok(app.outcome.unwrap_or(AppOutcome::Quit))
}

fn run_event_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> io::Result<()>
where
    io::Error: From<B::Error>,
{
    loop {
        terminal.draw(|f| draw(f, app))?;

        if let Event::Key(key) = event::read()? {
            app.handle_key(key);
        }

        if app.should_exit() {
            break;
        }
    }
    Ok(())
}
