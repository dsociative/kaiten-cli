use kaiten_client::KaitenClient;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const USER_CURRENT: &str = include_str!("fixtures/user_current.json");
const USERS_LIST: &str = include_str!("fixtures/users_list.json");

#[tokio::test]
async fn current_hits_users_current_with_bearer_token() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/current"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(USER_CURRENT, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let user = client.users().current().await.unwrap();

    assert_eq!(user.id, 1068514);
    assert_eq!(user.uid, "0abd61ea-9dc5-40eb-b0a9-0d452ba1d8a6");
    assert_eq!(user.full_name.as_deref(), Some("dxmuser"));
    assert_eq!(user.email.as_deref(), Some("user@example.com"));
    assert_eq!(user.activated, Some(true));
}

#[tokio::test]
async fn list_parses_users_and_tolerates_missing_fields() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(USERS_LIST, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let users = client.users().list().await.unwrap();

    assert_eq!(users.len(), 2);
    assert_eq!(users[0].username.as_deref(), Some("dxmuser"));
    assert_eq!(users[1].id, 1068515);
    assert_eq!(users[1].full_name, None);
    assert_eq!(users[1].email, None);
}
