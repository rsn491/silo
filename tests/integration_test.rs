use std::fs;
use std::process::{Command, Stdio};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

#[test]
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

    // 2. Create a dummy agent script
    let bin_dir = temp_dir.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let agent_path = bin_dir.join("claude");
    fs::write(&agent_path, "#!/bin/sh\nsleep 10").unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&agent_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&agent_path, perms).unwrap();
    }

    // 3. Launch silo launch in the background
    let silo_bin = PathBuf::from(env!("CARGO_BIN_EXE_silo"));

    // We need to pass the dummy agent in the PATH and set HOME
    let mut child = Command::new(&silo_bin)
        .args(["launch", "--agent", "claude"])
        .current_dir(&repo_dir)
        .env("HOME", &home_dir)
        .env("PATH", format!("{}:{}", bin_dir.to_str().unwrap(), std::env::var("PATH").unwrap()))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to launch silo");

    // Give it some time to create the worktree and start the agent
    thread::sleep(Duration::from_secs(2));

    // 4. Verify worktree was created
    let output = Command::new("git")
        .args(["worktree", "list"])
        .current_dir(&repo_dir)
        .output()
        .expect("Failed to run git worktree list");
    let stdout = String::from_utf8_lossy(&output.stdout);
    println!("Worktree list:\n{}", stdout);
    // Should have 2 worktrees: the main repo and the new one
    assert!(stdout.lines().count() >= 2);

    // 5. Verify silo ps shows agent running
    let output = Command::new(&silo_bin)
        .arg("ps")
        .current_dir(&repo_dir)
        .env("HOME", &home_dir)
        .output()
        .expect("Failed to run silo ps");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("Silo ps stdout:\n{}", stdout);
    println!("Silo ps stderr:\n{}", stderr);

    assert!(stdout.contains("claude"));
    assert!(stdout.contains("WORKSPACE"));

    // Cleanup: kill the child process if it's still running
    let _ = child.kill();
}
