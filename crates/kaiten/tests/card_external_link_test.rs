//! `kaiten card external-link list|add|edit|rm` (issue #21).

use assert_cmd::Command;
use predicates::prelude::*;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const LIST: &str = include_str!("fixtures/external_links_list.json");
const ADDED: &str = include_str!("fixtures/external_link_add.json");

fn kaiten(base_url: &str, config_dir: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("kaiten").unwrap();
    cmd.env_remove("KAITEN_DOMAIN")
        .env("KAITEN_BASE_URL", base_url)
        .env("KAITEN_TOKEN", "test-token")
        .env("KAITEN_CONFIG_DIR", config_dir)
        .env("NO_COLOR", "1");
    cmd
}

#[tokio::test(flavor = "multi_thread")]
async fn list_renders_table() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cards/67089469/external-links"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(LIST, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(&server.uri(), tmp.path())
        .args(["card", "external-link", "list", "67089469"])
        .assert()
        .success()
        .stdout(predicate::str::contains("21177131"))
        .stdout(predicate::str::contains("https://example.com/spike"))
        .stdout(predicate::str::contains("Source"))
        .stdout(predicate::str::contains("https://example.net/nodesc"))
        .stdout(predicate::str::contains("2026-08-27"));
}

#[tokio::test(flavor = "multi_thread")]
async fn list_json_prints_models() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cards/67089469/external-links"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(LIST, "application/json"))
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    let out = kaiten(&server.uri(), tmp.path())
        .args(["--json", "card", "external-link", "list", "67089469"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(value.as_array().unwrap().len(), 2);
    assert_eq!(value[0]["url"], "https://example.com/spike");
    assert!(value[1]["description"].is_null());
}

#[tokio::test(flavor = "multi_thread")]
async fn add_posts_url_and_description_and_prints_id() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cards/67089469/external-links"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_json(serde_json::json!({
            "url": "https://example.com/spike",
            "description": "Source"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_raw(ADDED, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(&server.uri(), tmp.path())
        .args([
            "card",
            "external-link",
            "add",
            "67089469",
            "--url",
            "https://example.com/spike",
            "--description",
            "Source",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("21177131"));
}

#[tokio::test(flavor = "multi_thread")]
async fn add_json_prints_model() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cards/67089469/external-links"))
        .and(body_json(
            serde_json::json!({ "url": "https://example.com/spike" }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_raw(ADDED, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    let out = kaiten(&server.uri(), tmp.path())
        .args([
            "--json",
            "card",
            "external-link",
            "add",
            "67089469",
            "--url",
            "https://example.com/spike",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(value["id"], 21_177_131);
}

/// The API stores anything as a url; the CLI refuses garbage before sending.
#[tokio::test(flavor = "multi_thread")]
async fn add_rejects_a_non_http_url_without_any_request() {
    let server = MockServer::start().await; // no mocks
    let tmp = tempfile::tempdir().unwrap();

    for bad in ["notaurl", "ftp://host/x", "https://user:secret@host/x", ""] {
        kaiten(&server.uri(), tmp.path())
            .args(["card", "external-link", "add", "67089469", "--url", bad])
            .assert()
            .failure()
            .code(1)
            .stderr(predicate::str::contains("--url"))
            .stderr(predicate::str::contains("secret").not());
    }
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn edit_patches_only_the_given_fields() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/cards/67089469/external-links/21177131"))
        .and(body_json(
            serde_json::json!({ "description": "Updated source" }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_raw(ADDED, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(&server.uri(), tmp.path())
        .args([
            "card",
            "external-link",
            "edit",
            "67089469",
            "21177131",
            "--description",
            "Updated source",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "updated external link 21177131 on card 67089469",
        ));
}

#[tokio::test(flavor = "multi_thread")]
async fn edit_without_changes_is_an_error_without_any_request() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(&server.uri(), tmp.path())
        .args(["card", "external-link", "edit", "67089469", "21177131"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("--url"))
        .stderr(predicate::str::contains("--description"));
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn rm_deletes_and_reports() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/cards/67089469/external-links/21177131"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"id": 21177131}"#))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(&server.uri(), tmp.path())
        .args(["card", "external-link", "rm", "67089469", "21177131"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "removed external link 21177131 from card 67089469",
        ));
}

/// Kaiten answers 403 for a link id that is not on the card.
#[tokio::test(flavor = "multi_thread")]
async fn rm_unknown_link_surfaces_the_api_error() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/cards/67089469/external-links/1"))
        .respond_with(ResponseTemplate::new(403).set_body_string("Forbidden"))
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(&server.uri(), tmp.path())
        .args(["card", "external-link", "rm", "67089469", "1"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("API error 403"));
}
