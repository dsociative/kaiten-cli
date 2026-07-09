use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::json;
use wiremock::matchers::{body_json, body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const CARD_CREATE: &str = include_str!("fixtures/card_create.json");
const CARD_UPDATE: &str = include_str!("fixtures/card_update.json");
const CARD_ARCHIVE: &str = include_str!("fixtures/card_archive.json");

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
async fn create_sends_board_and_title() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cards"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_partial_json(json!({
            "board_id": 1826109,
            "title": "new card"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARD_CREATE, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args([
            "card", "create", "--board", "1826109", "--title", "new card",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("67089469"))
        .stdout(predicate::str::contains("new card"));
}

#[tokio::test(flavor = "multi_thread")]
async fn create_sends_optional_fields() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cards"))
        .and(body_partial_json(json!({
            "board_id": 1826109,
            "title": "new card",
            "column_id": 6308511,
            "lane_id": 2293584,
            "description": "body",
            "type_id": 1,
            "asap": true
        })))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARD_CREATE, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args([
            "card",
            "create",
            "--board",
            "1826109",
            "--title",
            "new card",
            "--column",
            "6308511",
            "--lane",
            "2293584",
            "--description",
            "body",
            "--type",
            "1",
            "--asap",
        ])
        .assert()
        .success();
}

#[tokio::test(flavor = "multi_thread")]
async fn create_uses_defaults_board() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cards"))
        .and(body_partial_json(
            json!({"board_id": 1826109, "title": "new card"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARD_CREATE, "application/json"))
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
        .args(["card", "create", "--title", "new card"])
        .assert()
        .success();
}

#[tokio::test(flavor = "multi_thread")]
async fn create_without_board_or_defaults_fails() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["card", "create", "--title", "new card"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("specify --board"));
}

#[tokio::test(flavor = "multi_thread")]
async fn edit_sends_patch_body() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/cards/67089469"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_partial_json(json!({
            "asap": true,
            "description": "test **description**"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARD_UPDATE, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args([
            "card",
            "edit",
            "67089469",
            "--asap",
            "true",
            "--description",
            "test **description**",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("67089469"));
}

#[tokio::test(flavor = "multi_thread")]
async fn edit_asap_false_is_sent_explicitly() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/cards/67089469"))
        .and(body_json(json!({"asap": false})))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARD_UPDATE, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["card", "edit", "67089469", "--asap", "false"])
        .assert()
        .success();
}

#[tokio::test(flavor = "multi_thread")]
async fn edit_without_changes_fails() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["card", "edit", "67089469"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("nothing to edit"));
}

#[tokio::test(flavor = "multi_thread")]
async fn move_sends_exactly_column_id() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/cards/67089469"))
        .and(body_json(json!({"column_id": 6308512})))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARD_UPDATE, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["card", "move", "67089469", "--column", "6308512"])
        .assert()
        .success();
}

#[tokio::test(flavor = "multi_thread")]
async fn move_with_lane_and_board() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/cards/67089469"))
        .and(body_json(json!({
            "column_id": 6308512,
            "lane_id": 2293584,
            "board_id": 1826109
        })))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARD_UPDATE, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args([
            "card", "move", "67089469", "--column", "6308512", "--lane", "2293584", "--board",
            "1826109",
        ])
        .assert()
        .success();
}

#[tokio::test(flavor = "multi_thread")]
async fn archive_sends_condition_2() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/cards/67089469"))
        .and(body_json(json!({"condition": 2})))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARD_ARCHIVE, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["card", "archive", "67089469"])
        .assert()
        .success()
        .stdout(predicate::str::contains("67089469"));
}

#[tokio::test(flavor = "multi_thread")]
async fn create_json_output() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cards"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARD_CREATE, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    let assert = kaiten(tmp.path(), &server.uri())
        .args([
            "card", "create", "--board", "1826109", "--title", "new card", "--json",
        ])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["id"], 67089469);
    assert_eq!(value["title"], "new card");
}

#[tokio::test(flavor = "multi_thread")]
async fn create_api_error_prints_message_and_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cards"))
        .respond_with(ResponseTemplate::new(400).set_body_raw(
            r#"{"message":"Card should have required property 'board_id'"}"#,
            "application/json",
        ))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args([
            "card", "create", "--board", "1826109", "--title", "new card",
        ])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("400"))
        .stderr(predicate::str::contains(
            "Card should have required property 'board_id'",
        ))
        .stderr(predicate::str::contains(
            r#"{"message":"Card should have required property 'board_id'"}"#,
        ));
}
