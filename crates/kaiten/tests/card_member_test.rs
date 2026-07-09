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
async fn member_add_by_id_posts_user_id() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("POST"))
        .and(path("/cards/67089469/members"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_json(serde_json::json!({"user_id": 1068514})))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(include_str!("fixtures/member_added.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["card", "member", "add", "67089469", "1068514"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "added user 1068514 to card 67089469",
        ));
}

#[tokio::test(flavor = "multi_thread")]
async fn member_add_by_email_resolves_via_users_list() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/users"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(include_str!("fixtures/member_users.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/cards/67089469/members"))
        .and(body_json(serde_json::json!({"user_id": 555001})))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(include_str!("fixtures/member_added.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    // card задан URL-ом — заодно проверяем parse_card_ref
    kaiten(&server.uri(), tmp.path())
        .args([
            "card",
            "member",
            "add",
            "https://mycompany.kaiten.ru/space/810671/card/67089469",
            "second@example.com",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "added user 555001 to card 67089469",
        ));
}

#[tokio::test(flavor = "multi_thread")]
async fn member_add_json_prints_model() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("POST"))
        .and(path("/cards/67089469/members"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(include_str!("fixtures/member_added.json")),
        )
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["--json", "card", "member", "add", "67089469", "1068514"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"user_id\": 1068514"));
}

#[tokio::test(flavor = "multi_thread")]
async fn member_add_unknown_email_fails_with_invalid_arg() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/users"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(include_str!("fixtures/member_users.json")),
        )
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["card", "member", "add", "67089469", "ghost@example.com"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "no user with email `ghost@example.com`",
        ));
}

#[tokio::test(flavor = "multi_thread")]
async fn member_add_garbage_user_fails_with_invalid_arg() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(&server.uri(), tmp.path())
        .args(["card", "member", "add", "67089469", "bob"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid user `bob`"));
}

#[tokio::test(flavor = "multi_thread")]
async fn member_remove_sends_delete() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("DELETE"))
        .and(path("/cards/67089469/members/1068514"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["card", "member", "remove", "67089469", "1068514"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "removed user 1068514 from card 67089469",
        ));
}

#[tokio::test(flavor = "multi_thread")]
async fn member_remove_json_prints_removed_object() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("DELETE"))
        .and(path("/cards/67089469/members/1068514"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["--json", "card", "member", "remove", "67089469", "1068514"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"removed\": true"))
        .stdout(predicate::str::contains("\"user_id\": 1068514"));
}
