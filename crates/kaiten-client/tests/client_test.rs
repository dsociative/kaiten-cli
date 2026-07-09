use kaiten_client::{KaitenClient, KaitenError};
use reqwest::Method;
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn user_fixture() -> serde_json::Value {
    serde_json::from_str(include_str!("fixtures/user_current.json")).unwrap()
}

#[test]
fn error_display_formats() {
    // Display печатает только message; body в вывод не попадает.
    let api = KaitenError::Api {
        status: 400,
        message: "Card should have required property 'board_id'".to_string(),
        body: r#"{"message":"Card should have required property 'board_id'"}"#.to_string(),
    };
    assert_eq!(
        api.to_string(),
        "API error 400: Card should have required property 'board_id'"
    );

    let rate_limited = KaitenError::RateLimited {
        retry_after_secs: 3,
    };
    assert_eq!(rate_limited.to_string(), "rate limited, retry after 3s");

    let source = serde_json::from_str::<u64>("\"oops\"").unwrap_err();
    let decode = KaitenError::Decode {
        path: "id".to_string(),
        source,
    };
    assert!(
        decode
            .to_string()
            .starts_with("failed to decode response at `id`:"),
        "unexpected display: {decode}"
    );

    let invalid = KaitenError::InvalidBaseUrl("not a url".to_string());
    assert_eq!(invalid.to_string(), "invalid base url: not a url");
}

#[test]
fn new_rejects_invalid_base_url() {
    let err = KaitenClient::new("not a url", "test-token").unwrap_err();
    assert!(
        matches!(err, KaitenError::InvalidBaseUrl(_)),
        "got: {err:?}"
    );
}

#[tokio::test]
async fn get_sends_bearer_and_returns_json() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/current"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(user_fixture()))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let value = client
        .raw(Method::GET, "/users/current", None)
        .await
        .unwrap();

    assert_eq!(value["id"], 1_068_514);
    assert_eq!(value["email"], "user@example.com");
}

#[tokio::test]
async fn api_error_400_uses_json_message() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cards"))
        .respond_with(ResponseTemplate::new(400).set_body_raw(
            r#"{"message":"Card should have required property 'board_id'"}"#,
            "application/json",
        ))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let err = client
        .raw(Method::POST, "/cards", Some(json!({"title": "x"})))
        .await
        .unwrap_err();

    match err {
        KaitenError::Api {
            status,
            message,
            body,
        } => {
            assert_eq!(status, 400);
            assert_eq!(message, "Card should have required property 'board_id'");
            assert_eq!(
                body,
                r#"{"message":"Card should have required property 'board_id'"}"#
            );
        }
        other => panic!("expected Api error, got {other:?}"),
    }
}

#[tokio::test]
async fn api_error_403_empty_body_uses_canonical_reason() {
    // Реальный Kaiten отвечает 403 с ПУСТЫМ телом на чужие/несуществующие карточки.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cards/999"))
        .respond_with(ResponseTemplate::new(403))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let err = client
        .raw(Method::GET, "/cards/999", None)
        .await
        .unwrap_err();

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
        other => panic!("expected Api error, got {other:?}"),
    }
}

#[tokio::test]
async fn api_error_500_non_json_body_kept_verbatim() {
    // 5xx с не-JSON телом: message = body = сырое тело целиком.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/current"))
        .respond_with(ResponseTemplate::new(500).set_body_raw("internal error text", "text/plain"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let err = client
        .raw(Method::GET, "/users/current", None)
        .await
        .unwrap_err();

    match err {
        KaitenError::Api {
            status,
            message,
            body,
        } => {
            assert_eq!(status, 500);
            assert_eq!(message, "internal error text");
            assert_eq!(body, "internal error text");
        }
        other => panic!("expected Api error, got {other:?}"),
    }
}

#[tokio::test]
async fn retries_once_on_429_then_succeeds() {
    let server = MockServer::start().await;
    // Первый запрос получает 429; up_to_n_times(1) выключает мок после одного ответа.
    Mock::given(method("GET"))
        .and(path("/users/current"))
        .respond_with(ResponseTemplate::new(429).insert_header("X-RateLimit-Reset", "1"))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    // Второй запрос попадает сюда; expect(1) + expect(1) = ровно 2 запроса суммарно.
    Mock::given(method("GET"))
        .and(path("/users/current"))
        .respond_with(ResponseTemplate::new(200).set_body_json(user_fixture()))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let value = client
        .raw(Method::GET, "/users/current", None)
        .await
        .unwrap();

    assert_eq!(value["id"], 1_068_514);
}

#[tokio::test]
async fn gives_up_after_three_retries_on_429() {
    let server = MockServer::start().await;
    // Reset=0 → пауза 0 секунд, тест не тормозит. 1 запрос + 3 ретрая = 4 запроса.
    Mock::given(method("GET"))
        .and(path("/users/current"))
        .respond_with(ResponseTemplate::new(429).insert_header("X-RateLimit-Reset", "0"))
        .expect(4)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let err = client
        .raw(Method::GET, "/users/current", None)
        .await
        .unwrap_err();

    match err {
        // В ошибке — ФАКТИЧЕСКОЕ значение X-RateLimit-Reset последнего ответа (здесь 0);
        // клампится только пауза sleep, но не значение в ошибке.
        KaitenError::RateLimited { retry_after_secs } => assert_eq!(retry_after_secs, 0),
        other => panic!("expected RateLimited, got {other:?}"),
    }
}

#[tokio::test]
async fn raw_post_sends_body_and_returns_value() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cards"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_json(
            json!({"board_id": 1_826_109, "title": "from raw"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"{"id": 67089469, "title": "from raw", "board_id": 1826109}"#,
            "application/json",
        ))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let value = client
        .raw(
            Method::POST,
            "/cards",
            Some(json!({"board_id": 1_826_109, "title": "from raw"})),
        )
        .await
        .unwrap();

    assert_eq!(value["id"], 67_089_469);
    assert_eq!(value["title"], "from raw");
}
