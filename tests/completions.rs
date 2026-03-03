use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_bash_completion_script_is_emitted() {
    Command::cargo_bin("silo")
        .unwrap()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("silo"));
}
