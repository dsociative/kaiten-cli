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
async fn card_tag_add_posts_name() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("POST"))
        .and(path("/cards/67089469/tags"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_json(serde_json::json!({"name": "cli-test"})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/tag_added.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["card", "tag", "add", "67089469", "cli-test"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "added tag cli-test (1110772) to card 67089469",
        ));
}

#[tokio::test(flavor = "multi_thread")]
async fn card_tag_remove_deletes_by_tag_id() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/cards/67089469"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/card_with_tags.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("DELETE"))
        .and(path("/cards/67089469/tags/1110772"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["card", "tag", "remove", "67089469", "cli-test"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "removed tag cli-test from card 67089469",
        ));
}

#[tokio::test(flavor = "multi_thread")]
async fn card_tag_remove_json_prints_removed_object() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/cards/67089469"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/card_with_tags.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("DELETE"))
        .and(path("/cards/67089469/tags/1110772"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["--json", "card", "tag", "remove", "67089469", "cli-test"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"removed\": true"))
        .stdout(predicate::str::contains("\"tag\": \"cli-test\""));
}

#[tokio::test(flavor = "multi_thread")]
async fn card_tag_remove_falls_back_to_link_id_when_no_tag_id() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/cards/67089469"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/card_with_tags.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("DELETE"))
        .and(path("/cards/67089469/tags/2220001"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["card", "tag", "remove", "67089469", "legacy-link"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "removed tag legacy-link from card 67089469",
        ));
}

#[tokio::test(flavor = "multi_thread")]
async fn card_tag_remove_unknown_name_lists_existing_tags() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/cards/67089469"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/card_with_tags.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["card", "tag", "remove", "67089469", "nope"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("has no tag `nope`"))
        .stderr(predicate::str::contains("cli-test, legacy-link"));
}

#[tokio::test(flavor = "multi_thread")]
async fn tag_list_renders_table() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/tags"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/tags_list.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["tag", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ID"))
        .stdout(predicate::str::contains("NAME"))
        .stdout(predicate::str::contains("1110772"))
        .stdout(predicate::str::contains("cli-test"))
        .stdout(predicate::str::contains("backend"));
}

#[tokio::test(flavor = "multi_thread")]
async fn tag_list_json_prints_models() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/tags"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/tags_list.json")),
        )
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["--json", "tag", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"name\": \"backend\""));
}
