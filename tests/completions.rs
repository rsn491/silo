mod common;

use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn test_bash_completion_script_is_emitted() {
    let tmp = TempDir::new().unwrap();

    common::silo_cmd(tmp.path())
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("silo"));
}
