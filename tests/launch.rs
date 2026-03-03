mod common;

use assert_cmd::Command;
use tempfile::TempDir;

#[test]
fn test_launch_creates_worktree() {
    let tmp = TempDir::new().unwrap();
    let repo = common::setup_git_repo(tmp.path());
    let bin_dir = common::create_stub_claude(tmp.path(), "exit 0");

    let original_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bin_dir.display(), original_path);

    Command::cargo_bin("silo")
        .unwrap()
        .arg("launch")
        .current_dir(&repo)
        .env("HOME", tmp.path())
        .env("PATH", &new_path)
        .assert()
        .success();

    let silo_dir = tmp.path().join(".silo");
    let has_worktree = std::fs::read_dir(&silo_dir)
        .expect("~/.silo should exist after launch")
        .flatten()
        .any(|e| e.file_name().to_string_lossy().starts_with("repo-"));

    assert!(has_worktree, "expected a worktree directory starting with 'repo-' in ~/.silo/");
}
