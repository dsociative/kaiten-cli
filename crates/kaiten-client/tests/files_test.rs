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
/// Shape B as reported in issue #12 from another Kaiten instance (`type` 11,
/// UUID `id`, string `size`, host-root-relative `url`); not yet reproduced on
/// the test account — replace with a live capture when one is available.
const CARD_GET_FILES_TYPE11: &str = include_str!("fixtures/card_get_files_type11.json");

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
        .and(path("/cards/12345678"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(CARD_GET_FILES_TYPE11, "application/json"),
        )
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let files = client.files().list(12_345_678).await.unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].id, 0);
    assert_eq!(files[0].size, Some(58_818));
    assert!(
        files[0]
            .url
            .as_deref()
            .unwrap()
            .starts_with("/api/v1/cards/")
    );
    assert_eq!(
        kaiten_client::FileRef::from(&files[0]),
        kaiten_client::FileRef::Uid("6a8e66af-0000-0000-0000-000000000000".into())
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
