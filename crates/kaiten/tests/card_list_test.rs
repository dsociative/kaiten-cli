use assert_cmd::Command;
use predicates::prelude::*;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const CARDS: &str = include_str!("fixtures/cards_list.json");
const USER_CURRENT: &str = include_str!("fixtures/users_current.json");

fn kaiten(config_dir: &std::path::Path, base_url: &str) -> Command {
    let mut cmd = Command::cargo_bin("kaiten").unwrap();
    cmd.env_remove("KAITEN_TOKEN")
        .env_remove("KAITEN_DOMAIN")
        .env_remove("KAITEN_BASE_URL")
        .env_remove("RUST_LOG")
        .env("KAITEN_CONFIG_DIR", config_dir)
        .env("KAITEN_BASE_URL", base_url)
        .env("KAITEN_TOKEN", "test-token")
        .env("NO_COLOR", "1");
    cmd
}

#[tokio::test(flavor = "multi_thread")]
async fn card_list_uses_board_flag_and_default_limit() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cards"))
        .and(header("Authorization", "Bearer test-token"))
        .and(query_param("board_id", "1826109"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARDS, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    let assert = kaiten(tmp.path(), &server.uri())
        .args(["card", "list", "--board", "1826109"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("67089469"), "{stdout}");
    assert!(stdout.contains("urgent bugfix"), "{stdout}");
    insta::assert_snapshot!("card_list", stdout);
}

#[tokio::test(flavor = "multi_thread")]
async fn card_list_default_excludes_archived() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cards"))
        .and(header("Authorization", "Bearer test-token"))
        .and(query_param("board_id", "1826109"))
        .and(query_param("archived", "false"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARDS, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["card", "list", "--board", "1826109"])
        .assert()
        .success();
}

#[tokio::test(flavor = "multi_thread")]
async fn card_list_falls_back_to_defaults_board() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cards"))
        .and(query_param("board_id", "1826109"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARDS, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("config.toml"),
        "domain = \"mycompany\"\ntoken = \"file-token\"\n\n[defaults]\nboard = 1826109\n",
    )
    .unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["card", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("test card from cli"));
}

#[tokio::test(flavor = "multi_thread")]
async fn card_list_falls_back_to_defaults_space() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cards"))
        .and(query_param("space_id", "810671"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARDS, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("config.toml"),
        "domain = \"mycompany\"\ntoken = \"file-token\"\n\n[defaults]\nspace = 810671\n",
    )
    .unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["card", "list"])
        .assert()
        .success();
}

#[tokio::test(flavor = "multi_thread")]
async fn card_list_without_scope_is_invalid_arg() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["card", "list"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("specify --board/--space"));
}

#[tokio::test(flavor = "multi_thread")]
async fn card_list_mine_resolves_current_user() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/current"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(USER_CURRENT, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/cards"))
        .and(query_param("board_id", "1826109"))
        .and(query_param("member_ids", "1068514"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARDS, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["card", "list", "--board", "1826109", "--mine"])
        .assert()
        .success();
}

#[tokio::test(flavor = "multi_thread")]
async fn card_list_passes_all_filters() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cards"))
        .and(query_param("space_id", "810671"))
        .and(query_param("column_id", "6308511"))
        .and(query_param("member_ids", "42"))
        .and(query_param("query", "bug"))
        .and(query_param("tag", "cli-test"))
        .and(query_param("type_id", "1"))
        .and(query_param("archived", "true"))
        .and(query_param("limit", "10"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args([
            "card",
            "list",
            "--space",
            "810671",
            "--column",
            "6308511",
            "--member",
            "42",
            "--query",
            "bug",
            "--tag",
            "cli-test",
            "--type",
            "1",
            "--archived",
            "--limit",
            "10",
        ])
        .assert()
        .success();
}

#[tokio::test(flavor = "multi_thread")]
async fn card_list_json() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cards"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARDS, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    let assert = kaiten(tmp.path(), &server.uri())
        .args(["card", "list", "--board", "1826109", "--json"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value.as_array().unwrap().len(), 2);
    assert_eq!(value[1]["asap"], true);
}
