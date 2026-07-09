use kaiten_client::{CardFilter, CreateCard, KaitenClient, UpdateCard};
use wiremock::matchers::{body_partial_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const CARDS_LIST: &str = include_str!("fixtures/cards_list.json");
const CARD_GET_FULL: &str = include_str!("fixtures/card_get_full.json");

#[test]
fn to_query_skips_none_and_joins_member_ids() {
    let filter = CardFilter {
        space_id: Some(810671),
        query: Some("bug".to_string()),
        member_ids: vec![1, 2],
        archived: Some(false),
        ..Default::default()
    };
    let q = filter.to_query();
    assert_eq!(
        q,
        vec![
            ("space_id".to_string(), "810671".to_string()),
            ("query".to_string(), "bug".to_string()),
            ("member_ids".to_string(), "1,2".to_string()),
            ("archived".to_string(), "false".to_string()),
        ]
    );
}

#[test]
fn to_query_is_empty_for_default_filter() {
    assert!(CardFilter::default().to_query().is_empty());
}

#[tokio::test]
async fn list_sends_filter_query_params_and_parses_list_card() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cards"))
        .and(header("Authorization", "Bearer test-token"))
        .and(query_param("board_id", "1826109"))
        .and(query_param("member_ids", "1068514,42"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARDS_LIST, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let filter = CardFilter {
        board_id: Some(1826109),
        member_ids: vec![1068514, 42],
        limit: Some(50),
        ..Default::default()
    };
    let cards = client.cards().list(&filter).await.unwrap();

    assert_eq!(cards.len(), 1);
    let card = &cards[0];
    assert_eq!(card.id, 67089469);
    assert_eq!(card.title, "test card from cli");
    assert_eq!(card.state, Some(1));
    assert_eq!(card.condition, Some(1));
    // list-карточка приходит БЕЗ description/members/checklists/tags
    assert!(card.description.is_none());
    assert!(card.members.is_empty());
    assert!(card.checklists.is_empty());
    assert!(card.tags.is_empty());
    // вложенный board без columns/lanes → пустые векторы
    let board = card.board.as_ref().unwrap();
    assert_eq!(board.id, 1826109);
    assert!(board.columns.is_empty());
    assert_eq!(card.column.as_ref().unwrap().column_type, Some(1));
    assert_eq!(card.card_type.as_ref().unwrap().name, "Card");
    assert_eq!(card.owner.as_ref().unwrap().email.as_deref(), Some("user@example.com"));
}

#[tokio::test]
async fn get_parses_full_card() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cards/67089469"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARD_GET_FULL, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let card = client.cards().get(67089469).await.unwrap();

    assert_eq!(card.id, 67089469);
    assert_eq!(card.description.as_deref(), Some("test **description**"));
    assert_eq!(card.asap, Some(true));
    assert_eq!(card.comments_total, Some(1));
    assert_eq!(card.members.len(), 1);
    assert_eq!(card.members[0].user_id, Some(1068514));
    assert_eq!(card.members[0].member_type, Some(1));
    assert_eq!(card.tags.len(), 1);
    assert_eq!(card.tags[0].name, "cli-test");
    assert_eq!(card.tags[0].tag_id, Some(1110772));
    assert_eq!(card.checklists.len(), 1);
    assert_eq!(card.checklists[0].name, "todo");
    assert_eq!(card.checklists[0].items.len(), 1);
    assert_eq!(card.checklists[0].items[0].text, "first item");
    assert_eq!(card.checklists[0].items[0].checked, Some(true));
}

const CARD_CREATE: &str = include_str!("fixtures/card_create.json");
const CARD_UPDATE: &str = include_str!("fixtures/card_update.json");

/// Matches only if the JSON body is an object WITHOUT the given key.
struct BodyLacksKey(&'static str);

impl wiremock::Match for BodyLacksKey {
    fn matches(&self, request: &wiremock::Request) -> bool {
        serde_json::from_slice::<serde_json::Value>(&request.body)
            .ok()
            .and_then(|v| v.as_object().map(|o| !o.contains_key(self.0)))
            .unwrap_or(false)
    }
}

#[tokio::test]
async fn create_sends_board_id_and_title_and_omits_none_fields() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cards"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_partial_json(serde_json::json!({
            "board_id": 1826109,
            "title": "test card from cli"
        })))
        .and(BodyLacksKey("description"))
        .and(BodyLacksKey("column_id"))
        .and(BodyLacksKey("lane_id"))
        .and(BodyLacksKey("type_id"))
        .and(BodyLacksKey("asap"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARD_CREATE, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let req = CreateCard {
        board_id: 1826109,
        title: "test card from cli".to_string(),
        ..Default::default()
    };
    let card = client.cards().create(&req).await.unwrap();

    assert_eq!(card.id, 67089469);
    assert_eq!(card.board_id, Some(1826109));
    assert_eq!(card.column_id, Some(6308511));
    // "description": null в ответе → None
    assert!(card.description.is_none());
    assert!(card.checklists.is_empty());
}

#[tokio::test]
async fn update_with_column_id_is_move_and_omits_other_fields() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/cards/67089469"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_partial_json(serde_json::json!({ "column_id": 6308512 })))
        .and(BodyLacksKey("title"))
        .and(BodyLacksKey("description"))
        .and(BodyLacksKey("board_id"))
        .and(BodyLacksKey("condition"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARD_UPDATE, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let req = UpdateCard {
        column_id: Some(6308512),
        ..Default::default()
    };
    let card = client.cards().update(67089469, &req).await.unwrap();

    assert_eq!(card.id, 67089469);
    assert_eq!(card.asap, Some(true));
    assert_eq!(card.description.as_deref(), Some("test **description**"));
}
