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
        .stdout(predicate::str::contains("upm"));
}

#[test]
fn help_lists_commands() {
    upm()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("install"))
        .stdout(predicate::str::contains("doctor"))
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
