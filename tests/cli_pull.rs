use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn pull_help_is_available() {
    Command::cargo_bin("thufs")
        .expect("binary")
        .args(["pull", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Download a remote file"));
}

#[test]
fn pull_fails_when_local_destination_exists_without_overwrite() {
    let temp = tempdir().expect("tempdir");
    let destination = temp.path().join("existing.txt");
    std::fs::write(&destination, "content").expect("write");

    Command::cargo_bin("thufs")
        .expect("binary")
        .env("THUFS_DEFAULT_REPO", "course-lib")
        .args([
            "pull",
            "slides/week1.pdf",
            destination.to_str().expect("utf8"),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}
