use assert_cmd::Command;
use predicates::prelude::*;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const CARD: &str = include_str!("fixtures/card_get_full.json");
const CARD_NO_PROPERTIES: &str = include_str!("fixtures/card_get_no_properties.json");
const COMMENTS: &str = include_str!("fixtures/comments_list.json");

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

async fn mock_card(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/cards/67089469"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARD, "application/json"))
        .expect(1)
        .mount(server)
        .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn card_view_by_id_prints_details() {
    let server = MockServer::start().await;
    mock_card(&server).await;
    let tmp = tempfile::tempdir().unwrap();

    let assert = kaiten(tmp.path(), &server.uri())
        .args(["card", "view", "67089469"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("[x] first item"), "{stdout}");
    assert!(stdout.contains("[ ] second item"), "{stdout}");
    assert!(stdout.contains("test **description**"), "{stdout}");
    assert!(stdout.contains("Properties:"), "{stdout}");
    assert!(stdout.contains("\"id_19\": \"S\""), "{stdout}");
    insta::assert_snapshot!("card_view", stdout);
}

#[tokio::test(flavor = "multi_thread")]
async fn card_view_by_url() {
    let server = MockServer::start().await;
    mock_card(&server).await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args([
            "card",
            "view",
            "https://mycompany.kaiten.ru/space/810671/card/67089469",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("#67089469 test card from cli"));
}

#[tokio::test(flavor = "multi_thread")]
async fn card_view_with_comments_makes_second_request() {
    let server = MockServer::start().await;
    mock_card(&server).await;
    Mock::given(method("GET"))
        .and(path("/cards/67089469/comments"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(COMMENTS, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["card", "view", "67089469", "--comments"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Comments:"))
        .stdout(predicate::str::contains("test comment"))
        .stdout(predicate::str::contains("2026-07-09 dxmuser:"));
}

#[tokio::test(flavor = "multi_thread")]
async fn card_view_without_properties_hides_block() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cards/67089470"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(CARD_NO_PROPERTIES, "application/json"),
        )
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["card", "view", "67089470"])
        .assert()
        .success()
        .stdout(predicate::str::contains("#67089470 test card from cli"))
        .stdout(predicate::str::contains("Properties:").not());
}

#[tokio::test(flavor = "multi_thread")]
async fn card_view_json() {
    let server = MockServer::start().await;
    mock_card(&server).await;
    let tmp = tempfile::tempdir().unwrap();

    let assert = kaiten(tmp.path(), &server.uri())
        .args(["card", "view", "67089469", "--json"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["id"], 67_089_469);
    assert_eq!(value["checklists"][0]["items"][0]["checked"], true);
    assert_eq!(value["properties"]["id_19"], "S");
}

#[tokio::test(flavor = "multi_thread")]
async fn card_view_garbage_ref_fails() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["card", "view", "definitely-not-a-card"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("invalid card reference"));
}

// --- issue #21: `--include` sections and the deprecated `--comments` ---

const EXTERNAL_LINKS: &str = include_str!("fixtures/external_links_list.json");

async fn mock_external_links(server: &MockServer, expect: u64) {
    Mock::given(method("GET"))
        .and(path("/cards/67089469/external-links"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(EXTERNAL_LINKS, "application/json"))
        .expect(expect)
        .mount(server)
        .await;
}

async fn mock_comments(server: &MockServer, expect: u64) {
    Mock::given(method("GET"))
        .and(path("/cards/67089469/comments"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(COMMENTS, "application/json"))
        .expect(expect)
        .mount(server)
        .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn card_view_include_external_links_makes_second_request_and_prints_section() {
    let server = MockServer::start().await;
    mock_card(&server).await;
    mock_external_links(&server, 1).await;
    mock_comments(&server, 0).await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["card", "view", "67089469", "--include", "external_links"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Links:"))
        .stdout(predicate::str::contains("21177131"))
        .stdout(predicate::str::contains("https://example.com/spike"))
        .stdout(predicate::str::contains("Source"))
        .stdout(predicate::str::contains("Comments:").not());
}

#[tokio::test(flavor = "multi_thread")]
async fn card_view_include_both_sections_comma_separated() {
    let server = MockServer::start().await;
    mock_card(&server).await;
    mock_external_links(&server, 1).await;
    mock_comments(&server, 1).await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args([
            "card",
            "view",
            "67089469",
            "--include",
            "external_links,comments",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Links:"))
        .stdout(predicate::str::contains("Comments:"))
        .stderr(predicate::str::contains("deprecated").not());
}

/// `--comments` keeps working but says what to use instead — on stderr, so
/// stdout consumers are unaffected.
#[tokio::test(flavor = "multi_thread")]
async fn card_view_comments_flag_is_deprecated_but_still_works() {
    let server = MockServer::start().await;
    mock_card(&server).await;
    mock_comments(&server, 1).await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["card", "view", "67089469", "--comments"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Comments:"))
        .stderr(predicate::str::contains("--comments is deprecated"))
        .stderr(predicate::str::contains("--include comments"));
}

#[tokio::test(flavor = "multi_thread")]
async fn card_view_json_include_external_links_adds_the_key() {
    let server = MockServer::start().await;
    mock_card(&server).await;
    mock_external_links(&server, 1).await;
    let tmp = tempfile::tempdir().unwrap();

    let out = kaiten(tmp.path(), &server.uri())
        .args([
            "--json",
            "card",
            "view",
            "67089469",
            "--include",
            "external_links",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(value["card"]["id"], 67_089_469, "{value}");
    assert_eq!(value["external_links"][0]["id"], 21_177_131, "{value}");
    assert!(value.get("comments").is_none(), "{value}");
}

#[tokio::test(flavor = "multi_thread")]
async fn card_view_include_rejects_an_unknown_section() {
    let server = MockServer::start().await; // nothing may be requested
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["card", "view", "67089469", "--include", "attachments"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("external_links"))
        .stderr(predicate::str::contains("comments"));
    assert!(server.received_requests().await.unwrap().is_empty());
}
