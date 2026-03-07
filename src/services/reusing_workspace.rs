//! Workspace factory that reuses an existing inactive workspace when available.

use std::collections::HashSet;
use std::path::PathBuf;

use crate::infra::git::GitOperations;
use crate::infra::process::ProcessOperations;
use crate::services::agent_launcher::LaunchError;
use crate::services::agent_list::{AgentListService, ListError};
use crate::services::agent_workspace::{WorkspaceFactory, WorkspaceManager};
use crate::services::global_workspace::GlobalWorkspaceManager;

/// A [`WorkspaceFactory`] decorator that reuses an existing inactive workspace
/// when one is available, falling back to creating a new workspace otherwise.
///
/// A workspace is considered inactive when:
/// - No agent process is currently running in it.
/// - It has no uncommitted changes.
/// - It has no commits ahead of the remote base branch (i.e., all work has been pushed).
pub struct ReusingWorkspaceFactory<G, P>
where
    G: GitOperations + Clone,
    P: ProcessOperations + Clone,
{
    git: G,
    process: P,
    inner: Box<dyn WorkspaceFactory>,
}

impl<G, P> ReusingWorkspaceFactory<G, P>
where
    G: GitOperations + Clone,
    P: ProcessOperations + Clone,
{
    /// Creates a new `ReusingWorkspaceFactory`.
    ///
    /// * `git`     – Git operations used to enumerate existing workspaces.
    /// * `process` – Process operations used to detect running agents.
    /// * `inner`   – Fallback factory used when no inactive workspace is found.
    pub fn new(git: G, process: P, inner: Box<dyn WorkspaceFactory>) -> Self {
        Self { git, process, inner }
    }
}

impl<G, P> WorkspaceFactory for ReusingWorkspaceFactory<G, P>
where
    G: GitOperations + Clone,
    P: ProcessOperations + Clone,
{
    /// Returns the path of an existing inactive workspace when one exists,
    /// otherwise delegates to the inner factory to create a new one.
    ///
    /// # Errors
    ///
    /// Returns [`LaunchError`] if workspace listing, agent detection, or
    /// fallback workspace creation fails.
    fn create(&self, branch: Option<String>) -> Result<PathBuf, LaunchError> {
        let workspace_manager = GlobalWorkspaceManager::with_git(self.git.clone());
        let agent_list = AgentListService::new(
            GlobalWorkspaceManager::with_git(self.git.clone()),
            self.process.clone(),
        );

        let all = workspace_manager.get_all().map_err(LaunchError::Git)?;
        let active_paths: HashSet<PathBuf> = agent_list
            .get_active_paths()
            .map_err(map_list_error)?
            .into_iter()
            .collect();

        let inactive = all.into_iter().find(|ws| {
            !active_paths.contains(&ws.path)
                && !ws.has_uncommitted_changes
                && ws.commits_ahead == 0
        });

        if let Some(ws) = inactive {
            eprintln!("Reusing inactive workspace: {}", ws.path.display());
            return Ok(ws.path);
        }

        eprintln!("No inactive workspace found, creating new workspace...");
        self.inner.create(branch)
    }
}

fn map_list_error(e: ListError) -> LaunchError {
    match e {
        ListError::Git(e) => LaunchError::Git(e),
        ListError::Process(e) => LaunchError::AgentSpawnError(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::git::{GitOperations, GitWorkspaceInfo};
    use crate::infra::git_error::GitError;
    use crate::infra::process::ProcessError;
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    // --- workspace specification (drives all mock responses) ---

    #[derive(Clone)]
    struct WsSpec {
        path: PathBuf,
        /// Whether `get_status_porcelain` should report uncommitted changes.
        dirty: bool,
        /// Value returned by `count_commits_ahead`.
        ahead: usize,
    }

    fn spec(path: &str, dirty: bool, ahead: usize) -> WsSpec {
        WsSpec { path: PathBuf::from(path), dirty, ahead }
    }

    // --- Clone-able git mock ---
    //
    // `list_worktrees` always prepends a fake main-repo entry (index 0) so that
    // `GitWorktreeWorkspace::get_all()` — which skips the first entry — works
    // correctly.  Per-path status is derived from the stored `WsSpec`s.

    #[derive(Clone)]
    struct FakeGit {
        specs: Arc<Vec<WsSpec>>,
    }

    impl FakeGit {
        fn new(specs: Vec<WsSpec>) -> Self {
            Self { specs: Arc::new(specs) }
        }
    }

    impl GitOperations for FakeGit {
        fn get_repo_root(&self) -> Result<PathBuf, GitError> {
            Ok(PathBuf::from("/repo"))
        }
        fn get_project_name(&self) -> Result<String, GitError> {
            Ok("proj".to_string())
        }
        fn create_worktree(&self, _: &Path, _: &str) -> Result<(), GitError> {
            unimplemented!()
        }
        fn list_worktrees(&self) -> Result<Vec<GitWorkspaceInfo>, GitError> {
            let mut list = vec![GitWorkspaceInfo {
                path: PathBuf::from("/repo"),
                branch: Some("main".to_string()),
                ..Default::default()
            }];
            for s in self.specs.iter() {
                list.push(GitWorkspaceInfo {
                    path: s.path.clone(),
                    branch: Some("feat".to_string()),
                    ..Default::default()
                });
            }
            Ok(list)
        }
        fn remove_worktree(&self, _: &Path) -> Result<(), GitError> {
            unimplemented!()
        }
        fn get_default_remote_branch(&self) -> Result<String, GitError> {
            Ok("origin/main".to_string())
        }
        fn get_status_porcelain(&self, path: &Path) -> Result<String, GitError> {
            let dirty = self.specs.iter().find(|s| s.path == path).map(|s| s.dirty).unwrap_or(false);
            Ok(if dirty { "M file.rs\n".to_string() } else { String::new() })
        }
        fn count_commits_ahead(&self, path: &Path, _: &str) -> Result<usize, GitError> {
            Ok(self.specs.iter().find(|s| s.path == path).map(|s| s.ahead).unwrap_or(0))
        }
        fn count_commits_behind(&self, _: &Path, _: &str) -> Result<usize, GitError> {
            Ok(0)
        }
        fn clone_local(&self, _: &Path, _: &Path) -> Result<(), GitError> {
            unimplemented!()
        }
        fn checkout_new_branch(&self, _: &Path, _: &str) -> Result<(), GitError> {
            unimplemented!()
        }
        fn get_current_branch(&self, _: &Path) -> Result<Option<String>, GitError> {
            unimplemented!()
        }
    }

    // --- Clone-able process mock ---

    #[derive(Clone)]
    struct FakeProcess {
        processes: Vec<(u32, String)>,
        cwds: Vec<(u32, PathBuf)>,
    }

    impl ProcessOperations for FakeProcess {
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
                .ok_or_else(|| ProcessError::CommandFailed("not found".to_string()))
        }
    }

    // --- spy factory ---

    #[derive(Clone)]
    struct SpyFactory {
        called: Arc<Mutex<bool>>,
        path: PathBuf,
    }

    impl SpyFactory {
        fn new(path: &str) -> Self {
            Self { called: Arc::new(Mutex::new(false)), path: PathBuf::from(path) }
        }

        fn was_called(&self) -> bool {
            *self.called.lock().unwrap()
        }
    }

    impl WorkspaceFactory for SpyFactory {
        fn create(&self, _branch: Option<String>) -> Result<PathBuf, LaunchError> {
            *self.called.lock().unwrap() = true;
            Ok(self.path.clone())
        }
    }

    // --- builder ---

    fn factory(
        specs: Vec<WsSpec>,
        processes: Vec<(u32, String)>,
        cwds: Vec<(u32, PathBuf)>,
        spy: SpyFactory,
    ) -> ReusingWorkspaceFactory<FakeGit, FakeProcess> {
        ReusingWorkspaceFactory::new(
            FakeGit::new(specs),
            FakeProcess { processes, cwds },
            Box::new(spy),
        )
    }

    // --- tests ---

    #[test]
    fn reuses_inactive_workspace() {
        let spy = SpyFactory::new("/new");
        let f = factory(vec![spec("/ws/idle", false, 0)], vec![], vec![], spy.clone());

        assert_eq!(f.create(None).unwrap(), PathBuf::from("/ws/idle"));
        assert!(!spy.was_called(), "inner factory must not be called when a workspace is reused");
    }

    #[test]
    fn skips_workspace_with_running_agent() {
        let spy = SpyFactory::new("/new");
        let f = factory(
            vec![spec("/ws/active", false, 0)],
            vec![(42, "/usr/bin/claude".to_string())],
            vec![(42, PathBuf::from("/ws/active"))],
            spy.clone(),
        );

        assert_eq!(f.create(None).unwrap(), PathBuf::from("/new"));
        assert!(spy.was_called());
    }

    #[test]
    fn skips_workspace_with_uncommitted_changes() {
        let spy = SpyFactory::new("/new");
        let f = factory(vec![spec("/ws/dirty", true, 0)], vec![], vec![], spy.clone());

        assert_eq!(f.create(None).unwrap(), PathBuf::from("/new"));
        assert!(spy.was_called());
    }

    #[test]
    fn skips_workspace_with_unpushed_commits() {
        let spy = SpyFactory::new("/new");
        let f = factory(vec![spec("/ws/ahead", false, 3)], vec![], vec![], spy.clone());

        assert_eq!(f.create(None).unwrap(), PathBuf::from("/new"));
        assert!(spy.was_called());
    }

    #[test]
    fn no_workspaces_falls_back_to_inner() {
        let spy = SpyFactory::new("/new");
        let f = factory(vec![], vec![], vec![], spy.clone());

        assert_eq!(f.create(None).unwrap(), PathBuf::from("/new"));
        assert!(spy.was_called());
    }

    #[test]
    fn picks_first_eligible_workspace() {
        let spy = SpyFactory::new("/new");
        let f = factory(
            vec![
                spec("/ws/ahead", false, 1),       // ineligible: unpushed commits
                spec("/ws/dirty", true, 0),        // ineligible: uncommitted changes
                spec("/ws/clean", false, 0),       // eligible — should be picked
                spec("/ws/also-clean", false, 0),  // eligible but must not be picked
            ],
            vec![],
            vec![],
            spy.clone(),
        );

        assert_eq!(f.create(None).unwrap(), PathBuf::from("/ws/clean"));
        assert!(!spy.was_called());
    }
}
