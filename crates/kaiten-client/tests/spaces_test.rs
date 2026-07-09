use kaiten_client::KaitenClient;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SPACES_LIST: &str = include_str!("fixtures/spaces_list.json");

#[tokio::test]
async fn list_parses_spaces_and_tolerates_missing_fields() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/spaces"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(SPACES_LIST, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let spaces = client.spaces().list().await.unwrap();

    assert_eq!(spaces.len(), 2);
    assert_eq!(spaces[0].id, 810_669);
    assert_eq!(spaces[0].title, "Первое пространство");
    assert_eq!(spaces[0].archived, Some(false));
    assert_eq!(spaces[1].title, "kaiten-cli-test");
    assert_eq!(spaces[1].archived, None);
}
