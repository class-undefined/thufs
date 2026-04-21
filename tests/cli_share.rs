use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn share_help_is_available() {
    Command::cargo_bin("thufs")
        .expect("binary")
        .args(["share", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Create a THU Cloud Drive share link",
        ))
        .stdout(predicate::str::contains("--password"))
        .stdout(predicate::str::contains("--expire-days"));
}

#[test]
fn share_rejects_zero_day_expiration_before_network() {
    Command::cargo_bin("thufs")
        .expect("binary")
        .env("THUFS_DEFAULT_REPO", "course-lib")
        .args(["share", "slides/week1.pdf", "--expire-days", "0"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("at least 1 day"));
}
