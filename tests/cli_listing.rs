use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn ls_help_is_available() {
    Command::cargo_bin("thufs")
        .expect("binary")
        .args(["ls", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "List remote files and directories",
        ));
}

#[test]
fn ls_fails_without_default_repo_for_shorthand_path() {
    let temp = tempdir().expect("tempdir");

    Command::cargo_bin("thufs")
        .expect("binary")
        .env("THUFS_CONFIG_DIR", temp.path())
        .args(["ls", "notes"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("remote path must use repo:"));
}

#[test]
fn ls_fails_without_token_before_network_for_explicit_path() {
    let temp = tempdir().expect("tempdir");

    Command::cargo_bin("thufs")
        .expect("binary")
        .env("THUFS_CONFIG_DIR", temp.path())
        .args(["--json", "ls", "repo:course-lib/slides"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no token configured"));
}
