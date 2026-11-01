use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;

fn upm() -> Command {
    Command::cargo_bin("upm").expect("binary built")
}

#[test]
fn version_is_printed() {
    upm()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("upm 1.0.0"));
}

#[test]
fn help_lists_commands() {
    upm()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("install"))
        .stdout(predicate::str::contains("doctor"))
        .stdout(predicate::str::contains("outdated"))
        .stdout(predicate::str::contains("profile"))
        .stdout(predicate::str::contains("snapshot"))
        .stdout(predicate::str::contains("upgrade"));
}

#[test]
fn which_json_has_backends_array() {
    let output = upm().args(["which", "--json"]).output().unwrap();
    assert!(output.status.success(), "upm which --json failed");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(value["backends"].is_array());
    assert!(value["os"].is_string());
    assert!(value["arch"].is_string());
}

#[test]
fn completions_are_emitted() {
    upm()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("upm"));
}

#[test]
fn config_show_is_toml() {
    upm()
        .args(["config", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("[general]"));
}

#[test]
fn rejects_flag_like_package_names() {
    upm()
        .args(["install", "--dry-run", "--", "--force"])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn unknown_backend_is_a_usage_error() {
    upm()
        .args(["which", "--backend", "opkg"])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn capabilities_json_is_a_matrix() {
    let output = upm().args(["capabilities", "--json"]).output().unwrap();
    assert!(output.status.success(), "upm capabilities --json failed");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(value["backends"].is_array());
    assert!(value["backends"].as_array().unwrap().len() >= 8);
}

#[test]
fn history_and_snapshot_listing_start_empty() {
    let state = tempfile::tempdir().unwrap();
    let history = upm()
        .env("UPM_STATE_DIR", state.path())
        .args(["history", "--json"])
        .output()
        .unwrap();
    assert!(history.status.success());
    let value: serde_json::Value = serde_json::from_slice(&history.stdout).unwrap();
    assert!(value["entries"].as_array().unwrap().is_empty());

    let snapshots = upm()
        .env("UPM_STATE_DIR", state.path())
        .args(["snapshot", "list", "--json"])
        .output()
        .unwrap();
    assert!(snapshots.status.success());
    let value: serde_json::Value = serde_json::from_slice(&snapshots.stdout).unwrap();
    assert!(value["snapshots"].as_array().unwrap().is_empty());
}

#[test]
fn profile_lifecycle_round_trips() {
    let state = tempfile::tempdir().unwrap();
    let profile = |args: &[&str]| {
        upm()
            .env("UPM_STATE_DIR", state.path())
            .args(args)
            .output()
            .unwrap()
    };

    assert!(profile(&["profile", "create", "dev"]).status.success());
    assert!(
        profile(&["profile", "add", "dev", "git", "jq"])
            .status
            .success()
    );
    let shown = profile(&["profile", "show", "dev", "--json"]);
    assert!(shown.status.success());
    let value: serde_json::Value = serde_json::from_slice(&shown.stdout).unwrap();
    assert_eq!(value["packages"]["git"], "*");
    assert_eq!(value["packages"]["jq"], "*");

    let list = profile(&["profile", "list", "--json"]);
    assert!(list.status.success());
    let value: serde_json::Value = serde_json::from_slice(&list.stdout).unwrap();
    assert_eq!(value["profiles"][0]["name"], "dev");
    assert_eq!(value["profiles"][0]["packages"], 2);
}
