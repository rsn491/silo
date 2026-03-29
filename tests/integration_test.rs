use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
#[ignore = "requires opencode to be installed"]
fn test_launch_and_ps() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let home_dir = temp_dir.path().join("home");
    let repo_dir = temp_dir.path().join("repo");
    fs::create_dir_all(&home_dir).unwrap();
    fs::create_dir_all(&repo_dir).unwrap();

    // 1. Create a dummy git repo
    let run_git = |args: &[&str], dir: &PathBuf| {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .expect("Failed to run git");
        assert!(status.success());
    };

    run_git(&["init"], &repo_dir);
    run_git(&["config", "user.email", "you@example.com"], &repo_dir);
    run_git(&["config", "user.name", "Your Name"], &repo_dir);
    fs::write(repo_dir.join("README.md"), "Dummy README").unwrap();
    run_git(&["add", "README.md"], &repo_dir);
    run_git(&["commit", "-m", "Initial commit"], &repo_dir);

    // 2. Launch silo launch in the background
    let silo_bin = PathBuf::from(env!("CARGO_BIN_EXE_silo"));

    let child = Command::new(&silo_bin)
        .args(["launch", "--agent", "opencode"])
        .current_dir(&repo_dir)
        .env("HOME", &home_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to launch silo");
    let _child = ChildGuard(child);

    // Poll silo ps until the agent appears (implies worktree was created too)
    let timeout = Duration::from_secs(10);
    let interval = Duration::from_millis(100);
    let start = std::time::Instant::now();
    let ps_stdout = loop {
        let output = Command::new(&silo_bin)
            .arg("ps")
            .current_dir(&repo_dir)
            .env("HOME", &home_dir)
            .output()
            .expect("Failed to run silo ps");
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        if stdout.contains("opencode") {
            break stdout;
        }
        assert!(
            start.elapsed() < timeout,
            "timed out waiting for agent to appear in silo ps"
        );
        thread::sleep(interval);
    };

    println!("Silo ps stdout:\n{}", ps_stdout);
    assert!(ps_stdout.contains("WORKSPACE"));

    // 3. Verify worktree was created
    let output = Command::new("git")
        .args(["worktree", "list"])
        .current_dir(&repo_dir)
        .output()
        .expect("Failed to run git worktree list");
    let stdout = String::from_utf8_lossy(&output.stdout);
    println!("Worktree list:\n{}", stdout);
    assert!(stdout.lines().count() >= 2);

    // _child is dropped here, killing the process via ChildGuard::drop
}
