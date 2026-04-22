use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn info_help_is_available() {
    Command::cargo_bin("thufs")
        .expect("binary")
        .args(["info", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Show account information"));
}

#[test]
fn info_fails_without_token_before_network() {
    let temp = tempdir().expect("tempdir");

    Command::cargo_bin("thufs")
        .expect("binary")
        .env("THUFS_CONFIG_DIR", temp.path())
        .arg("info")
        .assert()
        .failure()
        .stderr(predicate::str::contains("no token configured"));
}
