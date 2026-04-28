use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn download_help_is_available() {
    Command::cargo_bin("thufs")
        .expect("binary")
        .args(["download", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Download a remote file"))
        .stdout(predicate::str::contains("--share"))
        .stdout(predicate::str::contains("--conflict"))
        .stdout(predicate::str::contains("--mode"))
        .stdout(predicate::str::contains("--workers"))
        .stdout(predicate::str::contains("--progress"))
        .stdout(predicate::str::contains("jsonl"));
}

#[test]
fn download_fails_when_local_destination_exists_with_fail_policy() {
    let temp = tempdir().expect("tempdir");
    let destination = temp.path().join("existing.txt");
    std::fs::write(&destination, "content").expect("write");

    Command::cargo_bin("thufs")
        .expect("binary")
        .env("THUFS_DEFAULT_REPO", "course-lib")
        .args([
            "download",
            "slides/week1.pdf",
            destination.to_str().expect("utf8"),
            "--conflict",
            "fail",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn download_without_local_argument_reaches_token_validation() {
    let temp = tempdir().expect("tempdir");

    Command::cargo_bin("thufs")
        .expect("binary")
        .env("THUFS_CONFIG_DIR", temp.path())
        .args(["download", "repo:course-lib/week1.pdf"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no token configured"));
}

#[test]
fn pull_alias_is_available() {
    Command::cargo_bin("thufs")
        .expect("binary")
        .args(["pull", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Download a remote file"));
}

#[test]
fn download_share_hash_without_flag_is_treated_as_remote_path() {
    let temp = tempdir().expect("tempdir");

    Command::cargo_bin("thufs")
        .expect("binary")
        .env("THUFS_CONFIG_DIR", temp.path())
        .args(["download", "abc123XYZ_"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("remote path must use repo:"));
}

#[test]
fn download_share_hash_with_flag_reaches_share_flow() {
    let temp = tempdir().expect("tempdir");

    Command::cargo_bin("thufs")
        .expect("binary")
        .env("THUFS_CONFIG_DIR", temp.path())
        .args(["download", "--share", "abc123XYZ_"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("failed to inspect shared file")
                .or(predicate::str::contains("shared file")),
        );
}
