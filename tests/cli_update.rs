use std::time::{SystemTime, UNIX_EPOCH};

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn cached_update_checks_work_without_authentication_and_support_json() {
    let temp = tempfile::tempdir().unwrap();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    std::fs::write(
        temp.path().join("update-check.json"),
        format!(r#"{{"checked_at":{now},"tag":"v999.0.0"}}"#),
    )
    .unwrap();
    let output = Command::cargo_bin("thufs")
        .unwrap()
        .env("THUFS_CONFIG_DIR", temp.path())
        .env_remove("THUFS_TOKEN")
        .args(["--json", "update", "check"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .get_output()
        .stdout
        .clone();
    let result: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(result["current_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(result["latest_version"], "999.0.0");
    assert_eq!(result["update_available"], true);
    assert_eq!(result["cached"], true);
    Command::cargo_bin("thufs")
        .unwrap()
        .env("THUFS_CONFIG_DIR", temp.path())
        .args(["update", "check"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Update available:"))
        .stdout(predicate::str::contains(
            "cargo install thufs --locked --force",
        ));
}

#[test]
fn update_help_explains_manual_refresh_and_version_is_available() {
    Command::cargo_bin("thufs")
        .unwrap()
        .args(["update", "check", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--force"));
    Command::cargo_bin("thufs")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}
