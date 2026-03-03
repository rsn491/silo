mod common;

use tempfile::TempDir;

#[test]
fn test_launch_creates_worktree() {
    let tmp = TempDir::new().unwrap();
    let repo = common::setup_git_repo(tmp.path());
    let bin_dir = common::create_stub_claude(tmp.path(), "exit 0");

    let original_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bin_dir.display(), original_path);

    common::silo_cmd(tmp.path())
        .arg("launch")
        .current_dir(&repo)
        .env("PATH", &new_path)
        .assert()
        .success();

    let silo_dir = common::silo_test_dir(tmp.path());
    let has_worktree = std::fs::read_dir(&silo_dir)
        .expect("SILO_DIR should exist after launch")
        .flatten()
        .any(|e| e.file_name().to_string_lossy().starts_with("repo-"));

    assert!(
        has_worktree,
        "expected a worktree directory starting with 'repo-' in SILO_DIR"
    );
}
