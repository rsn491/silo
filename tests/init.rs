use assert_cmd::Command;
use tempfile::TempDir;

#[test]
fn test_silo_dir_is_created() {
    let tmp = TempDir::new().unwrap();
    let silo_dir = tmp.path().join(".tmp_silo_it");

    Command::cargo_bin("silo")
        .unwrap()
        .arg("init")
        .env("SILO_DIR", &silo_dir)
        .assert()
        .success();

    assert!(silo_dir.is_dir(), "expected SILO_DIR to be created");
}
