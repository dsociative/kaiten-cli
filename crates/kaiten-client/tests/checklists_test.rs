use kaiten_client::KaitenClient;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const CHECKLIST_ADD: &str = include_str!("fixtures/checklist_add.json");
const CHECKLIST_ITEM_ADD: &str = include_str!("fixtures/checklist_item_add.json");
const CHECKLIST_ITEM_CHECK: &str = include_str!("fixtures/checklist_item_check.json");

#[tokio::test]
async fn add_posts_name_and_parses_checklist_without_items_key() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cards/67089469/checklists"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_json(serde_json::json!({ "name": "todo" })))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CHECKLIST_ADD, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let checklist = client.checklists().add(67_089_469, "todo").await.unwrap();

    assert_eq!(checklist.id, 11_747_430);
    assert_eq!(checklist.name, "todo");
    // ответ без ключа items → #[serde(default)] даёт пустой вектор
    assert!(checklist.items.is_empty());
}

#[tokio::test]
async fn add_item_posts_text() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cards/67089469/checklists/11747430/items"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_json(serde_json::json!({ "text": "first item" })))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(CHECKLIST_ITEM_ADD, "application/json"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let item = client
        .checklists()
        .add_item(67_089_469, 11_747_430, "first item")
        .await
        .unwrap();

    assert_eq!(item.id, 65_658_564);
    assert_eq!(item.text, "first item");
    assert_eq!(item.checked, Some(false));
}

#[tokio::test]
async fn set_item_checked_patches_checked_flag() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/cards/67089469/checklists/11747430/items/65658564"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_json(serde_json::json!({ "checked": true })))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(CHECKLIST_ITEM_CHECK, "application/json"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let item = client
        .checklists()
        .set_item_checked(67_089_469, 11_747_430, 65_658_564, true)
        .await
        .unwrap();

    assert_eq!(item.id, 65_658_564);
    assert_eq!(item.checked, Some(true));
}
