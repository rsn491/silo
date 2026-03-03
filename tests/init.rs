use assert_cmd::Command;
use tempfile::TempDir;

#[test]
fn test_silo_dir_is_created() {
    let tmp = TempDir::new().unwrap();

    Command::cargo_bin("silo")
        .unwrap()
        .arg("init")
        .env("HOME", tmp.path())
        .assert()
        .success();

    assert!(
        tmp.path().join(".silo").is_dir(),
        "expected ~/.silo to be created"
    );
}
