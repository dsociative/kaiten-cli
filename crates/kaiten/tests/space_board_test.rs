use assert_cmd::Command;
use predicates::prelude::*;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SPACES: &str = include_str!("fixtures/spaces_list.json");
const BOARDS: &str = include_str!("fixtures/boards_list.json");
const BOARD: &str = include_str!("fixtures/board_get.json");

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

async fn mock_get(server: &MockServer, url_path: &str, body: &str) {
    Mock::given(method("GET"))
        .and(path(url_path))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .expect(1)
        .mount(server)
        .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn space_list_prints_table() {
    let server = MockServer::start().await;
    mock_get(&server, "/spaces", SPACES).await;
    let tmp = tempfile::tempdir().unwrap();

    let assert = kaiten(tmp.path(), &server.uri())
        .args(["space", "list"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    insta::assert_snapshot!("space_list", stdout);
}

#[tokio::test(flavor = "multi_thread")]
async fn space_list_json() {
    let server = MockServer::start().await;
    mock_get(&server, "/spaces", SPACES).await;
    let tmp = tempfile::tempdir().unwrap();

    let assert = kaiten(tmp.path(), &server.uri())
        .args(["space", "list", "--json"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value[0]["id"], 810_671);
    assert_eq!(value[0]["title"], "kaiten-cli-test");
}

#[tokio::test(flavor = "multi_thread")]
async fn board_list_requires_space() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["board", "list"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("specify --space"));
}

#[tokio::test(flavor = "multi_thread")]
async fn board_list_with_flag() {
    let server = MockServer::start().await;
    mock_get(&server, "/spaces/810671/boards", BOARDS).await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["board", "list", "--space", "810671"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1826109"))
        .stdout(predicate::str::contains("test-board"));
}

#[tokio::test(flavor = "multi_thread")]
async fn board_list_uses_default_space_from_config() {
    let server = MockServer::start().await;
    mock_get(&server, "/spaces/810671/boards", BOARDS).await;
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("config.toml"),
        "domain = \"mycompany\"\ntoken = \"file-token\"\n\n[defaults]\nspace = 810671\n",
    )
    .unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["board", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("test-board"));
}

#[tokio::test(flavor = "multi_thread")]
async fn board_view_prints_columns_and_lanes() {
    let server = MockServer::start().await;
    mock_get(&server, "/boards/1826109", BOARD).await;
    let tmp = tempfile::tempdir().unwrap();

    let assert = kaiten(tmp.path(), &server.uri())
        .args(["board", "view", "1826109"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("queued"), "{stdout}");
    assert!(stdout.contains("in progress"), "{stdout}");
    assert!(stdout.contains("done"), "{stdout}");
    insta::assert_snapshot!("board_view", stdout);
}

#[tokio::test(flavor = "multi_thread")]
async fn board_view_json() {
    let server = MockServer::start().await;
    mock_get(&server, "/boards/1826109", BOARD).await;
    let tmp = tempfile::tempdir().unwrap();

    let assert = kaiten(tmp.path(), &server.uri())
        .args(["board", "view", "1826109", "--json"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["id"], 1_826_109);
    assert_eq!(value["columns"].as_array().unwrap().len(), 3);
    assert_eq!(value["lanes"].as_array().unwrap().len(), 1);
}
