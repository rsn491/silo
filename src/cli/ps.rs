//! Logic for the `ps` command.

use std::io;
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal, TerminalOptions, Viewport,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table, TableState},
};

use crate::infra::agent::Agent;
use crate::infra::git::GitOperations;
use crate::infra::system_process::ProcessOperations;
use crate::services::agent_list_service::{AgentListService, RunningAgent};

/// Handler for the `ps` command.
pub struct PsCommand<G: GitOperations + Clone, P: ProcessOperations> {
    /// Service for enumerating running agents.
    service: AgentListService<G, P>,
}

impl<G: GitOperations + Clone, P: ProcessOperations> PsCommand<G, P> {
    /// Creates a new `PsCommand`.
    pub fn new(service: AgentListService<G, P>) -> Self {
        Self { service }
    }

    /// Executes the `ps` operation, showing a live-updating dashboard of running agents.
    ///
    /// Press `q` or `Ctrl-C` to exit.
    ///
    /// # Errors
    ///
    /// Returns an error if listing running agents or terminal setup fails.
    pub fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        let agents = self.service.list_running_agents().unwrap_or_default();
        let height = (agents.len().min(15) + 4) as u16;

        enable_raw_mode()?;
        let stdout = io::stdout();
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Inline(height.max(5)),
            },
        )?;

        let result = self.run_loop(&mut terminal, agents);

        disable_raw_mode()?;
        terminal.show_cursor()?;

        result
    }

    /// Inner draw/event loop for the live-updating dashboard.
    fn run_loop<B: ratatui::backend::Backend>(
        &self,
        terminal: &mut Terminal<B>,
        initial_agents: Vec<RunningAgent>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut agents = initial_agents;
        let mut table_state = TableState::default();

        loop {
            terminal.draw(|f| draw_ps(f, &agents, &mut table_state))?;

            // Poll for key events; refresh data every 2 seconds.
            if event::poll(Duration::from_secs(2))? {
                if let Event::Key(key) = event::read()? {
                    match (key.code, key.modifiers) {
                        (KeyCode::Char('c'), KeyModifiers::CONTROL)
                        | (KeyCode::Char('q'), _)
                        | (KeyCode::Esc, _) => break,
                        _ => {}
                    }
                }
            } else {
                // Timeout — refresh agent list.
                agents = self.service.list_running_agents().unwrap_or_default();
            }
        }

        Ok(())
    }
}

/// Returns the display colour for the given agent type.
fn agent_color(agent: &Agent) -> Color {
    match agent {
        Agent::ClaudeCode => Color::Cyan,
        Agent::Codex => Color::Yellow,
        Agent::Gemini => Color::Blue,
        Agent::OpenCode => Color::Magenta,
    }
}

/// Render the running-agents dashboard into the current frame.
fn draw_ps(f: &mut ratatui::Frame<'_>, agents: &[RunningAgent], table_state: &mut TableState) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // table
            Constraint::Length(1), // footer
        ])
        .split(area);

    let agent_count = agents.len();

    if agents.is_empty() {
        let msg = Paragraph::new("No running agents found in this repository's workspaces.")
            .style(Style::default().fg(Color::DarkGray))
            .block(
                Block::default()
                    .title(Span::styled(
                        " Silo — Running Agents ",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ))
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Cyan)),
            );
        f.render_widget(msg, chunks[0]);
    } else {
        let header = Row::new(vec![
            Cell::from("PID").style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Cell::from("AGENT").style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Cell::from("BRANCH").style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Cell::from("WORKSPACE").style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ])
        .height(1);

        let rows: Vec<Row> = agents
            .iter()
            .map(|a| {
                let color = a
                    .agent_type
                    .as_ref()
                    .map(agent_color)
                    .unwrap_or(Color::White);
                let agent_name = a
                    .agent_type
                    .as_ref()
                    .map(|ag| ag.to_string())
                    .unwrap_or_else(|| "(unknown)".to_string());
                Row::new(vec![
                    Cell::from(a.pid.to_string()),
                    Cell::from(agent_name).style(Style::default().fg(color)),
                    Cell::from(a.branch.as_deref().unwrap_or("(detached)").to_string()),
                    Cell::from(a.path.display().to_string()),
                ])
            })
            .collect();

        let widths = [
            Constraint::Length(8),
            Constraint::Length(12),
            Constraint::Length(28),
            Constraint::Min(20),
        ];

        let title = format!(" Silo — Running Agents ({}) ", agent_count);
        let table = Table::new(rows, widths)
            .header(header)
            .block(
                Block::default()
                    .title(Span::styled(
                        title,
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ))
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .column_spacing(2);

        f.render_stateful_widget(table, chunks[0], table_state);
    }

    let footer = Paragraph::new(Line::from(vec![
        Span::styled("q", Style::default().fg(Color::DarkGray)),
        Span::raw(" quit  "),
        Span::styled(
            "auto-refreshes every 2s",
            Style::default().fg(Color::DarkGray),
        ),
    ]));
    f.render_widget(footer, chunks[1]);
}
