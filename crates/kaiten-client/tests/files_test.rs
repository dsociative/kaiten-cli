use kaiten_client::KaitenClient;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Matches a multipart body that carries the given file name and content.
struct MultipartWith {
    file_name: &'static str,
    content: &'static str,
}

impl wiremock::Match for MultipartWith {
    fn matches(&self, request: &wiremock::Request) -> bool {
        let content_type = request
            .headers
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        let body = String::from_utf8_lossy(&request.body);
        content_type.starts_with("multipart/form-data")
            && body.contains("name=\"file\"")
            && body.contains(self.file_name)
            && body.contains(self.content)
    }
}

#[tokio::test]
async fn attach_uploads_multipart_put_and_parses_file() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/cards/67089469/files"))
        .and(header("Authorization", "Bearer test-token"))
        .and(MultipartWith {
            file_name: "note.txt",
            content: "attachment body",
        })
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            r#"{"id": 61256602, "name": "note.txt", "size": 15,
                "url": "https://files.kaiten.ru/abc.txt", "type": 1}"#,
            "application/json",
        ))
        .expect(1)
        .mount(&server)
        .await;

    let dir = std::env::temp_dir().join(format!("kaiten-files-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file_path = dir.join("note.txt");
    std::fs::write(&file_path, "attachment body").unwrap();

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let file = client.files().attach(67_089_469, &file_path).await.unwrap();

    assert_eq!(file.id, 61_256_602);
    assert_eq!(file.name, "note.txt");
    assert_eq!(file.url.as_deref(), Some("https://files.kaiten.ru/abc.txt"));
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn detach_hits_card_scoped_file_path() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/cards/67089469/files/61256602"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    client.files().detach(67_089_469, 61_256_602).await.unwrap();
}

#[tokio::test]
async fn attach_missing_local_file_is_io_error_without_any_request() {
    let server = MockServer::start().await;
    // no mocks mounted: the request must never be sent

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let err = client
        .files()
        .attach(1, std::path::Path::new("/nonexistent/nope.txt"))
        .await
        .unwrap_err();
    assert!(
        matches!(err, kaiten_client::KaitenError::Io(_)),
        "expected Io error, got {err:?}"
    );
}
