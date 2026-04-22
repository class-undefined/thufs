use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn mkrepo_help_is_available() {
    Command::cargo_bin("thufs")
        .expect("binary")
        .args(["mkrepo", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Create a library or repository"));
}

#[test]
fn mkdir_help_is_available() {
    Command::cargo_bin("thufs")
        .expect("binary")
        .args(["mkdir", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Create a remote directory"));
}
