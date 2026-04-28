use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn upload_help_is_available() {
    Command::cargo_bin("thufs")
        .expect("binary")
        .args(["upload", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Upload a local file"))
        .stdout(predicate::str::contains("remote directory"))
        .stdout(predicate::str::contains("--conflict"))
        .stdout(predicate::str::contains("--progress"))
        .stdout(predicate::str::contains("jsonl"));
}

#[test]
fn upload_fails_for_missing_local_source() {
    let temp = tempdir().expect("tempdir");
    let missing = temp.path().join("missing.txt");

    Command::cargo_bin("thufs")
        .expect("binary")
        .arg("upload")
        .arg(&missing)
        .arg("repo:course-lib/file.txt")
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

#[test]
fn upload_repo_root_without_default_repo_can_auto_create_repo() {
    let temp = tempdir().expect("tempdir");
    let source = temp.path().join("report.txt");
    std::fs::write(&source, "content").expect("write");

    Command::cargo_bin("thufs")
        .expect("binary")
        .env("THUFS_CONFIG_DIR", temp.path().join("config"))
        .arg("upload")
        .arg(&source)
        .arg("repo:course-lib")
        .assert()
        .failure()
        .stderr(predicate::str::contains("no token configured"))
        .stderr(predicate::str::contains("remote path must use repo:").not());
}
