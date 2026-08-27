//! `kaiten card file list|get` (issue #12): listing from the card and
//! downloading either storage shape.

use assert_cmd::Command;
use predicates::prelude::*;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const CARD_WITH_FILES: &str = include_str!("fixtures/card_with_files.json");
const CARD_WITHOUT_FILES: &str = include_str!("fixtures/card_get_full.json");
const CLASSIC_PATH: &str = "/48c405aa-a7a3-455e-9752-f2c3225cfecb.txt";
const NEWER_PATH: &str =
    "/api/v1/cards/c78e313c-ab37-4456-9eb0-904681c4e309/files/6a8e66af-0000-0000-0000-000000000000";
const NEWER_UID: &str = "6a8e66af-0000-0000-0000-000000000000";

/// Matches a request that carries NO `Authorization` header.
struct NoAuthHeader;

impl wiremock::Match for NoAuthHeader {
    fn matches(&self, request: &wiremock::Request) -> bool {
        !request.headers.contains_key("authorization")
    }
}

fn kaiten(config_dir: &std::path::Path, base_url: &str, cwd: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("kaiten").unwrap();
    cmd.env_remove("KAITEN_TOKEN")
        .env_remove("KAITEN_DOMAIN")
        .env_remove("KAITEN_BASE_URL")
        .env_remove("RUST_LOG")
        .env("KAITEN_CONFIG_DIR", config_dir)
        .env("KAITEN_BASE_URL", base_url)
        .env("KAITEN_TOKEN", "test-token")
        .env("NO_COLOR", "1")
        .current_dir(cwd);
    cmd
}

/// The card GET, with the classic file's public host pointed at `storage`.
async fn mock_card(api: &MockServer, storage: &MockServer) {
    let body = CARD_WITH_FILES.replace("https://files.kaiten.ru", &storage.uri());
    Mock::given(method("GET"))
        .and(path("/cards/67089469"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .mount(api)
        .await;
}

async fn mock_classic_download(storage: &MockServer, expect: u64) {
    Mock::given(method("GET"))
        .and(path(CLASSIC_PATH))
        .and(NoAuthHeader)
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"attachment body".to_vec()))
        .expect(expect)
        .mount(storage)
        .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn card_file_list_prints_table() {
    let api = MockServer::start().await;
    let storage = MockServer::start().await;
    mock_card(&api, &storage).await;
    let tmp = tempfile::tempdir().unwrap();

    let out = kaiten(tmp.path(), &api.uri(), tmp.path())
        .args(["card", "file", "list", "67089469"])
        .assert()
        .success()
        .stdout(predicate::str::contains("61256602"))
        .stdout(predicate::str::contains("probe-attach.txt"))
        .stdout(predicate::str::contains(NEWER_UID))
        .stdout(predicate::str::contains("report.xlsx"))
        .stdout(predicate::str::contains("58818"))
        .get_output()
        .stdout
        .clone();
    insta::assert_snapshot!("card_file_list", String::from_utf8(out).unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn card_file_list_json_prints_raw_files_plus_uid() {
    let api = MockServer::start().await;
    let storage = MockServer::start().await;
    mock_card(&api, &storage).await;
    let tmp = tempfile::tempdir().unwrap();

    let out = kaiten(tmp.path(), &api.uri(), tmp.path())
        .args(["--json", "card", "file", "list", "67089469"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let files = value.as_array().unwrap();
    assert_eq!(files.len(), 2);
    assert_eq!(files[0]["id"], 61_256_602);
    assert!(files[0].get("uid").is_none(), "{value}");
    assert_eq!(files[1]["id"], 0);
    assert_eq!(files[1]["uid"], NEWER_UID, "{value}");
    assert_eq!(files[1]["size"], 58_818);
}

#[tokio::test(flavor = "multi_thread")]
async fn card_file_get_output_with_missing_parent_names_the_path() {
    let api = MockServer::start().await;
    let storage = MockServer::start().await;
    mock_card(&api, &storage).await;
    mock_classic_download(&storage, 0).await; // refused before any download
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &api.uri(), tmp.path())
        .args([
            "card",
            "file",
            "get",
            "67089469",
            "61256602",
            "-o",
            "missing/dir/x.txt",
        ])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("missing/dir"));
    assert!(storage.received_requests().await.unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn card_file_get_output_missing_directory_with_trailing_slash_is_an_error() {
    let api = MockServer::start().await;
    let storage = MockServer::start().await;
    mock_card(&api, &storage).await;
    mock_classic_download(&storage, 0).await; // refused before any download
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &api.uri(), tmp.path())
        .args([
            "card",
            "file",
            "get",
            "67089469",
            "61256602",
            "-o",
            "downloads/",
        ])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("does not exist"));
    assert!(!tmp.path().join("downloads").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn card_file_list_empty_card() {
    let api = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cards/67089469"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(CARD_WITHOUT_FILES, "application/json"),
        )
        .mount(&api)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &api.uri(), tmp.path())
        .args(["card", "file", "list", "67089469"])
        .assert()
        .success()
        .stdout(predicate::str::contains("no files on card 67089469"));
}

#[tokio::test(flavor = "multi_thread")]
async fn card_file_get_saves_original_name_in_cwd() {
    let api = MockServer::start().await;
    let storage = MockServer::start().await;
    mock_card(&api, &storage).await;
    mock_classic_download(&storage, 1).await;
    let tmp = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &api.uri(), cwd.path())
        .args(["card", "file", "get", "67089469", "61256602"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "saved probe-attach.txt (15 bytes) to ",
        ))
        .stdout(predicate::str::contains("/probe-attach.txt"));
    assert_eq!(
        std::fs::read_to_string(cwd.path().join("probe-attach.txt")).unwrap(),
        "attachment body"
    );
}

/// Newer storage, as observed live: the API path (bearer) answers with the
/// file's metadata carrying a short-lived signed storage url; the bytes come
/// from there, without credentials.
#[tokio::test(flavor = "multi_thread")]
async fn card_file_get_by_uid_fetches_the_signed_storage_url_from_the_metadata() {
    let api = MockServer::start().await;
    let storage = MockServer::start().await;
    mock_card(&api, &storage).await;
    Mock::given(method("GET"))
        .and(path(NEWER_PATH))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            format!(
                r#"{{"id": "{NEWER_UID}", "name": "report.xlsx", "size": "10",
                    "url": "{}/blob.xlsx?X-Amz-Signature=x"}}"#,
                storage.uri()
            ),
            "application/json",
        ))
        .expect(1)
        .mount(&api)
        .await;
    Mock::given(method("GET"))
        .and(path("/blob.xlsx"))
        .and(NoAuthHeader)
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"xlsx bytes".to_vec()))
        .expect(1)
        .mount(&storage)
        .await;
    let tmp = tempfile::tempdir().unwrap();
    let out = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &api.uri(), tmp.path())
        .args([
            "card",
            "file",
            "get",
            "67089469",
            NEWER_UID,
            "-o",
            out.path().to_str().unwrap(),
        ])
        .assert()
        .success();
    assert_eq!(
        std::fs::read(out.path().join("report.xlsx")).unwrap(),
        b"xlsx bytes"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn card_file_get_output_file_path() {
    let api = MockServer::start().await;
    let storage = MockServer::start().await;
    mock_card(&api, &storage).await;
    mock_classic_download(&storage, 1).await;
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("renamed.bin");

    kaiten(tmp.path(), &api.uri(), tmp.path())
        .args([
            "card",
            "file",
            "get",
            "67089469",
            "61256602",
            "--output",
            target.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("to ").and(predicate::str::contains("renamed.bin")));
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "attachment body");
}

#[tokio::test(flavor = "multi_thread")]
async fn card_file_get_refuses_overwrite_without_force() {
    let api = MockServer::start().await;
    let storage = MockServer::start().await;
    mock_card(&api, &storage).await;
    mock_classic_download(&storage, 0).await; // nothing may be downloaded
    let tmp = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    std::fs::write(cwd.path().join("probe-attach.txt"), "old").unwrap();

    kaiten(tmp.path(), &api.uri(), cwd.path())
        .args(["card", "file", "get", "67089469", "61256602"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("already exists"))
        .stderr(predicate::str::contains("--force"));
    assert_eq!(
        std::fs::read_to_string(cwd.path().join("probe-attach.txt")).unwrap(),
        "old"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn card_file_get_force_overwrites() {
    let api = MockServer::start().await;
    let storage = MockServer::start().await;
    mock_card(&api, &storage).await;
    mock_classic_download(&storage, 1).await;
    let tmp = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();
    std::fs::write(cwd.path().join("probe-attach.txt"), "old").unwrap();

    kaiten(tmp.path(), &api.uri(), cwd.path())
        .args(["card", "file", "get", "67089469", "61256602", "--force"])
        .assert()
        .success();
    assert_eq!(
        std::fs::read_to_string(cwd.path().join("probe-attach.txt")).unwrap(),
        "attachment body"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn card_file_get_unknown_file_lists_available_ids() {
    let api = MockServer::start().await;
    let storage = MockServer::start().await;
    mock_card(&api, &storage).await;
    mock_classic_download(&storage, 0).await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &api.uri(), tmp.path())
        .args(["card", "file", "get", "67089469", "999"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("no file `999`"))
        .stderr(predicate::str::contains("61256602 (probe-attach.txt)"))
        .stderr(predicate::str::contains(NEWER_UID));
}

#[tokio::test(flavor = "multi_thread")]
async fn card_file_get_json_prints_saved_descriptor() {
    let api = MockServer::start().await;
    let storage = MockServer::start().await;
    mock_card(&api, &storage).await;
    mock_classic_download(&storage, 1).await;
    let tmp = tempfile::tempdir().unwrap();
    let cwd = tempfile::tempdir().unwrap();

    let out = kaiten(tmp.path(), &api.uri(), cwd.path())
        .args(["--json", "card", "file", "get", "67089469", "61256602"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(value["name"], "probe-attach.txt");
    assert_eq!(value["size"], 15);
    assert_eq!(value["mime_type"], "text/plain");
    // absolute, and it is where the bytes actually are (macOS temp dirs are
    // symlinked, so read through the reported path, not the tempdir)
    let path = std::path::Path::new(value["path"].as_str().unwrap());
    assert!(path.is_absolute(), "{value}");
    assert_eq!(std::fs::read_to_string(path).unwrap(), "attachment body");
}
