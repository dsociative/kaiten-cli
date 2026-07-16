use kaiten_client::{KaitenClient, KaitenError};
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const MEMBER_ADD: &str = include_str!("fixtures/card_members_add.json");

#[tokio::test]
async fn add_posts_user_id_and_parses_member() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cards/67089469/members"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_json(serde_json::json!({ "user_id": 1_068_514 })))
        .respond_with(ResponseTemplate::new(200).set_body_raw(MEMBER_ADD, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let member = client.members().add(67_089_469, 1_068_514).await.unwrap();

    assert_eq!(member.id, 1_068_514);
    assert_eq!(member.member_type, Some(1));
    // в ответе POST /cards/{id}/members нет user_id
    assert_eq!(member.user_id, None);
}

#[tokio::test]
async fn remove_returns_ok_on_empty_body() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/cards/67089469/members/1068514"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    client
        .members()
        .remove(67_089_469, 1_068_514)
        .await
        .unwrap();
}

#[tokio::test]
async fn remove_retries_on_429_then_succeeds() {
    let server = MockServer::start().await;
    // Первый DELETE получает 429 (Reset=0 → пауза 0 секунд, тест не тормозит);
    // up_to_n_times(1) выключает мок после одного ответа.
    Mock::given(method("DELETE"))
        .and(path("/cards/1/members/2"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(429).insert_header("X-RateLimit-Reset", "0"))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    // Второй DELETE попадает сюда; expect(1) + expect(1) = ровно 2 запроса суммарно.
    Mock::given(method("DELETE"))
        .and(path("/cards/1/members/2"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("{}", "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    client.members().remove(1, 2).await.unwrap();
}

#[tokio::test]
async fn remove_maps_403_with_empty_body_to_api_error() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/cards/67089469/members/999"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(403))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let err = client.members().remove(67_089_469, 999).await.unwrap_err();

    match err {
        KaitenError::Api {
            status,
            message,
            body,
        } => {
            assert_eq!(status, 403);
            assert_eq!(message, "Forbidden");
            assert_eq!(body, "");
        }
        other => panic!("expected Api error, got: {other:?}"),
    }
}

/// PATCH /cards/{id}/members/{user_id} responds WITHOUT an `id` field
/// (live-verified) — the model must still parse.
#[tokio::test]
async fn update_role_parses_idless_response() {
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/cards/67089309/members/1068514"))
        .and(body_json(serde_json::json!({ "type": 2 })))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"{"card_id": 67089309, "user_id": 1068514, "type": 2,
                "created": "2026-07-16T13:44:58.899Z"}"#,
            "application/json",
        ))
        .expect(1)
        .mount(&server)
        .await;

    let client = kaiten_client::KaitenClient::new(&server.uri(), "test-token").unwrap();
    let member = client
        .members()
        .update_role(67_089_309, 1_068_514, true)
        .await
        .unwrap();
    assert_eq!(member.user_id, Some(1_068_514));
    assert_eq!(member.member_type, Some(2));
}
