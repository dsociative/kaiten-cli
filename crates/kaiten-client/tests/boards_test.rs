use kaiten_client::KaitenClient;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const BOARD_GET: &str = include_str!("fixtures/board_get.json");
const BOARDS_LIST: &str = include_str!("fixtures/boards_list.json");

#[tokio::test]
async fn get_parses_board_with_columns_and_lanes() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/boards/1826109"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(BOARD_GET, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let board = client.boards().get(1826109).await.unwrap();

    assert_eq!(board.id, 1826109);
    assert_eq!(board.title, "test-board");
    assert_eq!(board.default_card_type_id, Some(1));
    assert_eq!(board.columns.len(), 1);
    assert_eq!(board.columns[0].title, "To Do");
    // JSON-поле "type" (int) маппится в column_type
    assert_eq!(board.columns[0].column_type, Some(1));
    assert_eq!(board.columns[0].board_id, Some(1826109));
    assert_eq!(board.lanes.len(), 1);
    assert_eq!(board.lanes[0].title, "Default Lane");
}

#[tokio::test]
async fn list_parses_boards_without_columns_and_lanes() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/spaces/810669/boards"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(BOARDS_LIST, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let boards = client.boards().list(810669).await.unwrap();

    assert_eq!(boards.len(), 2);
    assert_eq!(boards[0].title, "Задачи");
    // ответ без ключей columns/lanes → #[serde(default)] даёт пустые векторы
    assert!(boards[0].columns.is_empty());
    assert!(boards[0].lanes.is_empty());
}
