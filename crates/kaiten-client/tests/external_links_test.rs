//! Card external links (`Links (common links)` in Kaiten), issue #21. Shapes
//! captured live from the API on 2026-08-27.

use kaiten_client::KaitenClient;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const LIST: &str = include_str!("fixtures/external_links_list.json");
const ADDED: &str = include_str!("fixtures/external_link_add.json");

#[tokio::test]
async fn list_parses_links_with_optional_description() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cards/67089469/external-links"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(LIST, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let links = client.external_links().list(67_089_469).await.unwrap();
    assert_eq!(links.len(), 2);
    assert_eq!(links[0].id, 21_177_131);
    assert_eq!(links[0].url, "https://example.com/spike");
    assert_eq!(links[0].description.as_deref(), Some("Source"));
    assert_eq!(
        links[0].uid.as_deref(),
        Some("b4efebe0-0000-0000-0000-000000000000")
    );
    assert_eq!(
        links[0].created.as_deref(),
        Some("2026-08-27T13:24:19.088Z")
    );
    assert_eq!(links[1].description, None);
}

#[tokio::test]
async fn add_posts_url_and_description_and_parses_the_link() {
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

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let link = client
        .external_links()
        .add(67_089_469, "https://example.com/spike", Some("Source"))
        .await
        .unwrap();
    assert_eq!(link.id, 21_177_131);
    assert_eq!(link.url, "https://example.com/spike");
}

/// Without a description only the url is sent — the API stores `null`.
#[tokio::test]
async fn add_without_description_sends_only_the_url() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cards/67089469/external-links"))
        .and(body_json(
            serde_json::json!({ "url": "https://example.net/nodesc" }),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_raw(ADDED, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    client
        .external_links()
        .add(67_089_469, "https://example.net/nodesc", None)
        .await
        .unwrap();
}

/// Updates are PATCH with only the changed fields (PUT answers 404).
#[tokio::test]
async fn update_patches_only_the_given_fields() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/cards/67089469/external-links/21177131"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_json(serde_json::json!({ "description": "patched" })))
        .respond_with(ResponseTemplate::new(200).set_body_raw(ADDED, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let link = client
        .external_links()
        .update(67_089_469, 21_177_131, None, Some("patched"))
        .await
        .unwrap();
    assert_eq!(link.id, 21_177_131);
}

#[tokio::test]
async fn remove_deletes_the_card_scoped_link() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/cards/67089469/external-links/21177131"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"id": 21177131}"#))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    client
        .external_links()
        .remove(67_089_469, 21_177_131)
        .await
        .unwrap();
}
