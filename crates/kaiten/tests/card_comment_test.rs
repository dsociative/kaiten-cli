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
async fn comment_add_prints_created_id() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("POST"))
        .and(path("/cards/67089469/comments"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_json(serde_json::json!({"text": "hello from cli"})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/comment_created.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args([
            "card",
            "comment",
            "add",
            "67089469",
            "--body",
            "hello from cli",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("85523991"));
}

#[tokio::test(flavor = "multi_thread")]
async fn comment_add_json_prints_model() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("POST"))
        .and(path("/cards/67089469/comments"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/comment_created.json")),
        )
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args([
            "--json",
            "card",
            "comment",
            "add",
            "67089469",
            "--body",
            "hello from cli",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"text\": \"hello from cli\""));
}

#[tokio::test(flavor = "multi_thread")]
async fn comment_list_renders_table_with_truncated_text() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/cards/67089469/comments"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/comments_list_two.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["card", "comment", "list", "67089469"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ID"))
        .stdout(predicate::str::contains("AUTHOR"))
        .stdout(predicate::str::contains("CREATED"))
        .stdout(predicate::str::contains("TEXT"))
        .stdout(predicate::str::contains("85523991"))
        .stdout(predicate::str::contains("dxmuser"))
        .stdout(predicate::str::contains("2026-07-09"))
        .stdout(predicate::str::contains("test comment"))
        .stdout(predicate::str::contains("seconduser"))
        .stdout(predicate::str::contains("2026-07-10"))
        // 60 символов + "…": хвост исходного текста обрезан
        .stdout(predicate::str::contains(
            "deployment pipeline configuration must be updated together w…",
        ))
        .stdout(predicate::str::contains("the next release").not());
}

#[tokio::test(flavor = "multi_thread")]
async fn comment_list_json_prints_full_models() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/cards/67089469/comments"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/comments_list_two.json")),
        )
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["--json", "card", "comment", "list", "67089469"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"text\": \"test comment\""))
        .stdout(predicate::str::contains("the next release"));
}
