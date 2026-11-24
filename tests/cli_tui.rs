use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn tui_help_explains_local_directory_demo_and_mouse_options() {
    Command::cargo_bin("thufs")
        .unwrap()
        .args(["tui", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--local"))
        .stdout(predicate::str::contains("--demo"))
        .stdout(predicate::str::contains("--no-mouse"));
}

#[test]
fn tui_rejects_json_before_entering_the_terminal() {
    Command::cargo_bin("thufs")
        .unwrap()
        .args(["--json", "tui"])
        .assert()
        .failure()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("does not support --json"));
}

#[test]
fn tui_requires_interactive_stdin_and_stdout_even_in_demo_mode() {
    Command::cargo_bin("thufs")
        .unwrap()
        .args(["tui", "--demo"])
        .assert()
        .failure()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("requires an interactive terminal"));
}

#[test]
fn root_help_includes_the_interactive_workspace() {
    Command::cargo_bin("thufs")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("tui"));
}

#[test]
fn tui_help_describes_settings_and_token_login_without_the_old_slash_prompt() {
    Command::cargo_bin("thufs")
        .unwrap()
        .args(["tui", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Ctrl+P"))
        .stdout(predicate::str::contains(
            "language, User-Agent, and account settings",
        ))
        .stdout(predicate::str::contains("Sign in with a token"))
        .stdout(predicate::str::contains("Press / for commands").not());
}
