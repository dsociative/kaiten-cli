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
        .stdout(predicate::str::contains("token source: file"));
}
