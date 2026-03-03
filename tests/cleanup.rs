mod common;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn test_inactive_worktree_is_removed() {
    let tmp = TempDir::new().unwrap();
    let repo = common::setup_git_repo(tmp.path());

    let silo_dir = tmp.path().join(".silo");
    let worktree = silo_dir.join("repo-feat");
    common::add_worktree(&repo, &worktree, "feat");

    assert!(worktree.is_dir(), "worktree should exist before cleanup");

    Command::cargo_bin("silo")
        .unwrap()
        .args(["cleanup", "-y"])
        .current_dir(&repo)
        .env("HOME", tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Successfully removed 1"));

    assert!(
        !worktree.exists(),
        "worktree directory should be gone after cleanup"
    );
}
