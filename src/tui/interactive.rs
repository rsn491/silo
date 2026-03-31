//! Inline interactive agent launcher TUI built on ratatui.

use std::io;

use ratatui::crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyModifiers, KeyboardEnhancementFlags,
    PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
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
    /// Currently selected agent.
    agent: Agent,
    /// All agents available for cycling.
    agents: Vec<Agent>,
    /// Index into `agents` pointing at the current agent.
    agent_index: usize,
    /// Currently selected interaction mode.
    mode: AgentMode,
    /// The text the user has typed (supports newlines via Shift+Enter).
    prompt: String,
    /// Logical cursor position measured in Unicode scalar values.
    cursor: usize,
    /// When `Some`, the event loop should stop with this outcome.
    outcome: Option<AppOutcome>,
}

impl App {
    /// Creates a new `App` starting with the given agent selected.
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

    /// Advances to the next agent in the cycle.
    fn next_agent(&mut self) {
        self.agent_index = (self.agent_index + 1) % self.agents.len();
        self.agent = self.agents[self.agent_index].clone();
    }

    /// Flips the interaction mode between Code and Plan.
    fn toggle_mode(&mut self) {
        self.mode = self.mode.toggle();
    }

    /// Inserts `c` at the current cursor position and advances the cursor.
    fn insert_char(&mut self, c: char) {
        let byte_pos = self.char_to_byte_pos(self.cursor);
        self.prompt.insert(byte_pos, c);
        self.cursor += 1;
    }

    /// Deletes the character immediately before the cursor (backspace behaviour).
    fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let byte_pos = self.char_to_byte_pos(self.cursor);
        // Walk backwards one char in the UTF-8 string.
        if let Some((char_start, _)) = self.prompt[..byte_pos].char_indices().next_back() {
            self.prompt.drain(char_start..byte_pos);
            self.cursor -= 1;
        }
    }

    /// Moves the cursor one character to the left.
    fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    /// Moves the cursor one character to the right.
    fn move_right(&mut self) {
        if self.cursor < self.prompt.chars().count() {
            self.cursor += 1;
        }
    }

    /// Converts a char-offset into the corresponding byte offset within `prompt`.
    fn char_to_byte_pos(&self, char_pos: usize) -> usize {
        self.prompt
            .char_indices()
            .nth(char_pos)
            .map(|(i, _)| i)
            .unwrap_or(self.prompt.len())
    }

    /// Signals that the agent should be launched when the loop exits.
    fn launch(&mut self) {
        self.outcome = Some(AppOutcome::Launch {
            agent: self.agent.clone(),
            mode: self.mode,
            prompt: self.prompt.clone(),
        });
    }

    /// Signals that the loop should exit without launching anything.
    fn quit(&mut self) {
        self.outcome = Some(AppOutcome::Quit);
    }

    /// Returns `true` once an outcome has been decided.
    fn should_exit(&self) -> bool {
        self.outcome.is_some()
    }

    /// Dispatches a key event to the appropriate state mutation.
    fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Tab => self.toggle_mode(),
            KeyCode::Char('.') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.next_agent();
            }
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
                self.insert_char('\n');
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

/// Renders the full TUI layout into `frame`.
fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // title
            Constraint::Length(3), // agent + mode status
            Constraint::Min(5),    // prompt input
            Constraint::Length(3), // keybinding help bar
        ])
        .split(area);

    // ── Title ──────────────────────────────────────────────────────────────
    let title = Paragraph::new("silo — interactive agent launcher")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(title, chunks[0]);

    // ── Status: agent + mode ───────────────────────────────────────────────
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

    // ── Prompt input ───────────────────────────────────────────────────────
    let prompt_chars: Vec<char> = app.prompt.chars().collect();
    // Split prompt into lines, tracking which line/col the cursor is on.
    let mut lines: Vec<Line> = Vec::new();
    let mut line_start = 0usize; // char index of the start of the current line
    let mut cursor_placed = false;
    for (i, &ch) in prompt_chars.iter().enumerate() {
        if ch == '\n' {
            // Build the line up to (but not including) the newline.
            let before: String = prompt_chars[line_start..i.min(app.cursor)].iter().collect();
            if app.cursor == i && !cursor_placed {
                // Cursor sits on the newline character itself.
                let after: String = String::new();
                lines.push(Line::from(vec![
                    Span::raw(before),
                    Span::styled(" ", Style::default().bg(Color::White).fg(Color::Black)),
                    Span::raw(after),
                ]));
                cursor_placed = true;
            } else if app.cursor > line_start && app.cursor < i && !cursor_placed {
                let before2: String = prompt_chars[line_start..app.cursor].iter().collect();
                let at: String = prompt_chars[app.cursor].to_string();
                let after: String = prompt_chars[app.cursor + 1..i].iter().collect();
                lines.push(Line::from(vec![
                    Span::raw(before2),
                    Span::styled(at, Style::default().bg(Color::White).fg(Color::Black)),
                    Span::raw(after),
                ]));
                cursor_placed = true;
            } else {
                let content: String = prompt_chars[line_start..i].iter().collect();
                lines.push(Line::from(Span::raw(content)));
            }
            line_start = i + 1;
        }
    }
    // Last (or only) line.
    if !cursor_placed {
        let before: String = prompt_chars[line_start..app.cursor].iter().collect();
        let at: String = prompt_chars
            .get(app.cursor)
            .map(|c| c.to_string())
            .unwrap_or_else(|| " ".to_string());
        let after: String = if app.cursor < prompt_chars.len() {
            prompt_chars[app.cursor + 1..].iter().collect()
        } else {
            String::new()
        };
        lines.push(Line::from(vec![
            Span::raw(before),
            Span::styled(at, Style::default().bg(Color::White).fg(Color::Black)),
            Span::raw(after),
        ]));
    } else {
        let content: String = prompt_chars[line_start..].iter().collect();
        if !content.is_empty() {
            lines.push(Line::from(Span::raw(content)));
        }
    }
    // Prepend the ">" prefix to the first line.
    if let Some(first) = lines.first_mut() {
        let mut spans = vec![Span::raw("> ")];
        spans.append(&mut first.spans);
        *first = Line::from(spans);
    }
    let prompt_widget =
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("Prompt"));
    frame.render_widget(prompt_widget, chunks[2]);

    // ── Help bar ───────────────────────────────────────────────────────────
    let help_line = Line::from(vec![
        Span::styled(" [Tab]", Style::default().fg(Color::Yellow)),
        Span::raw(" mode  "),
        Span::styled("[Ctrl+.]", Style::default().fg(Color::Yellow)),
        Span::raw(" agent  "),
        Span::styled("[Shift+Enter]", Style::default().fg(Color::Yellow)),
        Span::raw(" newline  "),
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
/// The terminal is always restored to its original state before this function
/// returns, even when an error occurs.
///
/// # Errors
///
/// Returns an [`io::Error`] if the terminal cannot be initialised or if a
/// read/write error occurs during event processing.
pub fn run(default_agent: Agent) -> io::Result<AppOutcome> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    // Enable the kitty keyboard protocol so terminals report Shift+Enter as a
    // distinct event from plain Enter.  Pushed *after* EnterAlternateScreen
    // because some terminals (e.g. Ghostty) reset keyboard state on screen
    // switch.  Terminals that don't support the protocol silently ignore the
    // escape sequence.
    let _ = execute!(
        stdout,
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    );
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(default_agent);
    let loop_result = run_event_loop(&mut terminal, &mut app);

    // Restore terminal regardless of whether the loop succeeded.
    let _ = disable_raw_mode();
    // Pop keyboard enhancement before leaving alternate screen (reverse order).
    let _ = execute!(terminal.backend_mut(), PopKeyboardEnhancementFlags);
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    loop_result?;

    Ok(app.outcome.unwrap_or(AppOutcome::Quit))
}

/// The inner event loop: draws the frame then blocks until a key arrives.
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
