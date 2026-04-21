use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn push_help_is_available() {
    Command::cargo_bin("thufs")
        .expect("binary")
        .args(["push", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Upload a local file"));
}

#[test]
fn push_fails_for_missing_local_source() {
    Command::cargo_bin("thufs")
        .expect("binary")
        .args(["push", "/definitely/missing", "repo:course-lib/file.txt"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not exist"));
}
