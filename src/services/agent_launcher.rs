use crate::infra::agent::Agent;
use crate::infra::terminal::Terminal;
use crate::services::agent_workspace::WorkspaceFactory;

pub use crate::infra::terminal::LaunchError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchMode {
    ExecReplace,
    NewTab,
    SplitPane,
}

pub struct AgentLauncher<W, T>
where
    W: WorkspaceFactory,
    T: Terminal,
{
    workspace: W,
    terminal: Option<T>,
    launch_mode: LaunchMode,
    agent: Agent,
    branch: Option<String>,
}

impl<W, T> AgentLauncher<W, T>
where
    W: WorkspaceFactory,
    T: Terminal,
{
    pub fn new(
        workspace: W,
        terminal: Option<T>,
        launch_mode: LaunchMode,
        agent: Agent,
        branch: Option<String>,
    ) -> Self {
        Self {
            workspace,
            terminal,
            launch_mode,
            agent,
            branch,
        }
    }

    fn launch_in_workspace(&self, workspace_path: &std::path::Path) -> Result<(), LaunchError> {
        let _status = self
            .agent
            .command()
            .current_dir(workspace_path)
            .status()
            .map_err(|e| LaunchError::AgentSpawnError(e.to_string()))?;
        Ok(())
    }

    pub fn launch(&self) -> Result<std::path::PathBuf, LaunchError> {
        // Step 1: Create workspace
        let workspace_path = self.workspace.create(self.branch.clone())?;

        // Step 2: Launch agent in the workspace
        match self.launch_mode {
            LaunchMode::ExecReplace => self.launch_in_workspace(&workspace_path)?,
            LaunchMode::NewTab => {
                let terminal = self.terminal.as_ref().ok_or_else(|| {
                    LaunchError::AgentSpawnError("no terminal provided for new tab".to_string())
                })?;
                match terminal.open_tab(&workspace_path, &self.agent) {
                    Ok(()) => {}
                    Err(_e) => self.launch_in_workspace(&workspace_path)?,
                }
            }
            LaunchMode::SplitPane => {
                let terminal = self.terminal.as_ref().ok_or_else(|| {
                    LaunchError::AgentSpawnError("no terminal provided for split pane".to_string())
                })?;
                match terminal.split_pane(&workspace_path, &self.agent) {
                    Ok(()) => {}
                    Err(_e) => self.launch_in_workspace(&workspace_path)?,
                }
            }
        }

        Ok(workspace_path)
    }
}
