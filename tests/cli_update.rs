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

#[test]
fn tracked_cargo_installations_query_the_cargo_source_and_use_its_cache() {
    let temp = tempfile::tempdir().unwrap();
    let bin = temp.path().join("bin");
    let config = temp.path().join("config");
    std::fs::create_dir(&bin).unwrap();
    std::fs::create_dir(&config).unwrap();
    let name = format!("thufs{}", std::env::consts::EXE_SUFFIX);
    let installed = bin.join(&name);
    std::fs::copy(assert_cmd::cargo::cargo_bin("thufs"), &installed).unwrap();
    let package = format!(
        "thufs {} (registry+https://github.com/rust-lang/crates.io-index)",
        env!("CARGO_PKG_VERSION")
    );
    std::fs::write(
        temp.path().join(".crates2.json"),
        serde_json::json!({"installs": {(package): {"bins": [name]}}}).to_string(),
    )
    .unwrap();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    std::fs::write(
        config.join("update-check.json"),
        format!(r#"{{"checked_at":{now},"tag":"999.0.0","source":"crates.io"}}"#),
    )
    .unwrap();
    let output = Command::new(installed)
        .env("THUFS_CONFIG_DIR", config)
        .env_remove("THUFS_TOKEN")
        .args(["--json", "update", "check"])
        .assert()
        .success()
        .stderr(predicate::str::is_empty())
        .get_output()
        .stdout
        .clone();
    let result: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(result["source"], "crates.io");
    assert_eq!(result["latest_version"], "999.0.0");
    assert_eq!(
        result["release_url"],
        "https://crates.io/crates/thufs/999.0.0"
    );
    assert_eq!(result["cached"], true);
}
