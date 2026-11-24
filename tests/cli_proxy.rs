use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn proxy_modes_persist_for_cli_and_tui_and_inspection_redacts_credentials() {
    let temp = tempfile::tempdir().unwrap();
    for (arguments, mode) in [
        (
            vec![
                "config",
                "proxy",
                "manual",
                "http://private-user:private-password@127.0.0.1:7890",
            ],
            "manual",
        ),
        (vec!["config", "proxy", "direct"], "direct"),
        (vec!["config", "proxy", "detect"], "auto"),
    ] {
        Command::cargo_bin("thufs")
            .unwrap()
            .env("THUFS_CONFIG_DIR", temp.path())
            .args(arguments)
            .assert()
            .success()
            .stdout(predicate::str::contains("private-user").not())
            .stdout(predicate::str::contains("private-password").not());
        let config: serde_json::Value =
            serde_json::from_slice(&std::fs::read(temp.path().join("config.json")).unwrap())
                .unwrap();
        assert_eq!(config["proxy"]["mode"], mode);
        Command::cargo_bin("thufs")
            .unwrap()
            .env("THUFS_CONFIG_DIR", temp.path())
            .args(["--json", "config", "show"])
            .assert()
            .success()
            .stdout(predicate::str::contains("\"proxy\""))
            .stdout(predicate::str::contains("private-user").not())
            .stdout(predicate::str::contains("private-password").not());
    }
}

#[test]
fn invalid_manual_proxies_leave_previous_settings_unchanged() {
    let temp = tempfile::tempdir().unwrap();
    Command::cargo_bin("thufs")
        .unwrap()
        .env("THUFS_CONFIG_DIR", temp.path())
        .args(["config", "proxy", "direct"])
        .assert()
        .success();
    let original = std::fs::read(temp.path().join("config.json")).unwrap();
    Command::cargo_bin("thufs")
        .unwrap()
        .env("THUFS_CONFIG_DIR", temp.path())
        .args([
            "config",
            "proxy",
            "manual",
            "file://private-user:private-password@host/data",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("private-password").not());
    assert_eq!(
        std::fs::read(temp.path().join("config.json")).unwrap(),
        original
    );
}
