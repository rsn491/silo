//! Interactive mode UI using ratatui.

use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    DefaultTerminal,
};

use crate::infra::agent::{Agent, AgentMode};

/// The result of the interactive session.
pub struct InteractiveResult {
    /// The selected agent.
    pub agent: Agent,
    /// The selected mode.
    pub mode: AgentMode,
    /// The initial prompt.
    pub prompt: String,
}

/// State for the interactive UI.
struct App {
    /// The prompt string.
    prompt: String,
    /// The currently selected agent.
    agent: Agent,
    /// The currently selected mode.
    mode: AgentMode,
    /// Whether the session was cancelled.
    cancelled: bool,
}

impl App {
    /// Creates a new App.
    fn new(agent: Agent) -> Self {
        Self {
            prompt: String::new(),
            agent,
            mode: AgentMode::Plan,
            cancelled: false,
        }
    }

    /// Cycles to the next available agent.
    fn next_agent(&mut self) {
        use strum::IntoEnumIterator;
        let mut iter = Agent::iter();
        while let Some(a) = iter.next() {
            if a == self.agent {
                self.agent = iter.next().unwrap_or_else(|| Agent::iter().next().unwrap());
                return;
            }
        }
    }

    /// Toggles between Plan and Code modes.
    fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            AgentMode::Plan => AgentMode::Code,
            AgentMode::Code => AgentMode::Plan,
        };
    }
}

/// Runs the interactive UI and returns the selected configuration.
pub fn run(default_agent: Agent) -> io::Result<Option<InteractiveResult>> {
    let mut terminal = ratatui::init();
    let mut app = App::new(default_agent);

    let result = run_app(&mut terminal, &mut app);

    ratatui::restore();

    if result? && !app.cancelled {
        Ok(Some(InteractiveResult {
            agent: app.agent,
            mode: app.mode,
            prompt: app.prompt,
        }))
    } else {
        Ok(None)
    }
}

fn run_app(terminal: &mut DefaultTerminal, app: &mut App) -> io::Result<bool> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Esc => {
                                app.cancelled = true;
                                return Ok(false);
                            }
                            KeyCode::Enter => {
                                return Ok(true);
                            }
                            KeyCode::Tab => {
                                app.toggle_mode();
                            }
                            // Handle Cmd+. (often represented as Alt+. or Ctrl+. depending on terminal)
                            KeyCode::Char('.') if key.modifiers.contains(KeyModifiers::CONTROL) || key.modifiers.contains(KeyModifiers::ALT) => {
                                app.next_agent();
                            }
                            // Fallback: allow Ctrl+A to cycle agent
                            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                app.next_agent();
                            }
                            KeyCode::Char(' ') => {
                                app.prompt.push(' ');
                            }
                            KeyCode::Char(c) => {
                                app.prompt.push(c);
                            }
                            KeyCode::Backspace => {
                                app.prompt.pop();
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

fn ui(f: &mut ratatui::Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Status bar
            Constraint::Min(0),    // Prompt
            Constraint::Length(1), // Help bar
        ])
        .split(f.area());

    // Status bar
    let agent_str = format!(" Agent: {} ", app.agent);
    let mode_str = format!(" Mode: {} ", app.mode);

    let status_line = Line::from(vec![
        Span::styled(agent_str, Style::default().bg(Color::Blue).fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        Span::styled(mode_str, Style::default().bg(Color::Magenta).fg(Color::White).add_modifier(Modifier::BOLD)),
    ]);

    let status_para = Paragraph::new(status_line)
        .block(Block::default().borders(Borders::ALL).title(" Configuration "));
    f.render_widget(status_para, chunks[0]);

    // Prompt area
    let prompt_para = Paragraph::new(app.prompt.as_str())
        .block(Block::default().borders(Borders::ALL).title(" Initial Prompt "));
    f.render_widget(prompt_para, chunks[1]);

    // Help bar
    let help_line = Line::from(vec![
        Span::styled("Tab", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(": Toggle Mode  "),
        Span::styled("Ctrl+.", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(": Cycle Agent  "),
        Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(": Launch  "),
        Span::styled("Esc", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(": Cancel"),
    ]);
    f.render_widget(Paragraph::new(help_line), chunks[2]);
}
