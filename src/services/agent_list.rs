use std::path::PathBuf;

use crate::infra::agent::Agent;
use crate::infra::git::GitOperations;
use crate::infra::git_error::GitError;
use crate::infra::process::{ProcessError, ProcessOperations};
use crate::services::agent_workspace::WorkspaceManager;
use crate::services::global_workspace::GlobalWorkspaceManager;
use strum::IntoEnumIterator;

#[derive(Debug)]
pub struct RunningAgent {
    pub pid: u32,
    pub agent_type: Option<Agent>,
    pub path: PathBuf,
    pub branch: Option<String>,
}

#[derive(Debug)]
pub enum ListError {
    Git(GitError),
    Process(ProcessError),
}

impl std::fmt::Display for ListError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ListError::Git(e) => write!(f, "Git error: {}", e),
            ListError::Process(e) => write!(f, "Process error: {}", e),
        }
    }
}

impl std::error::Error for ListError {}

impl From<GitError> for ListError {
    fn from(error: GitError) -> Self {
        ListError::Git(error)
    }
}

impl From<ProcessError> for ListError {
    fn from(error: ProcessError) -> Self {
        ListError::Process(error)
    }
}

pub struct AgentListService<G: GitOperations, P: ProcessOperations> {
    workspace_manager: GlobalWorkspaceManager<G>,
    process: P,
}

impl<G: GitOperations, P: ProcessOperations> AgentListService<G, P> {
    pub fn new(workspace_manager: GlobalWorkspaceManager<G>, process: P) -> Self {
        Self {
            workspace_manager,
            process,
        }
    }

    pub fn list_running_agents(&self) -> Result<Vec<RunningAgent>, ListError> {
        let workspaces = self.workspace_manager.get_all()?;
        let processes = self
            .process
            .find_processes_by_names(&Agent::all_process_names())?;
        let mut agents = Vec::new();
        for (pid, args) in processes {
            let cwd = match self.process.get_process_cwd(pid) {
                Ok(path) => path,
                Err(_) => continue,
            };

            for workspace in &workspaces {
                if cwd.starts_with(&workspace.path) {
                    let agent_type = extract_agent_type(&args);

                    // Deduplicate: skip if we already have an agent of this type in this workspace
                    if agents.iter().any(|a: &RunningAgent| {
                        a.path == workspace.path && a.agent_type == agent_type
                    }) {
                        break;
                    }

                    agents.push(RunningAgent {
                        pid,
                        agent_type,
                        path: workspace.path.clone(),
                        branch: workspace.branch.clone(),
                    });
                    break;
                }
            }
        }

        Ok(agents)
    }

    pub fn get_active_paths(&self) -> Result<Vec<PathBuf>, ListError> {
        let running_agents = self.list_running_agents()?;

        let active_paths: Vec<PathBuf> =
            running_agents.into_iter().map(|agent| agent.path).collect();

        Ok(active_paths)
    }
}

fn extract_agent_type(args: &str) -> Option<Agent> {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if let Some(first) = parts.first()
        && let Some(name) = first.split('/').next_back()
        && let Some(agent) = Agent::try_from_process_name(name)
    {
        return Some(agent);
    }

    Agent::iter().find(|agent| args.contains(agent.process_name()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::git::{GitWorkspaceInfo, MockGitOperations};
    use crate::services::git_checkout_workspace::GitCheckoutWorkspace;
    use crate::services::git_worktree_workspace::GitWorktreeWorkspace;

    // Mock ProcessOperations
    struct MockProcess {
        processes: Vec<(u32, String)>,
        cwds: Vec<(u32, PathBuf)>,
    }

    impl ProcessOperations for MockProcess {
        fn find_processes_by_names(
            &self,
            _names: &[&str],
        ) -> Result<Vec<(u32, String)>, ProcessError> {
            Ok(self.processes.clone())
        }

        fn get_process_cwd(&self, pid: u32) -> Result<PathBuf, ProcessError> {
            self.cwds
                .iter()
                .find(|(p, _)| *p == pid)
                .map(|(_, path)| path.clone())
                .ok_or_else(|| ProcessError::CommandFailed("Process not found".to_string()))
        }
    }

    #[test]
    fn test_list_running_agents_in_worktrees() {
        let mut worktree_mock = MockGitOperations::new();
        worktree_mock.expect_list_worktrees().return_once(|| {
            Ok(vec![
                GitWorkspaceInfo {
                    path: PathBuf::from("/repo/worktree1"),
                    branch: Some("feature-1".to_string()),
                },
                GitWorkspaceInfo {
                    path: PathBuf::from("/repo/worktree2"),
                    branch: Some("feature-2".to_string()),
                },
            ])
        });
        let mut checkout_mock = MockGitOperations::new();
        checkout_mock
            .expect_get_project_name()
            .returning(|| Ok("test-project".to_string()));
        let workspace_manager = GlobalWorkspaceManager::new(
            GitWorktreeWorkspace::new(worktree_mock),
            GitCheckoutWorkspace::new(checkout_mock),
        );

        let mock_process = MockProcess {
            processes: vec![
                (123, "/usr/bin/claude --args".to_string()),
                (456, "/usr/bin/claude --other".to_string()),
            ],
            cwds: vec![
                (123, PathBuf::from("/repo/worktree1")),
                (456, PathBuf::from("/repo/worktree2")),
            ],
        };

        let service = AgentListService::new(workspace_manager, mock_process);
        let agents = service.list_running_agents().unwrap();

        assert_eq!(agents.len(), 2);
        assert_eq!(agents[0].pid, 123);
        assert_eq!(agents[0].path, PathBuf::from("/repo/worktree1"));
        assert_eq!(agents[0].branch, Some("feature-1".to_string()));
        assert_eq!(agents[1].pid, 456);
        assert_eq!(agents[1].path, PathBuf::from("/repo/worktree2"));
        assert_eq!(agents[1].branch, Some("feature-2".to_string()));
    }

    #[test]
    fn test_list_running_agents_outside_worktrees() {
        let mut worktree_mock = MockGitOperations::new();
        worktree_mock.expect_list_worktrees().return_once(|| {
            Ok(vec![GitWorkspaceInfo {
                path: PathBuf::from("/repo/worktree1"),
                branch: Some("feature-1".to_string()),
            }])
        });
        let mut checkout_mock = MockGitOperations::new();
        checkout_mock
            .expect_get_project_name()
            .returning(|| Ok("test-project".to_string()));
        let workspace_manager = GlobalWorkspaceManager::new(
            GitWorktreeWorkspace::new(worktree_mock),
            GitCheckoutWorkspace::new(checkout_mock),
        );

        let mock_process = MockProcess {
            processes: vec![(123, "/usr/bin/claude --args".to_string())],
            cwds: vec![(123, PathBuf::from("/other/directory"))],
        };

        let service = AgentListService::new(workspace_manager, mock_process);
        let agents = service.list_running_agents().unwrap();

        assert_eq!(agents.len(), 0);
    }

    #[test]
    fn test_list_running_agents_cwd_resolution_failure() {
        let mut worktree_mock = MockGitOperations::new();
        worktree_mock.expect_list_worktrees().return_once(|| {
            Ok(vec![GitWorkspaceInfo {
                path: PathBuf::from("/repo/worktree1"),
                branch: Some("feature-1".to_string()),
            }])
        });
        let mut checkout_mock = MockGitOperations::new();
        checkout_mock
            .expect_get_project_name()
            .returning(|| Ok("test-project".to_string()));
        let workspace_manager = GlobalWorkspaceManager::new(
            GitWorktreeWorkspace::new(worktree_mock),
            GitCheckoutWorkspace::new(checkout_mock),
        );

        let mock_process = MockProcess {
            processes: vec![
                (123, "/usr/bin/claude --args".to_string()),
                (456, "/usr/bin/claude --other".to_string()),
            ],
            cwds: vec![(123, PathBuf::from("/repo/worktree1"))],
            // PID 456 doesn't have a CWD entry, simulating a failure
        };

        let service = AgentListService::new(workspace_manager, mock_process);
        let agents = service.list_running_agents().unwrap();

        // Should only find the agent with resolvable CWD
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].pid, 123);
    }

    #[test]
    fn test_list_running_agents_empty_cases() {
        // No worktrees
        let mut worktree_mock = MockGitOperations::new();
        worktree_mock
            .expect_list_worktrees()
            .return_once(|| Ok(vec![]));
        let mut checkout_mock = MockGitOperations::new();
        checkout_mock
            .expect_get_project_name()
            .returning(|| Ok("test-project".to_string()));
        let workspace_manager = GlobalWorkspaceManager::new(
            GitWorktreeWorkspace::new(worktree_mock),
            GitCheckoutWorkspace::new(checkout_mock),
        );
        let mock_process = MockProcess {
            processes: vec![(123, "/usr/bin/claude --args".to_string())],
            cwds: vec![(123, PathBuf::from("/repo/worktree1"))],
        };
        let service = AgentListService::new(workspace_manager, mock_process);
        let agents = service.list_running_agents().unwrap();
        assert_eq!(agents.len(), 0);

        // No processes
        let mut worktree_mock = MockGitOperations::new();
        worktree_mock.expect_list_worktrees().return_once(|| {
            Ok(vec![GitWorkspaceInfo {
                path: PathBuf::from("/repo/worktree1"),
                branch: Some("feature-1".to_string()),
            }])
        });
        let mut checkout_mock = MockGitOperations::new();
        checkout_mock
            .expect_get_project_name()
            .returning(|| Ok("test-project".to_string()));
        let workspace_manager = GlobalWorkspaceManager::new(
            GitWorktreeWorkspace::new(worktree_mock),
            GitCheckoutWorkspace::new(checkout_mock),
        );
        let mock_process = MockProcess {
            processes: vec![],
            cwds: vec![],
        };
        let service = AgentListService::new(workspace_manager, mock_process);
        let agents = service.list_running_agents().unwrap();
        assert_eq!(agents.len(), 0);
    }
}
