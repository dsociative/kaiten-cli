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
async fn checklist_list_prints_items_with_marks() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/cards/67089469"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/card_with_checklists.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["card", "checklist", "list", "67089469"])
        .assert()
        .success()
        .stdout(predicate::str::contains("todo (11747430)"))
        .stdout(predicate::str::contains("[x] 65658564 first item"))
        .stdout(predicate::str::contains("[ ] 65658565 second item"));
}

#[tokio::test(flavor = "multi_thread")]
async fn checklist_list_json_prints_checklists_array() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/cards/67089469"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/card_with_checklists.json")),
        )
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["--json", "card", "checklist", "list", "67089469"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"name\": \"todo\""))
        .stdout(predicate::str::contains("\"text\": \"second item\""));
}

#[tokio::test(flavor = "multi_thread")]
async fn checklist_add_posts_name() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("POST"))
        .and(path("/cards/67089469/checklists"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_json(serde_json::json!({"name": "todo"})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/checklist_created.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["card", "checklist", "add", "67089469", "--name", "todo"])
        .assert()
        .success()
        .stdout(predicate::str::contains("created checklist 11747430"));
}

#[tokio::test(flavor = "multi_thread")]
async fn checklist_item_add_posts_text() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("POST"))
        .and(path("/cards/67089469/checklists/11747430/items"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_json(serde_json::json!({"text": "first item"})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/checklist_item_created.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args([
            "card", "checklist", "item", "add", "67089469", "11747430", "--text", "first item",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("created item 65658564"));
}

#[tokio::test(flavor = "multi_thread")]
async fn checklist_item_check_sends_checked_true() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("PATCH"))
        .and(path("/cards/67089469/checklists/11747430/items/65658564"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_json(serde_json::json!({"checked": true})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/checklist_item_checked.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args([
            "card", "checklist", "item", "check", "67089469", "11747430", "65658564",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("item 65658564 checked"));
}

#[tokio::test(flavor = "multi_thread")]
async fn checklist_item_uncheck_sends_checked_false() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("PATCH"))
        .and(path("/cards/67089469/checklists/11747430/items/65658564"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_json(serde_json::json!({"checked": false})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/checklist_item_unchecked.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args([
            "card", "checklist", "item", "uncheck", "67089469", "11747430", "65658564",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("item 65658564 unchecked"));
}
