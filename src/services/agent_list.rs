use std::path::PathBuf;

use crate::infra::agent::Agent;
use crate::infra::git::GitOperations;
use crate::infra::git_error::GitError;
use crate::infra::process::{ProcessError, ProcessOperations};

#[derive(Debug)]
pub struct RunningAgent {
    pub pid: u32,
    pub agent_type: Option<Agent>,
    pub worktree_path: PathBuf,
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
    git: G,
    process: P,
}

impl<G: GitOperations, P: ProcessOperations> AgentListService<G, P> {
    pub fn new(git: G, process: P) -> Self {
        Self { git, process }
    }

    pub fn list_running_agents(&self) -> Result<Vec<RunningAgent>, ListError> {
        // Get all worktrees for the current repository
        let worktrees = self.git.list_worktrees()?;

        // Find processes matching known agent names
        let agent_names = Agent::all_names();
        let processes = self.process.find_processes_by_names(&agent_names)?;

        let mut agents = Vec::new();

        // Match processes to worktrees
        for (pid, args) in processes {
            // Try to get the process's current working directory
            // Skip on error (process may have exited)
            let cwd = match self.process.get_process_cwd(pid) {
                Ok(path) => path,
                Err(_) => continue,
            };

            // Check if this CWD matches any worktree
            for worktree in &worktrees {
                if cwd.starts_with(&worktree.path) {
                    agents.push(RunningAgent {
                        pid,
                        agent_type: extract_agent_type(&args),
                        worktree_path: worktree.path.clone(),
                        branch: worktree.branch.clone(),
                    });
                    break;
                }
            }
        }

        Ok(agents)
    }

    pub fn get_active_worktree_paths(&self) -> Result<Vec<PathBuf>, ListError> {
        let running_agents = self.list_running_agents()?;

        let active_paths: Vec<PathBuf> = running_agents
            .into_iter()
            .map(|agent| agent.worktree_path)
            .collect();

        Ok(active_paths)
    }
}

fn extract_agent_type(args: &str) -> Option<Agent> {
    // Extract the command name from the process args
    let parts: Vec<&str> = args.split_whitespace().collect();
    if let Some(first) = parts.first()
        && let Some(name) = first.split('/').next_back()
    {
        return Agent::try_from_str(name);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::git::WorktreeInfo;
    use std::path::Path;

    // Mock GitOperations
    struct MockGit {
        worktrees: Vec<WorktreeInfo>,
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

        fn list_worktrees(&self) -> Result<Vec<WorktreeInfo>, GitError> {
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
                WorktreeInfo {
                    path: PathBuf::from("/repo/worktree1"),
                    branch: Some("feature-1".to_string()),
                },
                WorktreeInfo {
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
        assert_eq!(agents[0].worktree_path, PathBuf::from("/repo/worktree1"));
        assert_eq!(agents[0].branch, Some("feature-1".to_string()));
        assert_eq!(agents[1].pid, 456);
        assert_eq!(agents[1].worktree_path, PathBuf::from("/repo/worktree2"));
        assert_eq!(agents[1].branch, Some("feature-2".to_string()));
    }

    #[test]
    fn test_list_running_agents_outside_worktrees() {
        let mock_git = MockGit {
            worktrees: vec![WorktreeInfo {
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
            worktrees: vec![WorktreeInfo {
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
            worktrees: vec![WorktreeInfo {
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
