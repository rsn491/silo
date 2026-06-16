//! Logic for the `ps` command.

use std::io;
use std::time::Duration;

use clap::Args;

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{Clear, ClearType, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal, TerminalOptions, Viewport,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table, TableState},
};

use crate::infra::agent::Agent;
use crate::infra::agent::icons::{
    ICON_CHAR_H, ICON_CHAR_W, agent_icon_index, all_table_icons,
};
use crate::infra::git::GitOperations;
use crate::infra::system_process::ProcessOperations;
use crate::services::agent_list_service::{AgentListService, RunningAgent};

/// Arguments for the `ps` subcommand.
#[derive(Args)]
pub struct PsArgs {
    /// Print agents as plain text and exit without entering the interactive TUI.
    #[arg(long)]
    pub plain: bool,
}

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

    /// Executes the `ps` operation.
    ///
    /// In plain mode, prints agents as a fixed-width table and exits immediately.
    /// Otherwise shows a live-updating TUI dashboard; press `q` or `Ctrl-C` to exit.
    ///
    /// # Errors
    ///
    /// Returns an error if listing running agents or terminal setup fails.
    pub fn run(&self, args: PsArgs) -> Result<(), Box<dyn std::error::Error>> {
        if args.plain {
            return self.run_plain();
        }

        let icons = all_table_icons();
        let mut agents = self.service.list_running_agents().unwrap_or_default();
        let mut height = viewport_height(agents.len());
        let mut table_state = TableState::default();

        enable_raw_mode()?;
        let mut terminal = Terminal::with_options(
            CrosstermBackend::new(io::stdout()),
            TerminalOptions {
                viewport: Viewport::Inline(height),
            },
        )?;

        loop {
            terminal.draw(|f| draw_ps(f, &agents, &mut table_state, &icons))?;
            if event::poll(Duration::from_secs(2))? {
                if let Event::Key(key) = event::read()? {
                    match (key.code, key.modifiers) {
                        (KeyCode::Char('c'), KeyModifiers::CONTROL)
                        | (KeyCode::Char('q'), _)
                        | (KeyCode::Esc, _) => {
                            disable_raw_mode()?;
                            terminal.show_cursor()?;
                            println!();
                            return Ok(());
                        }
                        _ => {}
                    }
                }
            } else {
                let new_agents = self.service.list_running_agents().unwrap_or_default();
                let new_height = viewport_height(new_agents.len());
                agents = new_agents;

                if new_height > height {
                    // The viewport must grow. Clear the old area before recreating
                    // the terminal so ratatui doesn't scroll and garble old content.
                    disable_raw_mode()?;
                    terminal.show_cursor()?;
                    execute!(
                        io::stdout(),
                        cursor::MoveUp(height),
                        Clear(ClearType::FromCursorDown)
                    )?;
                    height = new_height;
                    enable_raw_mode()?;
                    terminal = Terminal::with_options(
                        CrosstermBackend::new(io::stdout()),
                        TerminalOptions {
                            viewport: Viewport::Inline(height),
                        },
                    )?;
                }
            }
        }
    }

    /// Prints agents as a fixed-width plain-text table and returns.
    ///
    /// # Errors
    ///
    /// Returns an error if listing running agents fails.
    fn run_plain(&self) -> Result<(), Box<dyn std::error::Error>> {
        let agents = self.service.list_running_agents().unwrap_or_default();
        if agents.is_empty() {
            println!("No running agents found in this repository's workspaces.");
            return Ok(());
        }
        println!("{:<8} {:<15} {:<28} WORKSPACE", "PID", "AGENT", "BRANCH");
        for a in &agents {
            let agent_display = a
                .agent_type
                .as_ref()
                .map(|ag| format!("{} {}", ag.icon(), ag))
                .unwrap_or_else(|| "(unknown)".to_string());
            println!(
                "{:<8} {:<15} {:<28} {}",
                a.pid,
                agent_display,
                a.branch.as_deref().unwrap_or("(detached)"),
                a.path.display(),
            );
        }
        Ok(())
    }
}

/// Calculates the inline viewport height needed for the given number of agents.
fn viewport_height(agent_count: usize) -> u16 {
    // Each data row is ICON_CHAR_H rows tall. Add borders (2), header (1), footer (1), buffer (4).
    let data_rows = (agent_count.min(15) as u16) * ICON_CHAR_H;
    (data_rows + 8).max(8 + ICON_CHAR_H)
}

/// Returns the display colour for the given agent type.
fn agent_color(agent: &Agent) -> Color {
    match agent {
        Agent::ClaudeCode => Color::Cyan,
        Agent::Codex => Color::Yellow,
        Agent::Gemini => Color::Blue,
        Agent::OpenCode => Color::Magenta,
        Agent::Droid => Color::Green,
    }
}

/// Render the running-agents dashboard into the current frame.
fn draw_ps(
    f: &mut ratatui::Frame<'_>,
    agents: &[RunningAgent],
    table_state: &mut TableState,
    icons: &[Vec<Line<'static>>; 5],
) {
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
            Cell::from("").style(Style::default()),
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

                let icon_text: Text<'_> = a
                    .agent_type
                    .as_ref()
                    .map(|at| Text::from(icons[agent_icon_index(at)].clone()))
                    .unwrap_or_else(|| {
                        Text::from(
                            (0..ICON_CHAR_H)
                                .map(|_| Line::default())
                                .collect::<Vec<_>>(),
                        )
                    });

                Row::new(vec![
                    Cell::from(icon_text),
                    Cell::from(a.pid.to_string()),
                    Cell::from(agent_name).style(Style::default().fg(color)),
                    Cell::from(a.branch.as_deref().unwrap_or("(detached)").to_string()),
                    Cell::from(a.path.display().to_string()),
                ])
                .height(ICON_CHAR_H)
            })
            .collect();

        let widths = [
            Constraint::Length(ICON_CHAR_W),
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
        Span::raw(" quit"),
    ]));
    f.render_widget(footer, chunks[1]);
}
