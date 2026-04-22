use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn repos_help_is_available() {
    Command::cargo_bin("thufs")
        .expect("binary")
        .args(["repos", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("List repositories or libraries"));
}

#[test]
fn repositories_alias_is_available() {
    Command::cargo_bin("thufs")
        .expect("binary")
        .args(["libraries", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("List repositories or libraries"));
}

#[test]
fn repos_fail_without_token_before_network() {
    let temp = tempdir().expect("tempdir");

    Command::cargo_bin("thufs")
        .expect("binary")
        .env("THUFS_CONFIG_DIR", temp.path())
        .arg("repos")
        .assert()
        .failure()
        .stderr(predicate::str::contains("no token configured"));
}
