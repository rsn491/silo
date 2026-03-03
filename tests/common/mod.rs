//! Shared helpers for integration tests.
//!
//! Each test binary only calls the subset of helpers it needs, so the
//! module-level suppression below avoids spurious dead-code warnings that
//! would otherwise fire for helpers unused in a particular binary.
#![allow(dead_code)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Returns the isolated silo directory used by integration tests.
///
/// Tests set `SILO_DIR` to this path so they never touch `~/.silo`.
pub fn silo_test_dir(base: &Path) -> PathBuf {
    base.join(".tmp_silo_it")
}

/// Initialise a bare-minimum git repo with one commit at `base/repo`.
/// Returns the path to the repo root.
pub fn setup_git_repo(base: &Path) -> PathBuf {
    let repo = base.join("repo");
    fs::create_dir_all(&repo).unwrap();

    for args in &[
        vec!["init"],
        vec!["config", "user.email", "test@example.com"],
        vec!["config", "user.name", "Test"],
        vec!["config", "commit.gpgsign", "false"],
    ] {
        Command::new("git")
            .args(args)
            .current_dir(&repo)
            .status()
            .unwrap();
    }

    fs::write(repo.join("README.md"), "# Test\n").unwrap();

    for args in &[vec!["add", "."], vec!["commit", "-m", "Initial commit"]] {
        Command::new("git")
            .args(args)
            .current_dir(&repo)
            .status()
            .unwrap();
    }

    repo
}

/// Create `base/bin/claude` with `script` as its body and mark it executable.
/// Returns the `base/bin` directory so it can be prepended to `PATH`.
pub fn create_stub_claude(base: &Path, script: &str) -> PathBuf {
    let bin_dir = base.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let stub = bin_dir.join("claude");
    fs::write(&stub, format!("#!/bin/sh\n{}\n", script)).unwrap();
    fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).unwrap();
    bin_dir
}

/// Add a git worktree at `dest` with the given `branch`, linked to `repo`.
pub fn add_worktree(repo: &Path, dest: &Path, branch: &str) {
    fs::create_dir_all(dest.parent().unwrap()).unwrap();
    Command::new("git")
        .args(["worktree", "add", "-b", branch, dest.to_str().unwrap()])
        .current_dir(repo)
        .status()
        .unwrap();
}
