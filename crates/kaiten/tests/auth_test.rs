use assert_cmd::Command;
use predicates::prelude::*;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const USER_CURRENT: &str = include_str!("fixtures/users_current.json");

fn kaiten(config_dir: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("kaiten").unwrap();
    cmd.env_remove("KAITEN_TOKEN")
        .env_remove("KAITEN_DOMAIN")
        .env_remove("KAITEN_BASE_URL")
        .env_remove("RUST_LOG")
        .env("KAITEN_CONFIG_DIR", config_dir)
        .env("NO_COLOR", "1");
    cmd
}

async fn mock_current_user(server: &MockServer, token: &str) {
    Mock::given(method("GET"))
        .and(path("/users/current"))
        .and(header("Authorization", format!("Bearer {token}").as_str()))
        .respond_with(ResponseTemplate::new(200).set_body_raw(USER_CURRENT, "application/json"))
        .expect(1)
        .mount(server)
        .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn login_with_flags_saves_config_with_0600() {
    let server = MockServer::start().await;
    mock_current_user(&server, "secret-token").await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path())
        .env("KAITEN_BASE_URL", server.uri())
        .args([
            "auth",
            "login",
            "--domain",
            "mycompany",
            "--token",
            "secret-token",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Logged in to mycompany.kaiten.ru as dxmuser",
        ));

    let config_path = tmp.path().join("config.toml");
    let body = std::fs::read_to_string(&config_path).unwrap();
    assert!(body.contains("domain = \"mycompany\""), "{body}");
    assert!(body.contains("token = \"secret-token\""), "{body}");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&config_path)
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "config.toml must be 0600");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn save_tightens_permissions_of_existing_file() {
    let server = MockServer::start().await;
    mock_current_user(&server, "secret-token").await;
    let tmp = tempfile::tempdir().unwrap();

    let config_path = tmp.path().join("config.toml");
    std::fs::write(&config_path, "domain = \"old\"\ntoken = \"old-token\"\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&config_path, std::fs::Permissions::from_mode(0o644)).unwrap();
    }

    kaiten(tmp.path())
        .env("KAITEN_BASE_URL", server.uri())
        .args([
            "auth",
            "login",
            "--domain",
            "mycompany",
            "--token",
            "secret-token",
        ])
        .assert()
        .success();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&config_path)
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(
            mode & 0o777,
            0o600,
            "pre-existing config.toml must be tightened to 0600"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn login_with_bad_token_does_not_save_config() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/current"))
        .respond_with(
            ResponseTemplate::new(401)
                .set_body_raw(r#"{"message":"Unauthorized"}"#, "application/json"),
        )
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path())
        .env("KAITEN_BASE_URL", server.uri())
        .args(["auth", "login", "--domain", "mycompany", "--token", "bad"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("401"));

    assert!(
        !tmp.path().join("config.toml").exists(),
        "config must not be written on failed login"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn status_reports_env_token_source() {
    let server = MockServer::start().await;
    mock_current_user(&server, "test-token").await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path())
        .env("KAITEN_BASE_URL", server.uri())
        .env("KAITEN_TOKEN", "test-token")
        .args(["auth", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("token source: env"))
        .stdout(predicate::str::contains("logged in as: dxmuser"));
}

#[tokio::test(flavor = "multi_thread")]
async fn status_reports_file_token_source_and_domain() {
    let server = MockServer::start().await;
    mock_current_user(&server, "file-token").await;
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("config.toml"),
        "domain = \"mycompany\"\ntoken = \"file-token\"\n",
    )
    .unwrap();

    kaiten(tmp.path())
        .env("KAITEN_BASE_URL", server.uri())
        .args(["auth", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("domain:       mycompany"))
        // the mock is only reachable through KAITEN_BASE_URL, which wins
        .stdout(predicate::str::contains("url source:   env"))
        .stdout(predicate::str::contains("token source: file"));
}

// --- issue #13: on-premise installations via `--base-url` / config `base_url` ---

#[tokio::test(flavor = "multi_thread")]
async fn login_with_base_url_saves_base_url_and_no_domain() {
    let server = MockServer::start().await;
    mock_current_user(&server, "secret-token").await;
    let tmp = tempfile::tempdir().unwrap();

    // deliberately no KAITEN_BASE_URL in the environment: the flag must work on its own
    kaiten(tmp.path())
        .args([
            "auth",
            "login",
            "--base-url",
            &format!("{}/", server.uri()),
            "--token",
            "secret-token",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "Logged in to {} as dxmuser",
            server.uri()
        )));

    let body = std::fs::read_to_string(tmp.path().join("config.toml")).unwrap();
    assert!(
        body.contains(&format!("base_url = \"{}\"", server.uri())),
        "{body}"
    );
    assert!(!body.contains("domain"), "{body}");
    assert!(body.contains("token = \"secret-token\""), "{body}");
}

#[tokio::test(flavor = "multi_thread")]
async fn login_with_base_url_replaces_a_previous_domain() {
    let server = MockServer::start().await;
    mock_current_user(&server, "new-token").await;
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("config.toml"),
        "domain = \"old\"\ntoken = \"old-token\"\n",
    )
    .unwrap();

    kaiten(tmp.path())
        .args([
            "auth",
            "login",
            "--base-url",
            &server.uri(),
            "--token",
            "new-token",
        ])
        .assert()
        .success();

    let body = std::fs::read_to_string(tmp.path().join("config.toml")).unwrap();
    assert!(
        !body.contains("domain"),
        "stale domain must be cleared: {body}"
    );
    assert!(body.contains("base_url = "), "{body}");
}

#[tokio::test(flavor = "multi_thread")]
async fn login_rejects_base_url_together_with_domain() {
    let tmp = tempfile::tempdir().unwrap();
    kaiten(tmp.path())
        .args([
            "auth",
            "login",
            "--base-url",
            "https://kaiten.corp.local/api/latest",
            "--domain",
            "mycompany",
            "--token",
            "t",
        ])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("cannot be used with"));
}

#[tokio::test(flavor = "multi_thread")]
async fn login_with_invalid_base_url_fails_before_any_request() {
    let server = MockServer::start().await; // no mocks
    let tmp = tempfile::tempdir().unwrap();
    kaiten(tmp.path())
        // the env hook must not rescue an invalid flag value
        .env("KAITEN_BASE_URL", server.uri())
        .args(["auth", "login", "--base-url", "not a url", "--token", "t"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("not a URL"));
    assert!(server.received_requests().await.unwrap().is_empty());
    assert!(!tmp.path().join("config.toml").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn login_with_base_url_missing_api_prefix_hints_and_saves_nothing() {
    let server = MockServer::start().await;
    // real servers answer with an HTML 404 page, not an empty body
    Mock::given(method("GET"))
        .and(path("/wrong/users/current"))
        .respond_with(ResponseTemplate::new(404).set_body_raw(
            "<!DOCTYPE html><html><body><h1>Not Found</h1></body></html>",
            "text/html",
        ))
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();
    kaiten(tmp.path())
        .args([
            "auth",
            "login",
            "--base-url",
            &format!("{}/wrong", server.uri()),
            "--token",
            "t",
        ])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::starts_with(
            "kaiten: base URL must include the API prefix",
        ))
        .stderr(predicate::str::contains("HTTP 404"))
        .stderr(predicate::str::contains("api/latest"));
    assert!(!tmp.path().join("config.toml").exists());
}

/// Kaiten cloud's web root answers 200 with the SPA page for any path — a
/// base URL without the API prefix then fails to decode, not with a 404.
#[tokio::test(flavor = "multi_thread")]
async fn login_with_base_url_pointing_at_the_web_app_hints_too() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/current"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw("<!DOCTYPE html><html><body>app</body></html>", "text/html"),
        )
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();
    kaiten(tmp.path())
        .args(["auth", "login", "--base-url", &server.uri(), "--token", "t"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::starts_with(
            "kaiten: base URL must include the API prefix",
        ))
        .stderr(predicate::str::contains("not JSON"));
    assert!(!tmp.path().join("config.toml").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn login_rejects_base_url_with_query_or_fragment_or_non_http_scheme() {
    let tmp = tempfile::tempdir().unwrap();
    for bad in [
        "https://host/api/latest?x=1",
        "https://host/api/latest#frag",
        "ftp://host/api/latest",
    ] {
        kaiten(tmp.path())
            .args(["auth", "login", "--base-url", bad, "--token", "t"])
            .assert()
            .failure()
            .code(1)
            .stderr(predicate::str::contains("http(s) URL"));
    }
    assert!(!tmp.path().join("config.toml").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn login_flag_base_url_wins_over_env_base_url() {
    let flag_server = MockServer::start().await;
    let env_server = MockServer::start().await;
    mock_current_user(&flag_server, "t").await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path())
        .env("KAITEN_BASE_URL", env_server.uri())
        .args([
            "auth",
            "login",
            "--base-url",
            &flag_server.uri(),
            "--token",
            "t",
        ])
        .assert()
        .success();
    assert!(env_server.received_requests().await.unwrap().is_empty());
    let body = std::fs::read_to_string(tmp.path().join("config.toml")).unwrap();
    assert!(body.contains(&flag_server.uri()), "{body}");
}

#[tokio::test(flavor = "multi_thread")]
async fn login_with_domain_clears_a_previous_base_url_and_writes_no_base_url_line() {
    let server = MockServer::start().await;
    mock_current_user(&server, "new-token").await;
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("config.toml"),
        "base_url = \"https://old.example/api/latest\"\ntoken = \"old-token\"\n",
    )
    .unwrap();

    kaiten(tmp.path())
        .env("KAITEN_BASE_URL", server.uri())
        .args([
            "auth",
            "login",
            "--domain",
            "mycompany",
            "--token",
            "new-token",
        ])
        .assert()
        .success();
    let body = std::fs::read_to_string(tmp.path().join("config.toml")).unwrap();
    assert!(!body.contains("base_url"), "{body}");
    assert!(body.contains("domain = \"mycompany\""), "{body}");
}

#[tokio::test(flavor = "multi_thread")]
async fn status_reports_env_base_url_over_file_base_url_and_env_domain_source() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/current"))
        .and(header("Authorization", "Bearer file-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(USER_CURRENT, "application/json"))
        .expect(2)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("config.toml"),
        "base_url = \"https://ignored.example/api/latest\"\ntoken = \"file-token\"\n",
    )
    .unwrap();

    // KAITEN_BASE_URL beats the file base_url
    kaiten(tmp.path())
        .env("KAITEN_BASE_URL", server.uri())
        .args(["auth", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("url source:   env\n"));

    // --json carries the same value (KAITEN_DOMAIN set too: the base URL still wins)
    let out = kaiten(tmp.path())
        .env("KAITEN_DOMAIN", "envdomain")
        .env("KAITEN_BASE_URL", server.uri())
        .args(["--json", "auth", "status"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(value["base_url_source"], "env", "{value}");
    assert_eq!(value["domain"], "envdomain", "{value}");
}

/// Valid JSON that does not fit the `User` model is a model mismatch, not a
/// wrong base URL — the API-prefix hint must not claim otherwise.
#[tokio::test(flavor = "multi_thread")]
async fn login_with_base_url_and_mismatched_json_reports_the_decode_error_without_the_hint() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/current"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(r#"{"foo": 1}"#, "application/json"))
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();
    kaiten(tmp.path())
        .args(["auth", "login", "--base-url", &server.uri(), "--token", "t"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("failed to decode response"))
        .stderr(predicate::str::contains("API prefix").not());
    assert!(!tmp.path().join("config.toml").exists());
}

/// Credentials in the URL can never work (the bearer header wins) and must
/// not end up in config.toml or on stdout.
#[tokio::test(flavor = "multi_thread")]
async fn login_rejects_base_url_with_userinfo() {
    let tmp = tempfile::tempdir().unwrap();
    for bad in [
        "https://user:secret@host/api/latest",
        "https://user@host/api/latest",
    ] {
        kaiten(tmp.path())
            .args(["auth", "login", "--base-url", bad, "--token", "t"])
            .assert()
            .failure()
            .code(1)
            .stderr(predicate::str::contains("credentials"))
            .stderr(predicate::str::contains("secret").not());
    }
    assert!(!tmp.path().join("config.toml").exists());
}

/// The on-premise happy path: only `base_url` in the file.
#[tokio::test(flavor = "multi_thread")]
async fn status_reports_file_base_url_source() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/current"))
        .and(header("Authorization", "Bearer file-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(USER_CURRENT, "application/json"))
        .expect(2)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("config.toml"),
        format!("base_url = \"{}\"\ntoken = \"file-token\"\n", server.uri()),
    )
    .unwrap();

    kaiten(tmp.path())
        .args(["auth", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("domain:       -"))
        .stdout(predicate::str::contains(format!(
            "base_url:     {}\n",
            server.uri()
        )))
        .stdout(predicate::str::contains("url source:   file\n"))
        .stdout(predicate::str::contains("token source: file"));

    let out = kaiten(tmp.path())
        .args(["--json", "auth", "status"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(value["base_url_source"], "file", "{value}");
    assert_eq!(value["base_url"], server.uri(), "{value}");
    assert!(value["domain"].is_null(), "{value}");
}
