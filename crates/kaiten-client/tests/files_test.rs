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

// ---------------------------------------------------------------------------
// issue #12: list + download
// ---------------------------------------------------------------------------

const CARD_GET_FULL: &str = include_str!("fixtures/card_get_full.json");
/// Shape B, captured live (sanitized) from a card on the newer storage
/// (`type` 11): UUID `id`, string `size`, host-root-relative `url`.
const CARD_GET_FILES_TYPE11: &str = include_str!("fixtures/card_get_files_type11.json");
/// What the newer storage's API path answers (captured live, sanitized): the
/// file's metadata with a short-lived signed storage `url` — not the bytes.
const FILE_METADATA_TYPE11: &str = include_str!("fixtures/file_metadata_type11.json");

/// Matches a request that carries NO `Authorization` header.
struct NoAuthHeader;

impl wiremock::Match for NoAuthHeader {
    fn matches(&self, request: &wiremock::Request) -> bool {
        !request.headers.contains_key("authorization")
    }
}

fn card_file(json: &str) -> kaiten_client::CardFile {
    serde_json::from_str(json).unwrap()
}

#[tokio::test]
async fn list_takes_files_from_card_get_not_from_files_endpoint() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cards/67089469"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARD_GET_FULL, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    // the dedicated endpoint returns [] on the newer storage — must not be used
    Mock::given(method("GET"))
        .and(path("/cards/67089469/files"))
        .respond_with(ResponseTemplate::new(200).set_body_string("[]"))
        .expect(0)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let files = client.files().list(67_089_469).await.unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].id, 61_256_602);
    assert_eq!(files[0].size, Some(58));
}

#[tokio::test]
async fn list_parses_uuid_id_and_string_size_from_newer_storage() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cards/64533247"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(CARD_GET_FILES_TYPE11, "application/json"),
        )
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let files = client.files().list(64_533_247).await.unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].id, 0);
    assert_eq!(files[0].size, Some(33_055));
    assert_eq!(files[0].file_type, Some(11));
    assert!(
        files[0]
            .url
            .as_deref()
            .unwrap()
            .starts_with("/api/v1/cards/")
    );
    assert_eq!(
        kaiten_client::FileRef::from(&files[0]),
        kaiten_client::FileRef::Uid("08b5876d-0000-0000-0000-000000000000".into())
    );
}

#[tokio::test]
async fn download_absolute_public_url_sends_no_token_and_no_api_request() {
    let api = MockServer::start().await;
    let storage = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/abc.txt"))
        .and(NoAuthHeader)
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"hello".to_vec()))
        .expect(1)
        .mount(&storage)
        .await;

    let client = KaitenClient::new(&api.uri(), "test-token").unwrap();
    let file = card_file(&format!(
        r#"{{"id": 1, "name": "abc.txt", "url": "{}/abc.txt"}}"#,
        storage.uri()
    ));
    let bytes = client.files().download(&file).await.unwrap();
    assert_eq!(bytes, b"hello");
    assert!(api.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn download_relative_url_resolves_against_host_root_with_bearer_and_follows_same_host_302() {
    let api = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/cards/CU/files/FU"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(302)
                .insert_header("Location", format!("{}/storage/FU", api.uri()).as_str()),
        )
        .expect(1)
        .mount(&api)
        .await;
    Mock::given(method("GET"))
        .and(path("/storage/FU"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"bytes".to_vec()))
        .expect(1)
        .mount(&api)
        .await;

    // the client's base url carries the /api/latest prefix; the file url must
    // be resolved against the host root, not appended to that prefix
    let client = KaitenClient::new(&format!("{}/api/latest", api.uri()), "test-token").unwrap();
    let file = card_file(r#"{"id": "FU", "name": "r.xlsx", "url": "/api/v1/cards/CU/files/FU"}"#);
    let bytes = client.files().download(&file).await.unwrap();
    assert_eq!(bytes, b"bytes");
    let first = api.received_requests().await.unwrap().remove(0);
    assert_eq!(first.url.path(), "/api/v1/cards/CU/files/FU");
}

#[tokio::test]
async fn download_redirect_to_other_host_drops_authorization() {
    let api = MockServer::start().await;
    let storage = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/cards/CU/files/FU"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(302)
                .insert_header("Location", format!("{}/blob", storage.uri()).as_str()),
        )
        .expect(1)
        .mount(&api)
        .await;
    Mock::given(method("GET"))
        .and(path("/blob"))
        .and(NoAuthHeader)
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"blob".to_vec()))
        .expect(1)
        .mount(&storage)
        .await;

    let client = KaitenClient::new(&format!("{}/api/latest", api.uri()), "test-token").unwrap();
    let file = card_file(r#"{"id": "FU", "name": "r", "url": "/api/v1/cards/CU/files/FU"}"#);
    assert_eq!(client.files().download(&file).await.unwrap(), b"blob");
}

#[tokio::test]
async fn download_non_2xx_is_api_error() {
    let api = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/cards/CU/files/FU"))
        .respond_with(
            ResponseTemplate::new(404)
                .set_body_raw(r#"{"message":"File not found"}"#, "application/json"),
        )
        .mount(&api)
        .await;

    let client = KaitenClient::new(&format!("{}/api/latest", api.uri()), "test-token").unwrap();
    let file = card_file(r#"{"id": "FU", "name": "r", "url": "/api/v1/cards/CU/files/FU"}"#);
    let err = client.files().download(&file).await.unwrap_err();
    assert!(
        matches!(err, kaiten_client::KaitenError::Api { status: 404, .. }),
        "{err:?}"
    );
    assert!(err.to_string().contains("File not found"), "{err}");
}

#[tokio::test]
async fn download_without_url_is_io_error_without_request() {
    let api = MockServer::start().await;
    let client = KaitenClient::new(&api.uri(), "test-token").unwrap();
    let file = card_file(r#"{"id": 5, "name": "r"}"#);
    let err = client.files().download(&file).await.unwrap_err();
    assert!(
        matches!(&err, kaiten_client::KaitenError::Io(e) if e.kind() == std::io::ErrorKind::InvalidInput),
        "{err:?}"
    );
    assert!(err.to_string().contains("no download url"), "{err}");
    assert!(api.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn download_to_writes_bytes_to_path() {
    let storage = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/f.bin"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![0, 1, 2, 255]))
        .mount(&storage)
        .await;

    let client = KaitenClient::new("http://127.0.0.1:9", "test-token").unwrap(); // API never called
    let file = card_file(&format!(
        r#"{{"id": 1, "name": "f.bin", "url": "{}/f.bin"}}"#,
        storage.uri()
    ));
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("out.bin");
    client.files().download_to(&file, &target).await.unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), vec![0, 1, 2, 255]);
}

/// An expired public link answers with a whole HTML page; the error must
/// stay readable (reason phrase) and must not carry the page around.
#[tokio::test]
async fn download_html_error_page_is_not_dumped_verbatim() {
    let storage = MockServer::start().await;
    let page = format!(
        "<!DOCTYPE html><html><body>{}</body></html>",
        "x".repeat(5000)
    );
    Mock::given(method("GET"))
        .and(path("/gone.txt"))
        .respond_with(ResponseTemplate::new(404).set_body_raw(page, "text/html"))
        .mount(&storage)
        .await;

    let client = KaitenClient::new("http://127.0.0.1:9", "test-token").unwrap();
    let file = card_file(&format!(
        r#"{{"id": 1, "name": "gone.txt", "url": "{}/gone.txt"}}"#,
        storage.uri()
    ));
    let err = client.files().download(&file).await.unwrap_err();
    assert_eq!(err.to_string(), "API error 404: Not Found");
    match err {
        kaiten_client::KaitenError::Api { body, .. } => {
            assert!(
                body.len() <= 300,
                "body must be truncated, got {} bytes",
                body.len()
            );
        }
        other => panic!("unexpected {other:?}"),
    }
}

/// Newer storage, as observed live: `GET /api/v1/cards/{card_uid}/files/{id}`
/// (bearer) answers 200 with JSON metadata whose `url` is a short-lived signed
/// storage link; the bytes are fetched from there, without credentials.
#[tokio::test]
async fn download_newer_storage_follows_the_signed_url_from_the_metadata() {
    let api = MockServer::start().await;
    let storage = MockServer::start().await;
    let meta = FILE_METADATA_TYPE11.replace(
        "https://storage.example/bucket",
        &format!("{}/bucket", storage.uri()),
    );
    Mock::given(method("GET"))
        .and(path("/api/v1/cards/14dc3064-0000-0000-0000-000000000000/files/08b5876d-0000-0000-0000-000000000000"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(meta, "application/json"))
        .expect(1)
        .mount(&api)
        .await;
    Mock::given(method("GET"))
        .and(path("/bucket/08b5876d-0000-0000-0000-000000000000"))
        .and(NoAuthHeader)
        .respond_with(ResponseTemplate::new(200).set_body_raw("<html>report</html>", "text/html"))
        .expect(1)
        .mount(&storage)
        .await;

    let client = KaitenClient::new(&format!("{}/api/latest", api.uri()), "test-token").unwrap();
    let card: kaiten_client::Card = serde_json::from_str(CARD_GET_FILES_TYPE11).unwrap();
    let bytes = client.files().download(&card.files[0]).await.unwrap();
    assert_eq!(bytes, b"<html>report</html>");
}

#[tokio::test]
async fn download_newer_storage_metadata_without_url_is_an_error_without_a_second_request() {
    let api = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/cards/14dc3064-0000-0000-0000-000000000000/files/08b5876d-0000-0000-0000-000000000000"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(r#"{"id": "x", "name": "r.html"}"#, "application/json"))
        .expect(1)
        .mount(&api)
        .await;

    let client = KaitenClient::new(&format!("{}/api/latest", api.uri()), "test-token").unwrap();
    let card: kaiten_client::Card = serde_json::from_str(CARD_GET_FILES_TYPE11).unwrap();
    let err = client.files().download(&card.files[0]).await.unwrap_err();
    assert!(
        matches!(&err, kaiten_client::KaitenError::Io(e) if e.kind() == std::io::ErrorKind::InvalidInput),
        "{err:?}"
    );
    assert!(err.to_string().contains("download url"), "{err}");
}

/// A redirecting instance hands the bytes over as-is: a JSON *attachment*
/// reached through a 302 must never be mistaken for storage metadata.
#[tokio::test]
async fn download_json_attachment_behind_a_redirect_is_returned_verbatim() {
    let api = MockServer::start().await;
    let storage = MockServer::start().await;
    let attachment = format!(r#"{{"url": "{}/elsewhere"}}"#, storage.uri());
    Mock::given(method("GET"))
        .and(path("/api/v1/cards/CU/files/FU"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(302)
                .insert_header("Location", format!("{}/data.json", storage.uri()).as_str()),
        )
        .expect(1)
        .mount(&api)
        .await;
    Mock::given(method("GET"))
        .and(path("/data.json"))
        .and(NoAuthHeader)
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(attachment.clone(), "application/json"),
        )
        .expect(1)
        .mount(&storage)
        .await;
    Mock::given(method("GET"))
        .and(path("/elsewhere"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"wrong".to_vec()))
        .expect(0)
        .mount(&storage)
        .await;

    let client = KaitenClient::new(&format!("{}/api/latest", api.uri()), "test-token").unwrap();
    let file =
        card_file(r#"{"id": "FU", "name": "data.json", "url": "/api/v1/cards/CU/files/FU"}"#);
    assert_eq!(
        client.files().download(&file).await.unwrap(),
        attachment.as_bytes()
    );
}

#[tokio::test]
async fn download_classic_json_attachment_is_returned_verbatim() {
    let storage = MockServer::start().await;
    let attachment = r#"{"url": "https://example.invalid/not-followed"}"#;
    Mock::given(method("GET"))
        .and(path("/a.json"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(attachment, "application/json"))
        .expect(1)
        .mount(&storage)
        .await;

    let client = KaitenClient::new("http://127.0.0.1:9", "test-token").unwrap();
    let file = card_file(&format!(
        r#"{{"id": 1, "name": "a.json", "url": "{}/a.json"}}"#,
        storage.uri()
    ));
    assert_eq!(
        client.files().download(&file).await.unwrap(),
        attachment.as_bytes()
    );
}

#[tokio::test]
async fn download_newer_storage_recognises_the_media_type_case_insensitively() {
    let api = MockServer::start().await;
    let storage = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/cards/CU/files/FU"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            format!(r#"{{"id": "FU", "url": "{}/signed"}}"#, storage.uri()),
            "Application/JSON; charset=UTF-8",
        ))
        .expect(1)
        .mount(&api)
        .await;
    Mock::given(method("GET"))
        .and(path("/signed"))
        .and(NoAuthHeader)
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"real".to_vec()))
        .expect(1)
        .mount(&storage)
        .await;

    let client = KaitenClient::new(&format!("{}/api/latest", api.uri()), "test-token").unwrap();
    let file = card_file(r#"{"id": "FU", "name": "r", "url": "/api/v1/cards/CU/files/FU"}"#);
    assert_eq!(client.files().download(&file).await.unwrap(), b"real");
}

#[tokio::test]
async fn download_newer_storage_invalid_json_metadata_is_a_decode_error() {
    let api = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/cards/CU/files/FU"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw("<html>login</html>", "application/json"),
        )
        .mount(&api)
        .await;

    let client = KaitenClient::new(&format!("{}/api/latest", api.uri()), "test-token").unwrap();
    let file = card_file(r#"{"id": "FU", "name": "r", "url": "/api/v1/cards/CU/files/FU"}"#);
    let err = client.files().download(&file).await.unwrap_err();
    assert!(
        matches!(err, kaiten_client::KaitenError::Decode { .. }),
        "{err:?}"
    );
}

/// The signed link lives for seconds; when storage refuses it the error must
/// say so and name the file, not just echo an S3 XML page.
#[tokio::test]
async fn download_newer_storage_refused_signed_link_names_the_file_and_storage() {
    let api = MockServer::start().await;
    let storage = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/cards/CU/files/FU"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            format!(
                r#"{{"id": "FU", "name": "report.html", "url": "{}/signed"}}"#,
                storage.uri()
            ),
            "application/json",
        ))
        .mount(&api)
        .await;
    Mock::given(method("GET"))
        .and(path("/signed"))
        .respond_with(ResponseTemplate::new(403).set_body_raw(
            "<?xml version=\"1.0\"?><Error><Code>AccessDenied</Code><Message>Request has expired</Message></Error>",
            "application/xml",
        ))
        .mount(&storage)
        .await;

    let client = KaitenClient::new(&format!("{}/api/latest", api.uri()), "test-token").unwrap();
    let file =
        card_file(r#"{"id": "FU", "name": "report.html", "url": "/api/v1/cards/CU/files/FU"}"#);
    let err = client.files().download(&file).await.unwrap_err();
    assert!(
        matches!(err, kaiten_client::KaitenError::Api { status: 403, .. }),
        "{err:?}"
    );
    let text = err.to_string();
    assert!(
        text.contains("storage refused") && text.contains("report.html"),
        "{text}"
    );
}
