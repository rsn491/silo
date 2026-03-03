mod common;

use tempfile::TempDir;

#[test]
fn test_silo_dir_is_created() {
    let tmp = TempDir::new().unwrap();

    common::silo_cmd(tmp.path())
        .arg("init")
        .assert()
        .success();

    assert!(
        common::silo_test_dir(tmp.path()).is_dir(),
        "expected SILO_DIR to be created"
    );
}
