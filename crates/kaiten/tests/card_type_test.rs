use assert_cmd::Command;
use predicates::prelude::*;
use wiremock::matchers::{header, method, path};
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
async fn card_type_list_renders_table() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/card-types"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/card_types_list.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["card-type", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ID"))
        .stdout(predicate::str::contains("NAME"))
        .stdout(predicate::str::contains("LETTER"))
        .stdout(predicate::str::contains("692717"))
        .stdout(predicate::str::contains("Feature"))
        .stdout(predicate::str::contains("Bug"));
}

#[tokio::test(flavor = "multi_thread")]
async fn card_type_list_json_prints_models() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/card-types"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/card_types_list.json")),
        )
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["--json", "card-type", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"letter\": \"F\""));
}
