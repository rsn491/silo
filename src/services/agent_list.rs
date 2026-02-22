use std::path::PathBuf;

use crate::infra::agent::Agent;
use crate::infra::git::GitOperations;
use crate::infra::git_error::GitError;
use crate::infra::process::{ProcessError, ProcessOperations};
use crate::services::agent_workspace::AgentWorkspaceManager;
use crate::services::git_checkout_workspace::GitCheckoutWorkspace;
use crate::services::git_worktree_workspace::GitWorktreeWorkspace;
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

pub struct AgentListService<G: GitOperations + Clone, P: ProcessOperations> {
    git: G,
    process: P,
}

impl<G: GitOperations + Clone, P: ProcessOperations> AgentListService<G, P> {
    pub fn new(git: G, process: P) -> Self {
        Self { git, process }
    }

    pub fn list_running_agents(&self) -> Result<Vec<RunningAgent>, ListError> {
        let worktrees = GitWorktreeWorkspace::new(self.git.clone(), None).get_all()?;
        let checkouts = GitCheckoutWorkspace::new(self.git.clone()).get_all()?;
        let workspaces: Vec<_> = worktrees
            .iter()
            .map(|w| (&w.path, &w.branch))
            .chain(checkouts.iter().map(|c| (&c.path, &c.branch)))
            .collect();

        let processes = self
            .process
            .find_processes_by_names(&Agent::all_process_names())?;
        let mut agents = Vec::new();
        for (pid, args) in processes {
            let cwd = match self.process.get_process_cwd(pid) {
                Ok(path) => path,
                Err(_) => continue,
            };

            for (path, branch) in &workspaces {
                if cwd.starts_with(path) {
                    let agent_type = extract_agent_type(&args);

                    // Deduplicate: skip if we already have an agent of this type in this workspace
                    if agents
                        .iter()
                        .any(|a: &RunningAgent| a.path == **path && a.agent_type == agent_type)
                    {
                        break;
                    }

                    agents.push(RunningAgent {
                        pid,
                        agent_type,
                        path: path.to_path_buf(),
                        branch: (*branch).clone(),
                    });
                    break;
                }
            }
        }

        Ok(agents)
    }

    pub fn get_active_worktree_paths(&self) -> Result<Vec<PathBuf>, ListError> {
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
    use crate::infra::git::GitWorkspaceInfo;
    use std::path::Path;

    // Mock GitOperations
    #[derive(Clone)]
    struct MockGit {
        worktrees: Vec<GitWorkspaceInfo>,
    }

    impl GitOperations for MockGit {
        fn get_repo_root(&self) -> Result<PathBuf, GitError> {
            Ok(PathBuf::from("/repo"))
        }

        fn get_project_name(&self) -> Result<String, GitError> {
            Ok("test-project".to_string())
        }

        fn create_worktree(&self, _path: &Path, _branch: &str) -> Result<(), GitError> {
            Ok(())
        }

        fn list_worktrees(&self) -> Result<Vec<GitWorkspaceInfo>, GitError> {
            Ok(self.worktrees.clone())
        }

        fn remove_worktree(&self, _path: &Path) -> Result<(), GitError> {
            Ok(())
        }

        fn get_default_remote_branch(&self) -> Result<String, GitError> {
            Ok("origin/main".to_string())
        }

        fn get_status_porcelain(&self, _worktree_path: &Path) -> Result<String, GitError> {
            Ok(String::new())
        }

        fn count_commits_ahead(
            &self,
            _worktree_path: &Path,
            _base_branch: &str,
        ) -> Result<usize, GitError> {
            Ok(0)
        }

        fn count_commits_behind(
            &self,
            _worktree_path: &Path,
            _base_branch: &str,
        ) -> Result<usize, GitError> {
            Ok(0)
        }

        fn clone_local(&self, _source: &Path, _dest: &Path) -> Result<(), GitError> {
            todo!()
        }

        fn checkout_new_branch(&self, _path: &Path, _branch: &str) -> Result<(), GitError> {
            todo!()
        }

        fn get_current_branch(&self, _path: &Path) -> Result<Option<String>, GitError> {
            todo!()
        }
    }

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
        let mock_git = MockGit {
            worktrees: vec![
                GitWorkspaceInfo {
                    path: PathBuf::from("/repo/worktree1"),
                    branch: Some("feature-1".to_string()),
                },
                GitWorkspaceInfo {
                    path: PathBuf::from("/repo/worktree2"),
                    branch: Some("feature-2".to_string()),
                },
            ],
        };

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

        let service = AgentListService::new(mock_git, mock_process);
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
        let mock_git = MockGit {
            worktrees: vec![GitWorkspaceInfo {
                path: PathBuf::from("/repo/worktree1"),
                branch: Some("feature-1".to_string()),
            }],
        };

        let mock_process = MockProcess {
            processes: vec![(123, "/usr/bin/claude --args".to_string())],
            cwds: vec![(123, PathBuf::from("/other/directory"))],
        };

        let service = AgentListService::new(mock_git, mock_process);
        let agents = service.list_running_agents().unwrap();

        assert_eq!(agents.len(), 0);
    }

    #[test]
    fn test_list_running_agents_cwd_resolution_failure() {
        let mock_git = MockGit {
            worktrees: vec![GitWorkspaceInfo {
                path: PathBuf::from("/repo/worktree1"),
                branch: Some("feature-1".to_string()),
            }],
        };

        let mock_process = MockProcess {
            processes: vec![
                (123, "/usr/bin/claude --args".to_string()),
                (456, "/usr/bin/claude --other".to_string()),
            ],
            cwds: vec![(123, PathBuf::from("/repo/worktree1"))],
            // PID 456 doesn't have a CWD entry, simulating a failure
        };

        let service = AgentListService::new(mock_git, mock_process);
        let agents = service.list_running_agents().unwrap();

        // Should only find the agent with resolvable CWD
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].pid, 123);
    }

    #[test]
    fn test_list_running_agents_empty_cases() {
        // No worktrees
        let mock_git = MockGit { worktrees: vec![] };
        let mock_process = MockProcess {
            processes: vec![(123, "/usr/bin/claude --args".to_string())],
            cwds: vec![(123, PathBuf::from("/repo/worktree1"))],
        };
        let service = AgentListService::new(mock_git, mock_process);
        let agents = service.list_running_agents().unwrap();
        assert_eq!(agents.len(), 0);

        // No processes
        let mock_git = MockGit {
            worktrees: vec![GitWorkspaceInfo {
                path: PathBuf::from("/repo/worktree1"),
                branch: Some("feature-1".to_string()),
            }],
        };
        let mock_process = MockProcess {
            processes: vec![],
            cwds: vec![],
        };
        let service = AgentListService::new(mock_git, mock_process);
        let agents = service.list_running_agents().unwrap();
        assert_eq!(agents.len(), 0);
    }
}
