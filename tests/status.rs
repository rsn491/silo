mod common;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn test_worktree_with_changes_is_shown() {
    let tmp = TempDir::new().unwrap();
    let repo = common::setup_git_repo(tmp.path());

    let silo_dir = tmp.path().join(".silo");
    let worktree = silo_dir.join("repo-feat");
    common::add_worktree(&repo, &worktree, "feat");

    // Create an uncommitted file inside the worktree so `git status --porcelain`
    // reports one changed file.
    std::fs::write(worktree.join("new-file.txt"), "hello\n").unwrap();

    Command::cargo_bin("silo")
        .unwrap()
        .arg("status")
        .current_dir(&repo)
        .env("HOME", tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("feat"))
        .stdout(predicate::str::contains("1 files"));
}
