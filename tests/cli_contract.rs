use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn root_help_shows_grouped_management_commands() {
    Command::cargo_bin("thufs")
        .expect("binary")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "info, repos, ls, upload, download, and share",
        ))
        .stdout(predicate::str::contains("info"))
        .stdout(predicate::str::contains("repos"))
        .stdout(predicate::str::contains("upload"))
        .stdout(predicate::str::contains("download"))
        .stdout(predicate::str::contains("auth"))
        .stdout(predicate::str::contains("config"));
}

#[test]
fn set_token_redacts_secret_in_stdout() {
    let temp = tempdir().expect("tempdir");

    Command::cargo_bin("thufs")
        .expect("binary")
        .env("THUFS_CONFIG_DIR", temp.path())
        .args(["auth", "set-token", "sample-token-value"])
        .assert()
        .success()
        .stdout(predicate::str::contains("sa...ue"))
        .stdout(predicate::str::contains("sample-token-value").not())
        .stderr(predicate::str::is_empty());
}

#[test]
fn config_show_supports_json_output() {
    let temp = tempdir().expect("tempdir");

    Command::cargo_bin("thufs")
        .expect("binary")
        .env("THUFS_CONFIG_DIR", temp.path())
        .args(["auth", "set-token", "sample-token-value"])
        .assert()
        .success();

    Command::cargo_bin("thufs")
        .expect("binary")
        .env("THUFS_CONFIG_DIR", temp.path())
        .args(["--json", "config", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"token\": \"sa...ue\""))
        .stderr(predicate::str::is_empty());
}
