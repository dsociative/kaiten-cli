use assert_cmd::Command;
use predicates::prelude::*;

fn kaiten_no_config() -> Command {
    let tmp = tempfile::tempdir().unwrap();
    let mut cmd = Command::cargo_bin("kaiten").unwrap();
    cmd.env("KAITEN_CONFIG_DIR", tmp.path())
        .env("NO_COLOR", "1")
        .env_remove("KAITEN_TOKEN")
        .env_remove("KAITEN_DOMAIN")
        .env_remove("KAITEN_BASE_URL");
    // tempdir удалится по выходу из функции — для completion конфиг всё равно не читается
    cmd
}

#[test]
fn completion_zsh_contains_function_and_subcommands() {
    kaiten_no_config()
        .args(["completion", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("_kaiten"))
        .stdout(predicate::str::contains("card"))
        .stdout(predicate::str::contains("board"))
        .stdout(predicate::str::contains("completion"));
}

#[test]
fn completion_bash_contains_function() {
    kaiten_no_config()
        .args(["completion", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("_kaiten"));
}

#[test]
fn completion_fish_mentions_binary() {
    kaiten_no_config()
        .args(["completion", "fish"])
        .assert()
        .success()
        .stdout(predicate::str::contains("kaiten"));
}

#[test]
fn completion_rejects_unknown_shell() {
    kaiten_no_config()
        .args(["completion", "powershell"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value"));
}
