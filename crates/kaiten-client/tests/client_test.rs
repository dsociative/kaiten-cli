use kaiten_client::KaitenError;

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

    let rate_limited = KaitenError::RateLimited { retry_after_secs: 3 };
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
