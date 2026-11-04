//! Logic for the `ps` command.

use std::io::{self, Write};
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
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table, TableState},
};

use crate::infra::agent::Agent;
use crate::infra::system_process::ProcessOperations;
use crate::services::agent_list_service::{AgentListService, RunningAgent};
use crate::services::workspace_manager::WorkspaceManager;

/// Arguments for the `ps` subcommand.
#[derive(Args)]
pub struct PsArgs {
    /// Print agents as plain text and exit without entering the interactive TUI.
    #[arg(long)]
    pub plain: bool,
}

/// Handler for the `ps` command.
pub struct PsCommand<W: WorkspaceManager, P: ProcessOperations> {
    /// Service for enumerating running agents.
    service: AgentListService<W, P>,
}

/// What the dashboard loop should do after handling one poll result.
#[derive(Debug, PartialEq, Eq)]
enum Tick {
    /// The user asked to quit.
    Quit,
    /// Keep running with the current viewport.
    Continue,
    /// The agent list grew; the viewport must be recreated with this height.
    Grow(u16),
}

impl<W: WorkspaceManager, P: ProcessOperations> PsCommand<W, P> {
    /// Creates a new `PsCommand`.
    pub fn new(service: AgentListService<W, P>) -> Self {
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
            terminal.draw(|f| draw_ps(f, &agents, &mut table_state))?;
            let event = if event::poll(Duration::from_secs(2))? {
                Some(event::read()?)
            } else {
                None
            };
            match self.handle_tick(event, &mut agents, height) {
                Tick::Quit => {
                    disable_raw_mode()?;
                    terminal.show_cursor()?;
                    println!();
                    return Ok(());
                }
                Tick::Continue => {}
                Tick::Grow(new_height) => {
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

    /// Handles one poll result: a key event (`Some`) or a refresh timeout (`None`).
    ///
    /// On timeout the agent list is refreshed in place.
    fn handle_tick(
        &self,
        event: Option<Event>,
        agents: &mut Vec<RunningAgent>,
        height: u16,
    ) -> Tick {
        match event {
            Some(Event::Key(key)) => match (key.code, key.modifiers) {
                (KeyCode::Char('c'), KeyModifiers::CONTROL)
                | (KeyCode::Char('q'), _)
                | (KeyCode::Esc, _) => Tick::Quit,
                _ => Tick::Continue,
            },
            Some(_) => Tick::Continue,
            None => {
                *agents = self.service.list_running_agents().unwrap_or_default();
                let new_height = viewport_height(agents.len());
                if new_height > height {
                    Tick::Grow(new_height)
                } else {
                    Tick::Continue
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
        self.write_plain(&mut io::stdout())
    }

    /// Writes agents as a fixed-width plain-text table to `out`.
    ///
    /// # Errors
    ///
    /// Returns an error if writing to `out` fails.
    fn write_plain(&self, out: &mut impl Write) -> Result<(), Box<dyn std::error::Error>> {
        let agents = self.service.list_running_agents().unwrap_or_default();
        if agents.is_empty() {
            writeln!(
                out,
                "No running agents found in this repository's workspaces."
            )?;
            return Ok(());
        }
        writeln!(
            out,
            "{:<8} {:<12} {:<28} WORKSPACE",
            "PID", "AGENT", "BRANCH"
        )?;
        for a in &agents {
            let agent_name = a
                .agent_type
                .as_ref()
                .map(|ag| ag.to_string())
                .unwrap_or_else(|| "(unknown)".to_string());
            writeln!(
                out,
                "{:<8} {:<12} {:<28} {}",
                a.pid,
                agent_name,
                a.branch.as_deref().unwrap_or("(detached)"),
                a.path.display(),
            )?;
        }
        Ok(())
    }
}

/// Calculates the inline viewport height needed for the given number of agents.
fn viewport_height(agent_count: usize) -> u16 {
    // +2 borders +1 header +1 footer outside box +4 buffer rows below last agent
    (agent_count.min(15) + 8).max(8) as u16
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
        Span::raw(" quit"),
    ]));
    f.render_widget(footer, chunks[1]);
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::infra::git::GitWorkspaceInfo;
    use crate::infra::system_process::MockProcessOperations;
    use crate::services::workspace_manager::MockWorkspaceManager;
    use crate::tui::test_support::{ctrl, key, screen_text, test_terminal};

    /// A ps command whose workspaces contain processes for each `(pid, args)` pair.
    fn command(
        processes: Vec<(u32, &'static str)>,
    ) -> PsCommand<MockWorkspaceManager, MockProcessOperations> {
        let mut workspaces = MockWorkspaceManager::new();
        workspaces.expect_get_all().returning(|_| {
            Ok(vec![
                GitWorkspaceInfo {
                    path: PathBuf::from("/silo/ws-main"),
                    branch: Some("main".to_string()),
                    ..Default::default()
                },
                GitWorkspaceInfo {
                    path: PathBuf::from("/silo/ws-detached"),
                    branch: None,
                    ..Default::default()
                },
            ])
        });
        let mut process = MockProcessOperations::new();
        process
            .expect_find_processes_by_names()
            .returning(move |_| {
                Ok(processes
                    .iter()
                    .map(|(pid, args)| (*pid, args.to_string()))
                    .collect())
            });
        process.expect_get_process_cwd().returning(|pid| {
            Ok(PathBuf::from(if pid % 2 == 0 {
                "/silo/ws-main"
            } else {
                "/silo/ws-detached"
            }))
        });
        PsCommand::new(AgentListService::new(workspaces, process))
    }

    /// Sample running agents covering known, unknown and detached cases.
    fn sample_agents() -> Vec<RunningAgent> {
        vec![
            RunningAgent {
                pid: 42,
                agent_type: Some(Agent::Codex),
                path: PathBuf::from("/silo/ws-main"),
                branch: Some("main".to_string()),
            },
            RunningAgent {
                pid: 43,
                agent_type: None,
                path: PathBuf::from("/silo/ws-detached"),
                branch: None,
            },
        ]
    }

    #[test]
    fn plain_output_lists_agents() {
        // Given
        let cmd = command(vec![(10, "claude --resume"), (11, "/usr/bin/mystery")]);
        let mut out = Vec::new();
        cmd.write_plain(&mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        // When
        let actual_1 = text.starts_with("PID");
        let actual_2 = text.contains("claude");
        let actual_3 = text.contains("/silo/ws-main");
        let actual_4 = text.contains("(unknown)");
        let actual_5 = text.contains("(detached)");
        // Then
        assert!(actual_1);
        assert!(actual_2);
        assert!(actual_3);
        assert!(actual_4);
        assert!(actual_5);
    }

    #[test]
    fn plain_output_without_agents() {
        // Given
        let mut out = Vec::new();
        // When
        command(vec![]).write_plain(&mut out).unwrap();
        // Then
        assert!(
            String::from_utf8(out)
                .unwrap()
                .contains("No running agents")
        );
    }

    #[test]
    fn quit_keys_end_the_dashboard() {
        // Given
        let cmd = command(vec![]);
        let mut agents = Vec::new();
        let events = [key(KeyCode::Char('q')), key(KeyCode::Esc), ctrl('c')];

        // When
        let ticks: Vec<Tick> = events
            .into_iter()
            .map(|event| cmd.handle_tick(Some(event), &mut agents, 8))
            .collect();

        // Then
        assert_eq!(ticks, vec![Tick::Quit, Tick::Quit, Tick::Quit]);
    }

    #[test]
    fn other_events_keep_running_without_refresh() {
        // Given
        let cmd = command(vec![(10, "claude")]);
        let mut agents = Vec::new();
        // When
        let actual_1 = cmd.handle_tick(Some(key(KeyCode::Char('x'))), &mut agents, 8);
        let actual_2 = cmd.handle_tick(Some(Event::FocusGained), &mut agents, 8);
        let actual_3 = agents.is_empty();
        // Then
        assert_eq!(actual_1, Tick::Continue);
        assert_eq!(actual_2, Tick::Continue);
        assert!(actual_3);
    }

    #[test]
    fn timeout_refreshes_agents_and_grows_viewport() {
        // Given
        let cmd = command(vec![(10, "claude"), (11, "codex")]);
        let mut agents = Vec::new();
        // When
        let actual_1 = cmd.handle_tick(None, &mut agents, 8);
        let actual_2 = agents.len();
        let actual_3 = cmd.handle_tick(None, &mut agents, 10);
        // Then
        assert_eq!(actual_1, Tick::Grow(10));
        assert_eq!(actual_2, 2);
        assert_eq!(actual_3, Tick::Continue);
    }

    #[test]
    fn viewport_height_is_clamped() {
        // Given
        // (no setup needed)
        // When
        let actual_1 = viewport_height(0);
        let actual_2 = viewport_height(3);
        let actual_3 = viewport_height(100);
        // Then
        assert_eq!(actual_1, 8);
        assert_eq!(actual_2, 11);
        assert_eq!(actual_3, 23);
    }

    #[test]
    fn agent_colors_are_distinct() {
        // Given
        let colors: std::collections::HashSet<_> = [
            Agent::ClaudeCode,
            Agent::Codex,
            Agent::Gemini,
            Agent::OpenCode,
            Agent::Droid,
        ]
        .iter()
        .map(|a| format!("{:?}", agent_color(a)))
        .collect();
        // When
        let actual = colors.len();
        // Then
        assert_eq!(actual, 5);
    }

    #[test]
    fn dashboard_renders_agents() {
        // Given
        let mut terminal = test_terminal(100, 10);
        let agents = sample_agents();
        terminal
            .draw(|f| draw_ps(f, &agents, &mut TableState::default()))
            .unwrap();
        // When
        let screen = screen_text(&terminal);
        // Then
        assert!(screen.contains("Running Agents (2)"));
        assert!(screen.contains("codex"));
        assert!(screen.contains("(unknown)"));
        assert!(screen.contains("(detached)"));
        assert!(screen.contains("/silo/ws-main"));
        assert!(screen.contains("q quit"));
    }

    #[test]
    fn dashboard_renders_empty_state() {
        // Given
        let mut terminal = test_terminal(100, 8);
        // When
        terminal
            .draw(|f| draw_ps(f, &[], &mut TableState::default()))
            .unwrap();
        // Then
        assert!(screen_text(&terminal).contains("No running agents found"));
    }
}
