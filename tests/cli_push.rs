use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn upload_help_is_available() {
    Command::cargo_bin("thufs")
        .expect("binary")
        .args(["upload", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Upload a local file"));
}

#[test]
fn upload_fails_for_missing_local_source() {
    Command::cargo_bin("thufs")
        .expect("binary")
        .args(["upload", "/definitely/missing", "repo:course-lib/file.txt"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not exist"));
}

#[test]
fn push_alias_is_available() {
    Command::cargo_bin("thufs")
        .expect("binary")
        .args(["push", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Upload a local file"));
}
