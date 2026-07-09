use assert_cmd::Command;
use predicates::prelude::*;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn kaiten(base_url: &str, config_dir: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("kaiten").unwrap();
    cmd.env("KAITEN_BASE_URL", base_url)
        .env("KAITEN_TOKEN", "test-token")
        .env("KAITEN_CONFIG_DIR", config_dir)
        .env("NO_COLOR", "1");
    cmd
}

#[tokio::test(flavor = "multi_thread")]
async fn api_get_prints_pretty_json_and_accepts_lowercase_method() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/users/current"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/api_user_current.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    let assert = kaiten(&server.uri(), tmp.path())
        .args(["api", "get", "/users/current"])
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    // stdout — валидный JSON
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["id"], 1_068_514);
    // и он pretty-printed (двухпробельный отступ to_string_pretty)
    assert!(stdout.contains("  \"id\": 1068514"), "stdout: {stdout}");
}

#[tokio::test(flavor = "multi_thread")]
async fn api_post_sends_data_as_json_body() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("POST"))
        .and(path("/cards"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_json(serde_json::json!({
            "board_id": 1_826_109,
            "title": "from raw api"
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/api_card_created.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args([
            "api",
            "POST",
            "/cards",
            "--data",
            "{\"board_id\":1826109,\"title\":\"from raw api\"}",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"id\": 67089469"))
        .stdout(predicate::str::contains("\"title\": \"from raw api\""));
}

#[tokio::test(flavor = "multi_thread")]
async fn api_unsupported_method_exits_with_error() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(&server.uri(), tmp.path())
        .args(["api", "FETCH", "/users/current"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unsupported method"));
}

#[tokio::test(flavor = "multi_thread")]
async fn api_garbage_data_exits_with_json_error() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(&server.uri(), tmp.path())
        .args(["api", "POST", "/cards", "--data", "{not json"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("kaiten: json:"));
}

#[tokio::test(flavor = "multi_thread")]
async fn api_ignores_global_json_flag() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/users/current"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/api_user_current.json")),
        )
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["--json", "api", "GET", "/users/current"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"id\": 1068514"));
}
