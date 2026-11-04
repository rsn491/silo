//! Logic for the `init` command.

use clap::Parser;
use std::io::{self, IsTerminal};
use std::path::Path;

use crossterm::style::Color;

use crate::infra::agent::Agent;
use crate::services::silo_config::SiloConfig;
use crate::services::workspace_kind::WorkspaceKind;
use crate::tui::{Prompt, SelectItem, print_status};

/// Arguments for the `init` command.
#[derive(Parser, Debug)]
pub struct InitArgs {
    /// Default agent to use (skips interactive prompt).
    #[arg(long)]
    pub agent: Option<Agent>,

    /// Default workspace type: worktree (default) or checkout.
    #[arg(long, value_name = "TYPE")]
    pub workspace_type: Option<WorkspaceKind>,

    /// Enable commit+push prompts after agent exits (default: true).
    #[arg(long, value_name = "BOOL")]
    pub exit_work: Option<bool>,
}

/// Handler for the `init` command.
pub struct InitCommand<U: Prompt> {
    /// Interactive prompt used to choose settings.
    prompt: U,
}

impl<U: Prompt> InitCommand<U> {
    /// Creates a new `InitCommand`.
    pub fn new(prompt: U) -> Self {
        Self { prompt }
    }

    /// Executes the initialization process.
    ///
    /// # Errors
    ///
    /// Returns an error if directory creation or configuration saving fails.
    pub fn run(&self, args: InitArgs) -> Result<(), Box<dyn std::error::Error>> {
        match SiloConfig::initialize() {
            Ok(path) => {
                print_status(
                    "✓",
                    Color::Green,
                    "Silo directory initialized successfully.",
                )?;
                print_status(
                    "→",
                    Color::Cyan,
                    &format!("Future worktrees will be created in: {}", path.display()),
                )?;
                print_status(
                    "→",
                    Color::DarkGrey,
                    "Run 'silo launch' to create worktrees.",
                )?;
            }
            Err(e) => {
                print_status(
                    "✗",
                    Color::Red,
                    &format!("Error initializing silo directory: {}", e),
                )?;
                std::process::exit(1);
            }
        }

        let settings_path = SiloConfig::get_settings_path()?;
        self.configure(args, io::stdin().is_terminal(), &settings_path)
    }

    /// Updates the settings file at `settings_path` from `args`, prompting for anything
    /// not given on the command line when `is_tty` is set.
    ///
    /// # Errors
    ///
    /// Returns an error if a prompt fails or the settings cannot be saved.
    fn configure(
        &self,
        args: InitArgs,
        is_tty: bool,
        settings_path: &Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !is_tty
            && args.agent.is_none()
            && args.workspace_type.is_none()
            && args.exit_work.is_none()
        {
            print_status(
                "–",
                Color::DarkGrey,
                "Non-interactive init; skipping settings.json.",
            )?;
            print_status(
                "→",
                Color::DarkGrey,
                "Use `silo init --agent <name>` to set the default agent.",
            )?;
            return Ok(());
        }

        // Load existing settings so we only update what the user specifies.
        let mut settings = SiloConfig::load_settings_from_path(settings_path).unwrap_or_default();

        let agent = if let Some(agent) = args.agent {
            Some(agent)
        } else if is_tty {
            prompt_for_agent(&self.prompt)?
        } else {
            None
        };

        let workspace_type = if let Some(wt) = args.workspace_type {
            Some(wt)
        } else if is_tty {
            prompt_for_workspace_type(&self.prompt)?
        } else {
            None
        };

        let exit_work = if let Some(ew) = args.exit_work {
            Some(ew)
        } else if is_tty {
            prompt_for_exit_work(&self.prompt)?
        } else {
            None
        };

        if agent.is_none() && workspace_type.is_none() && exit_work.is_none() {
            print_status(
                "–",
                Color::DarkGrey,
                "No settings selected; skipping settings.json.",
            )?;
            return Ok(());
        }

        if settings_path.exists() && is_tty && !confirm_overwrite(&self.prompt)? {
            print_status(
                "–",
                Color::DarkGrey,
                "Existing settings.json left unchanged.",
            )?;
            return Ok(());
        }

        if let Some(agent) = agent {
            settings.agent = Some(agent);
        }
        if let Some(wt) = workspace_type {
            settings.workspace_type = Some(wt);
        }
        if let Some(ew) = exit_work {
            settings.exit_work = Some(ew);
        }

        SiloConfig::save_settings_to_path(settings_path, &settings)?;
        print_status(
            "✓",
            Color::Green,
            &format!("Saved settings to {}", settings_path.display()),
        )?;

        Ok(())
    }
}

/// Prompts the user to choose a default AI agent.
fn prompt_for_agent(prompt: &impl Prompt) -> Result<Option<Agent>, Box<dyn std::error::Error>> {
    let names = Agent::all_names();
    let mut items: Vec<SelectItem> = names
        .iter()
        .map(|n| SelectItem {
            label: n.to_string(),
            detail: None,
        })
        .collect();
    items.push(SelectItem {
        label: "Skip".to_string(),
        detail: Some("Leave agent setting unchanged".to_string()),
    });

    match prompt.select("Choose default agent", &items, 0)? {
        Some(i) if i < names.len() => Ok(Agent::try_from_str(names[i]).map(Some).unwrap_or(None)),
        _ => Ok(None),
    }
}

/// Prompts the user to choose a default workspace type.
fn prompt_for_workspace_type(
    prompt: &impl Prompt,
) -> Result<Option<WorkspaceKind>, Box<dyn std::error::Error>> {
    let items = vec![
        SelectItem {
            label: "worktree (default)".to_string(),
            detail: Some("Faster; shares git objects with the main repo".to_string()),
        },
        SelectItem {
            label: "clone".to_string(),
            detail: Some("Fully isolated; uses more disk space".to_string()),
        },
        SelectItem {
            label: "Skip".to_string(),
            detail: Some("Leave workspace type setting unchanged".to_string()),
        },
    ];

    match prompt.select("Choose default workspace type", &items, 0)? {
        Some(0) => Ok(Some(WorkspaceKind::Worktree)),
        Some(1) => Ok(Some(WorkspaceKind::Clone)),
        _ => Ok(None),
    }
}

/// Prompts the user to enable or disable commit+push prompts on agent exit.
fn prompt_for_exit_work(prompt: &impl Prompt) -> Result<Option<bool>, Box<dyn std::error::Error>> {
    let items = vec![
        SelectItem {
            label: "Yes (default)".to_string(),
            detail: Some("Prompt to commit and push when the agent exits".to_string()),
        },
        SelectItem {
            label: "No".to_string(),
            detail: Some("Skip commit/push prompts on agent exit".to_string()),
        },
        SelectItem {
            label: "Skip".to_string(),
            detail: Some("Leave this setting unchanged".to_string()),
        },
    ];

    match prompt.select("Enable commit+push prompts on agent exit?", &items, 0)? {
        Some(0) => Ok(Some(true)),
        Some(1) => Ok(Some(false)),
        _ => Ok(None),
    }
}

/// Asks the user to confirm overwriting the existing `settings.json` file.
fn confirm_overwrite(prompt: &impl Prompt) -> Result<bool, Box<dyn std::error::Error>> {
    prompt.confirm("settings.json already exists. Overwrite?", false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::silo_config::SiloSettings;
    use crate::tui::prompt::MockPrompt;
    use mockall::Sequence;
    use tempfile::TempDir;

    /// Args with nothing set.
    fn no_args() -> InitArgs {
        InitArgs {
            agent: None,
            workspace_type: None,
            exit_work: None,
        }
    }

    /// A temp dir and the path of a (not yet existing) settings file inside it.
    fn settings_file() -> (TempDir, std::path::PathBuf) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("silo").join("settings.json");
        (dir, path)
    }

    /// Reads the settings file back.
    fn read(path: &Path) -> SiloSettings {
        SiloConfig::load_settings_from_path(path).unwrap()
    }

    /// A prompt that answers the three select prompts with `answers` in order.
    fn selecting(answers: [Option<usize>; 3]) -> MockPrompt {
        let mut prompt = MockPrompt::new();
        let mut seq = Sequence::new();
        for answer in answers {
            prompt
                .expect_select()
                .times(1)
                .in_sequence(&mut seq)
                .returning(move |_, _, _| Ok(answer));
        }
        prompt
    }

    #[test]
    fn non_interactive_without_args_skips_settings() {
        let (_dir, path) = settings_file();

        InitCommand::new(MockPrompt::new())
            .configure(no_args(), false, &path)
            .unwrap();

        assert!(!path.exists());
    }

    #[test]
    fn non_interactive_args_are_saved_and_merged() {
        let (_dir, path) = settings_file();
        SiloConfig::save_settings_to_path(
            &path,
            &SiloSettings {
                agent: Some(Agent::Gemini),
                workspace_type: None,
                exit_work: Some(true),
            },
        )
        .unwrap();
        let args = InitArgs {
            workspace_type: Some(WorkspaceKind::Clone),
            exit_work: Some(false),
            ..no_args()
        };

        InitCommand::new(MockPrompt::new())
            .configure(args, false, &path)
            .unwrap();

        let settings = read(&path);
        assert_eq!(settings.agent, Some(Agent::Gemini));
        assert_eq!(settings.workspace_type, Some(WorkspaceKind::Clone));
        assert_eq!(settings.exit_work, Some(false));
    }

    #[test]
    fn interactive_prompts_choose_every_setting() {
        let (_dir, path) = settings_file();
        let codex_index = Agent::all_names()
            .iter()
            .position(|n| *n == "codex")
            .unwrap();
        let prompt = selecting([Some(codex_index), Some(1), Some(1)]);

        InitCommand::new(prompt)
            .configure(no_args(), true, &path)
            .unwrap();

        let settings = read(&path);
        assert_eq!(settings.agent, Some(Agent::Codex));
        assert_eq!(settings.workspace_type, Some(WorkspaceKind::Clone));
        assert_eq!(settings.exit_work, Some(false));
    }

    #[test]
    fn interactive_default_choices() {
        let (_dir, path) = settings_file();
        let prompt = selecting([Some(0), Some(0), Some(0)]);

        InitCommand::new(prompt)
            .configure(no_args(), true, &path)
            .unwrap();

        let settings = read(&path);
        assert_eq!(settings.agent, Some(Agent::ClaudeCode));
        assert_eq!(settings.workspace_type, Some(WorkspaceKind::Worktree));
        assert_eq!(settings.exit_work, Some(true));
    }

    #[test]
    fn interactive_skip_everything_writes_nothing() {
        let (_dir, path) = settings_file();
        let skip_agent = Agent::all_names().len();
        let prompt = selecting([Some(skip_agent), Some(2), None]);

        InitCommand::new(prompt)
            .configure(no_args(), true, &path)
            .unwrap();

        assert!(!path.exists());
    }

    #[test]
    fn interactive_existing_file_requires_confirmation() {
        let (_dir, path) = settings_file();
        SiloConfig::save_settings_to_path(&path, &SiloSettings::default()).unwrap();
        let args = || InitArgs {
            agent: Some(Agent::Droid),
            workspace_type: Some(WorkspaceKind::Worktree),
            exit_work: Some(true),
        };

        let mut declining = MockPrompt::new();
        declining
            .expect_confirm()
            .times(1)
            .returning(|_, default_yes| {
                assert!(!default_yes);
                Ok(false)
            });
        InitCommand::new(declining)
            .configure(args(), true, &path)
            .unwrap();
        assert_eq!(read(&path).agent, None);

        let mut accepting = MockPrompt::new();
        accepting
            .expect_confirm()
            .times(1)
            .returning(|_, _| Ok(true));
        InitCommand::new(accepting)
            .configure(args(), true, &path)
            .unwrap();
        assert_eq!(read(&path).agent, Some(Agent::Droid));
    }

    #[test]
    fn prompt_errors_are_propagated() {
        let (_dir, path) = settings_file();
        let mut prompt = MockPrompt::new();
        prompt
            .expect_select()
            .returning(|_, _, _| Err("terminal gone".into()));

        let result = InitCommand::new(prompt).configure(no_args(), true, &path);

        assert_eq!(result.unwrap_err().to_string(), "terminal gone");
    }

    #[test]
    fn run_initializes_silo_directory_non_interactively() {
        // Without args this never writes settings: non-interactively it skips them, and
        // when the test runs with a TTY on stdin every prompt answers "skip".
        let mut prompt = MockPrompt::new();
        prompt.expect_select().returning(|_, _, _| Ok(None));
        let settings_path = SiloConfig::get_settings_path().unwrap();
        let before = std::fs::read(&settings_path).ok();

        let result = InitCommand::new(prompt).run(no_args());

        assert!(result.is_ok());
        assert_eq!(std::fs::read(&settings_path).ok(), before);
    }
}
