//! Workspace factory that reuses an existing inactive workspace when available.

use std::path::PathBuf;

use crate::infra::git::GitOperations;
use crate::services::agent_launcher::LaunchError;
use crate::services::agent_workspace::{WorkspaceFactory, WorkspaceManager};
use crate::services::global_workspace::GlobalWorkspaceManager;
use crate::services::workspace_lock::WorkspaceLock;

/// A [`WorkspaceFactory`] decorator that reuses an existing inactive workspace
/// when one is available, falling back to creating a new workspace otherwise.
///
/// A workspace is considered inactive when:
/// - It has no `silo.lock` file (no agent is running in it).
/// - It has no uncommitted changes.
/// - It has no commits ahead of the remote base branch (i.e., all work has been pushed).
pub struct ReusingWorkspaceFactory<G>
where
    G: GitOperations + Clone,
{
    /// Git operations used to inspect workspace state.
    git: G,
    /// Fallback factory used when no inactive workspace is available.
    inner: Box<dyn WorkspaceFactory>,
}

impl<G> ReusingWorkspaceFactory<G>
where
    G: GitOperations + Clone,
{
    /// Creates a new `ReusingWorkspaceFactory`.
    ///
    /// * `git`   – Git operations used to enumerate existing workspaces.
    /// * `inner` – Fallback factory used when no inactive workspace is found.
    pub fn new(git: G, inner: Box<dyn WorkspaceFactory>) -> Self {
        Self { git, inner }
    }
}

impl<G> WorkspaceFactory for ReusingWorkspaceFactory<G>
where
    G: GitOperations + Clone,
{
    /// Returns the path of an existing inactive workspace when one exists,
    /// otherwise delegates to the inner factory to create a new one.
    ///
    /// # Errors
    ///
    /// Returns [`LaunchError`] if workspace listing or fallback workspace
    /// creation fails.
    fn create(&self, branch: Option<String>) -> Result<PathBuf, LaunchError> {
        let workspace_manager = GlobalWorkspaceManager::with_git(self.git.clone());
        let all = workspace_manager.get_all().map_err(LaunchError::Git)?;

        let inactive = all.into_iter().find(|ws| {
            !WorkspaceLock::is_locked(&ws.path)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::git::{GitOperations, GitWorkspaceInfo};
    use crate::infra::git_error::GitError;
    use std::path::Path;
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;

    // --- workspace specification (drives all mock responses) ---

    #[derive(Clone)]
    struct WsSpec {
        /// Temporary directory that backs this workspace (keeps it alive).
        _dir: Arc<TempDir>,
        path: PathBuf,
        /// Whether `get_status_porcelain` should report uncommitted changes.
        dirty: bool,
        /// Value returned by `count_commits_ahead`.
        ahead: usize,
    }

    fn spec(dirty: bool, ahead: usize) -> WsSpec {
        let dir = Arc::new(tempfile::tempdir().unwrap());
        let path = dir.path().to_path_buf();
        WsSpec {
            _dir: dir,
            path,
            dirty,
            ahead,
        }
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
            Self {
                specs: Arc::new(specs),
            }
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
            let dirty = self
                .specs
                .iter()
                .find(|s| s.path == path)
                .map(|s| s.dirty)
                .unwrap_or(false);
            Ok(if dirty {
                "M file.rs\n".to_string()
            } else {
                String::new()
            })
        }
        fn count_commits_ahead(&self, path: &Path, _: &str) -> Result<usize, GitError> {
            Ok(self
                .specs
                .iter()
                .find(|s| s.path == path)
                .map(|s| s.ahead)
                .unwrap_or(0))
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
        fn stage_all(&self, _: &Path) -> Result<(), GitError> {
            Ok(())
        }
        fn commit_all(&self, _: &Path, _: &str) -> Result<(), GitError> {
            Ok(())
        }
        fn push(&self, _: &Path) -> Result<(), GitError> {
            Ok(())
        }
        fn get_changes_summary(&self, _: &Path) -> Result<String, GitError> {
            Ok(String::new())
        }
        fn rename_branch(&self, _: &Path, _: &str) -> Result<(), GitError> {
            Ok(())
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
            Self {
                called: Arc::new(Mutex::new(false)),
                path: PathBuf::from(path),
            }
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

    fn factory(specs: Vec<WsSpec>, spy: SpyFactory) -> ReusingWorkspaceFactory<FakeGit> {
        ReusingWorkspaceFactory::new(FakeGit::new(specs), Box::new(spy))
    }

    // --- tests ---

    #[test]
    fn reuses_inactive_workspace() {
        let ws = spec(false, 0);
        let path = ws.path.clone();
        let spy = SpyFactory::new("/new");
        let f = factory(vec![ws], spy.clone());

        assert_eq!(f.create(None).unwrap(), path);
        assert!(
            !spy.was_called(),
            "inner factory must not be called when a workspace is reused"
        );
    }

    #[test]
    fn skips_locked_workspace() {
        let ws = spec(false, 0);
        // Simulate an active agent by placing a lock file.
        WorkspaceLock::new(&ws.path).try_acquire().unwrap();
        let spy = SpyFactory::new("/new");
        let f = factory(vec![ws], spy.clone());

        assert_eq!(f.create(None).unwrap(), PathBuf::from("/new"));
        assert!(spy.was_called());
    }

    #[test]
    fn skips_workspace_with_uncommitted_changes() {
        let ws = spec(true, 0);
        let spy = SpyFactory::new("/new");
        let f = factory(vec![ws], spy.clone());

        assert_eq!(f.create(None).unwrap(), PathBuf::from("/new"));
        assert!(spy.was_called());
    }

    #[test]
    fn skips_workspace_with_unpushed_commits() {
        let ws = spec(false, 3);
        let spy = SpyFactory::new("/new");
        let f = factory(vec![ws], spy.clone());

        assert_eq!(f.create(None).unwrap(), PathBuf::from("/new"));
        assert!(spy.was_called());
    }

    #[test]
    fn no_workspaces_falls_back_to_inner() {
        let spy = SpyFactory::new("/new");
        let f = factory(vec![], spy.clone());

        assert_eq!(f.create(None).unwrap(), PathBuf::from("/new"));
        assert!(spy.was_called());
    }

    #[test]
    fn picks_first_eligible_workspace() {
        let ahead = spec(false, 1); // ineligible: unpushed commits
        let dirty = spec(true, 0); // ineligible: uncommitted changes
        let clean = spec(false, 0); // eligible — should be picked
        let also_clean = spec(false, 0); // eligible but must not be picked

        let clean_path = clean.path.clone();

        let spy = SpyFactory::new("/new");
        let f = factory(vec![ahead, dirty, clean, also_clean], spy.clone());

        assert_eq!(f.create(None).unwrap(), clean_path);
        assert!(!spy.was_called());
    }

    #[test]
    fn unlocked_after_manual_lock_removal() {
        let ws = spec(false, 0);
        let path = ws.path.clone();

        // Acquire and then manually release the lock (simulating manual cleanup).
        let lock = WorkspaceLock::new(&path);
        lock.try_acquire().unwrap();
        lock.release();

        let spy = SpyFactory::new("/new");
        let f = factory(vec![ws], spy.clone());

        assert_eq!(f.create(None).unwrap(), path);
        assert!(!spy.was_called());
    }
}
