mod common;

use assert_cmd::Command;
use predicates::prelude::*;
use std::time::Duration;
use tempfile::TempDir;

/// RAII guard that kills a child process when dropped, so the stub never
/// outlives the test even if it panics.
struct KillOnDrop(std::process::Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn test_running_agent_is_listed() {
    let tmp = TempDir::new().unwrap();
    let repo = common::setup_git_repo(tmp.path());
    let silo_dir = common::silo_test_dir(tmp.path());

    let worktree = silo_dir.join("repo-feat");
    common::add_worktree(&repo, &worktree, "feat");

    // Stub that sleeps long enough for silo ps to observe it.
    let bin_dir = common::create_stub_claude(tmp.path(), "sleep 30");

    let _child = KillOnDrop(
        std::process::Command::new(bin_dir.join("claude"))
            .current_dir(&worktree)
            .spawn()
            .expect("failed to spawn stub claude"),
    );

    // Give the OS a moment to register the process in the process table.
    std::thread::sleep(Duration::from_millis(200));

    let original_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bin_dir.display(), original_path);

    Command::cargo_bin("silo")
        .unwrap()
        .arg("ps")
        .current_dir(&repo)
        .env("SILO_DIR", &silo_dir)
        .env("PATH", &new_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("claude"))
        .stdout(predicate::str::contains("feat"));
}
