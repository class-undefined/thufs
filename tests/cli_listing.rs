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
fn ls_supports_json_output() {
    let temp = tempdir().expect("tempdir");

    Command::cargo_bin("thufs")
        .expect("binary")
        .env("THUFS_CONFIG_DIR", temp.path())
        .args(["auth", "set-token", "sample-token-value"])
        .assert()
        .success();

    std::fs::write(
        temp.path().join("config.json"),
        r#"{
  "token": "sample-token-value",
  "default_repo": "course-lib",
  "output": "human"
}"#,
    )
    .expect("write config");

    Command::cargo_bin("thufs")
        .expect("binary")
        .env("THUFS_CONFIG_DIR", temp.path())
        .args(["--json", "ls", "slides"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"repo\": \"course-lib\""))
        .stdout(predicate::str::contains("\"items\""))
        .stderr(predicate::str::is_empty());
}
