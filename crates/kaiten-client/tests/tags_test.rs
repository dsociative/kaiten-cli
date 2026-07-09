use kaiten_client::KaitenClient;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TAGS_LIST: &str = include_str!("fixtures/tags_list.json");
const CARD_TAG_ADD: &str = include_str!("fixtures/card_tag_add.json");
const CARD_TYPES: &str = include_str!("fixtures/card_types.json");

#[tokio::test]
async fn list_parses_company_tags() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/tags"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(TAGS_LIST, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let tags = client.tags().list().await.unwrap();

    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].id, 1_110_772);
    assert_eq!(tags[0].name, "cli-test");
    assert_eq!(tags[0].color, Some(15));
}

#[tokio::test]
async fn add_to_card_posts_tag_name() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cards/67089469/tags"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_json(serde_json::json!({ "name": "cli-test" })))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARD_TAG_ADD, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let tag = client
        .tags()
        .add_to_card(67_089_469, "cli-test")
        .await
        .unwrap();

    assert_eq!(tag.id, 1_110_772);
    assert_eq!(tag.name, "cli-test");
}

#[tokio::test]
async fn remove_from_card_returns_ok_on_empty_body() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/cards/67089469/tags/1110772"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    client
        .tags()
        .remove_from_card(67_089_469, 1_110_772)
        .await
        .unwrap();
}

#[tokio::test]
async fn card_types_parses_type_list() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/card-types"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARD_TYPES, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let types = client.tags().card_types().await.unwrap();

    assert_eq!(types.len(), 2);
    assert_eq!(types[0].name, "Card");
    assert_eq!(types[1].name, "Bug");
    assert_eq!(types[1].letter.as_deref(), Some("B"));
    assert_eq!(types[1].archived, Some(false));
}
