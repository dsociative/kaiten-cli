use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn help_lists_all_subcommands() {
    let mut cmd = Command::cargo_bin("kaiten").unwrap();
    cmd.env("NO_COLOR", "1");
    let assert = cmd.arg("--help").assert().success();
    let out = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    for sub in [
        "auth",
        "space",
        "board",
        "card",
        "card-type",
        "tag",
        "api",
        "completion",
        "mcp",
    ] {
        assert!(out.contains(sub), "help must mention `{sub}`:\n{out}");
    }
}

#[test]
fn card_list_without_config_fails_with_no_token() {
    let tmp = tempfile::tempdir().unwrap();
    let mut cmd = Command::cargo_bin("kaiten").unwrap();
    cmd.env_remove("KAITEN_TOKEN")
        .env_remove("KAITEN_DOMAIN")
        .env_remove("KAITEN_BASE_URL")
        .env_remove("RUST_LOG")
        .env("KAITEN_CONFIG_DIR", tmp.path())
        .env("NO_COLOR", "1");
    cmd.args(["card", "list"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("no token"));
}
