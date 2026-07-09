# kaiten CLI + MCP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Консольная утилита `kaiten` (в стиле gh/glab) и MCP-сервер для трекера Kaiten с типизированным клиентом, полным покрытием тестами и удобной отладкой.

**Architecture:** Cargo workspace из двух крейтов: `kaiten-client` (типизированный API-клиент: reqwest + serde, thiserror-ошибки, 429-ретрай, tracing) и `kaiten` (бинарник: clap-CLI + MCP-сервер на rmcp как сабкоманда `mcp serve`). Вся работа с HTTP изолирована в клиенте; CLI и MCP — тонкие слои поверх него.

**Tech Stack:** Rust (edition 2024), tokio, reqwest (rustls), serde/serde_json/serde_path_to_error, thiserror, clap v4 + clap_complete, rmcp (официальный MCP SDK, stdio), tracing, comfy-table, wiremock/assert_cmd/insta.

**Спека:** `docs/superpowers/specs/2026-07-09-kaiten-cli-mcp-design.md`

## Global Constraints

- Ошибки ТОЛЬКО через `thiserror` (типизированные enum). **`anyhow` запрещён во всех крейтах.**
- Коммиты: conventional commits на английском, **БЕЗ трейлера Co-Authored-By** и упоминаний Claude.
- Никаких упоминаний работодателя пользователя в файлах проекта; в примерах — домен `mycompany`.
- База API: `https://{domain}.kaiten.ru/api/latest`; авторизация `Authorization: Bearer <token>`; токен никогда не логируется и не попадает в фикстуры/доки.
- Толерантная десериализация: неизвестные поля игнорируются, отсутствующие — `Option`/`#[serde(default)]`; даты — String (без chrono).
- Факты реального API (проверены живьём): все успехи — HTTP 200; 400 → `{"message": "..."}`; 403 — ПУСТОЕ тело (в т.ч. для несуществующих карточек — 404 не бывает); `GET /cards/{id}/checklists` не существует (405), чеклисты приходят внутри `GET /cards/{id}`; `POST /cards/{id}/tags` принимает `{"name": "..."}`.
- Тесты: клиент — wiremock; CLI — assert_cmd + wiremock (env: `KAITEN_BASE_URL`, `KAITEN_TOKEN=test-token`, `KAITEN_CONFIG_DIR=<tempdir>`, `NO_COLOR=1`); снапшоты — insta.
- Логи только в stderr; stdout — только данные (таблицы/JSON/протокол MCP).
- Проверка каждой задачи: `cargo test -p <crate>`; финал каждой вехи: `cargo clippy --all-targets -- -D warnings` и `cargo fmt --all -- --check`.
- Живые smoke-тесты — на тестовом инстансе `dstest` через `source .env.test` (файл в .gitignore); реальные ID там: user 1068514, space 810671, board 1826109.

---

### Task 1: Workspace scaffolding

Превращаем текущий одиночный пакет (корневой `Cargo.toml` c `[package] kaiten-cli` + `src/main.rs`) в cargo workspace из двух крейтов по INTERFACES.md §1. Логики нет — TDD-цикл не нужен, но проверочные команды обязательны. Старые `src/` и `Cargo.toml` не закоммичены (untracked), поэтому их можно просто удалить/переписать без `git rm`.

Важно: `kaiten-client` получает `tokio` не только в dev-dependencies, но и в `[dependencies]` с дополнительной фичей `time` — ядру клиента нужен `tokio::time::sleep` для паузы 429-ретрая (Task 2); фичи добавляются поверх воркспейсных (`macros`, `rt-multi-thread`), это штатный механизм cargo.

**Files:**
- Create: Cargo.toml (корневой, workspace — перезаписывает существующий)
- Create: crates/kaiten-client/Cargo.toml
- Create: crates/kaiten-client/src/lib.rs
- Create: crates/kaiten/Cargo.toml
- Create: crates/kaiten/src/main.rs
- Modify: src/ — удалить целиком (заглушка hello world переезжает в crates/kaiten/src/main.rs)
- Test: — (логики нет; проверка — `cargo build` и `cargo test`)

**Interfaces:**
- Consumes: — (первая задача)
- Produces: workspace с членами `crates/kaiten-client` (lib `kaiten_client`) и `crates/kaiten` (bin `kaiten`); секция `[workspace.dependencies]` — единственное место версий для всех последующих задач. Здесь же пинуются `rmcp = { version = "2", features = ["server", "macros", "transport-io"] }` и `schemars = "1"` (актуальная rmcp — 2.2.0; макросы `#[tool]`/`#[tool_router]`/`#[tool_handler]` и `Parameters<T>` подтверждены по docs.rs) — крейт `kaiten` подключит их в Task 21 строками `rmcp = { workspace = true }` и `schemars = { workspace = true }` (НЕ `cargo add`)

- [ ] **Step 1: Переписать корневой Cargo.toml на workspace-манифест и удалить старый src**

Полное содержимое корневого `Cargo.toml` (перезаписать существующий файл):

```toml
[workspace]
resolver = "2"
members = ["crates/kaiten-client", "crates/kaiten"]

[workspace.package]
version = "0.1.0"
edition = "2024"

[workspace.dependencies]
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_path_to_error = "0.1"
thiserror = "2"
url = "2"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
clap = { version = "4", features = ["derive", "env"] }
clap_complete = "4"
comfy-table = "7"
toml = "0.8"
dirs = "6"
rpassword = "7"
rmcp = { version = "2", features = ["server", "macros", "transport-io"] }
schemars = "1"
wiremock = "0.6"
assert_cmd = "2"
predicates = "3"
insta = { version = "1", features = ["filters"] }
tempfile = "3"
```

Удалить старый каталог с заглушкой (untracked, git-операций не требуется):

Run: `rm -rf src`

- [ ] **Step 2: Создать крейт kaiten-client**

Run: `mkdir -p crates/kaiten-client/src`

`crates/kaiten-client/Cargo.toml` (полностью):

```toml
[package]
name = "kaiten-client"
version.workspace = true
edition.workspace = true

[dependencies]
reqwest = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
serde_path_to_error = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true, features = ["time"] }
tracing = { workspace = true }
url = { workspace = true }

[dev-dependencies]
tokio = { workspace = true }
wiremock = { workspace = true }
```

`crates/kaiten-client/src/lib.rs` (полностью):

```rust
//! Typed client for the Kaiten API (<https://developers.kaiten.ru>).
```

- [ ] **Step 3: Создать крейт kaiten (бинарник)**

Run: `mkdir -p crates/kaiten/src`

`crates/kaiten/Cargo.toml` (полностью):

```toml
[package]
name = "kaiten"
version.workspace = true
edition.workspace = true

[[bin]]
name = "kaiten"
path = "src/main.rs"

[dependencies]
kaiten-client = { path = "../kaiten-client" }
clap = { workspace = true }
clap_complete = { workspace = true }
comfy-table = { workspace = true }
dirs = { workspace = true }
rpassword = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
toml = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
url = { workspace = true }

[dev-dependencies]
assert_cmd = { workspace = true }
insta = { workspace = true }
predicates = { workspace = true }
tempfile = { workspace = true }
wiremock = { workspace = true }
```

`crates/kaiten/src/main.rs` (полностью, временная заглушка — заменяется в задаче про CLI-скелет):

```rust
fn main() {
    println!("kaiten: work in progress");
}
```

- [ ] **Step 4: Проверить сборку**

Run: `cargo build`
Expected: успех; в выводе `Compiling kaiten-client v0.1.0` и `Compiling kaiten v0.1.0`, в конце `Finished \`dev\` profile`. Первый прогон долгий — качаются зависимости.

- [ ] **Step 5: Проверить прогон тестов**

Run: `cargo test`
Expected: PASS; для обоих крейтов `running 0 tests` и `test result: ok. 0 passed; 0 failed`.

- [ ] **Step 6: Проверить .gitignore**

Run: `cat .gitignore`
Expected: файл содержит строки `/target` и `.env.test` (обе уже есть; если `/target` вдруг нет — добавить такой строкой первой).

- [ ] **Step 7: Commit**

Run: `git add .gitignore Cargo.toml Cargo.lock crates && git commit -m "chore: scaffold cargo workspace"`
Expected: коммит создан; `git status` не показывает untracked-файлов проекта (`.env.test` игнорируется).

---

### Task 2: KaitenError + HTTP-ядро KaitenClient

Два коммита. Сначала enum ошибок — точно по INTERFACES.md §2 (тест на Display-форматы; Display у `Api` печатает ТОЛЬКО `message`, а поле `body` несёт сырое тело ответа целиком — его CLI в Task 10 допечатает в stderr отдельно). Затем HTTP-ядро: `KaitenClient::new` (парс base_url) и единый внутренний помощник `pub(crate) async fn send_with_retry(&self, method, path, query, body) -> Result<(u16, String)>` — Bearer-заголовок, 429-ретрай до 3 повторов, tracing без токена, маппинг 4xx/5xx в `Api { status, message, body }`; на 2xx возвращает статус и сырое тело. Поверх него строятся `pub(crate) request<T>` (десериализация через `serde_path_to_error`) и публичный `raw()` для `kaiten api`; `request_empty` в Task 8 сядет на то же ядро и получит ретрай и trace бесплатно.

Покрываемые кейсы:

1. Bearer-заголовок на запросе; успешный GET возвращает JSON.
2. 400 с JSON-телом `{"message": ...}` → `Api { status: 400, message: <из поля message>, body: <весь JSON как строка> }`.
3. 403 с пустым телом → `Api { status: 403, message: "Forbidden" (canonical reason), body: "" }`.
4. 500 с не-JSON телом → `Api { status: 500, message: <тело как есть>, body: <то же тело> }`.
5. 429 → ретрай → успех со второй попытки.
6. 429 на всех 4 запросах (1 + 3 ретрая) → `RateLimited { retry_after_secs }` с ФАКТИЧЕСКИМ значением `X-RateLimit-Reset` последнего ответа.
7. Ошибка десериализации → `Decode` с путём до поля.
8. `raw()` POST отправляет JSON-тело и возвращает `serde_json::Value`.

Кейсы 1–6 и 8 тестируются через `raw()` в интеграционном тесте; кейс 7 — юнит-тестом внутри `src/client.rs`, где доступен `pub(crate) request<T>`. Никакого публичного `get_json` не делаем — фасады следующих задач ходят в `request` напрямую.

**Files:**
- Create: crates/kaiten-client/src/error.rs
- Create: crates/kaiten-client/src/client.rs
- Create: crates/kaiten-client/tests/fixtures/user_current.json
- Modify: crates/kaiten-client/src/lib.rs — объявить модули и pub use
- Test: crates/kaiten-client/tests/client_test.rs
- Test: crates/kaiten-client/src/client.rs (юнит-тест `decode_error_reports_field_path` в `#[cfg(test)]`)

**Interfaces:**
- Consumes: workspace и манифест kaiten-client из Task 1 (deps: reqwest, serde, serde_json, serde_path_to_error, thiserror, tokio+time, tracing, url; dev: tokio, wiremock)
- Produces:
  - `pub type Result<T> = std::result::Result<T, KaitenError>;`
  - `pub enum KaitenError { Api { status: u16, message: String, body: String }, RateLimited { retry_after_secs: u64 }, Network(reqwest::Error), Decode { path: String, source: serde_json::Error }, InvalidBaseUrl(String) }` — `body` у `Api` = сырое тело ответа целиком (пустая строка, если тела нет); Display печатает только `message`
  - `impl KaitenClient { pub fn new(base_url: &str, token: &str) -> Result<Self> }`
  - `pub(crate) async fn send_with_retry(&self, method: reqwest::Method, path: &str, query: Option<Vec<(String, String)>>, body: Option<serde_json::Value>) -> Result<(u16, String)>` — единое ядро (Bearer, 429-ретрай, tracing, маппинг ошибок); на него садится `request<T>` здесь и `request_empty` в Task 8 (DELETE-эндпоинты получают ретрай и trace бесплатно)
  - `pub(crate) async fn request<T: DeserializeOwned>(&self, method: reqwest::Method, path: &str, query: Option<Vec<(String, String)>>, body: Option<serde_json::Value>) -> Result<T>` — им пользуются ВСЕ фасады api/* следующих задач
  - `pub async fn raw(&self, method: reqwest::Method, path: &str, body: Option<serde_json::Value>) -> Result<serde_json::Value>` — для команды `kaiten api`
  - фикстура `tests/fixtures/user_current.json` — создаётся ЗДЕСЬ и ПЕРЕИСПОЛЬЗУЕТСЯ Task 3 для теста `users().current()` (Task 3 НЕ создаёт дубликат этой фикстуры, только добавляет `users_list.json`)

- [ ] **Step 1: Failing test на Display-форматы KaitenError**

`crates/kaiten-client/tests/client_test.rs` (полностью):

```rust
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
```

- [ ] **Step 2: Убедиться, что тест падает**

Run: `cargo test -p kaiten-client --test client_test`
Expected: FAIL (не компилируется), `error[E0432]: unresolved import kaiten_client::KaitenError`.

- [ ] **Step 3: Реализовать error.rs и подключить в lib.rs**

`crates/kaiten-client/src/error.rs` (полностью, ТОЧНО по контракту):

```rust
pub type Result<T> = std::result::Result<T, KaitenError>;

#[derive(Debug, thiserror::Error)]
pub enum KaitenError {
    /// message — из JSON-поля "message" (или reason-фраза при пустом теле);
    /// body — сырое тело ответа целиком (пустая строка, если тела нет).
    #[error("API error {status}: {message}")]
    Api { status: u16, message: String, body: String },
    #[error("rate limited, retry after {retry_after_secs}s")]
    RateLimited { retry_after_secs: u64 },
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("failed to decode response at `{path}`: {source}")]
    Decode {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("invalid base url: {0}")]
    InvalidBaseUrl(String),
}
```

`crates/kaiten-client/src/lib.rs` (полностью):

```rust
//! Typed client for the Kaiten API (<https://developers.kaiten.ru>).

mod error;

pub use error::{KaitenError, Result};
```

- [ ] **Step 4: Убедиться, что тест проходит**

Run: `cargo test -p kaiten-client --test client_test`
Expected: PASS, `test error_display_formats ... ok`, `1 passed; 0 failed`.

- [ ] **Step 5: Commit (первый)**

Run: `git add crates/kaiten-client/src/error.rs crates/kaiten-client/src/lib.rs crates/kaiten-client/tests/client_test.rs && git commit -m "feat(client): typed error enum"`

- [ ] **Step 6: Добавить фикстуру user_current.json**

Урезанный реальный ответ `GET /users/current` (все поля будущей модели `User` + 5 лишних полей для проверки толерантности; email обезличен). Эту же фикстуру Task 3 переиспользует для теста `users().current()` — дубликат там не создавать.

Run: `mkdir -p crates/kaiten-client/tests/fixtures`

`crates/kaiten-client/tests/fixtures/user_current.json` (полностью):

```json
{
  "id": 1068514,
  "uid": "0abd61ea-9dc5-40eb-b0a9-0d452ba1d8a6",
  "full_name": "dxmuser",
  "username": "dxmuser",
  "email": "user@example.com",
  "activated": true,
  "lng": "ru",
  "timezone": "UTC",
  "theme": "auto",
  "created": "2026-07-09T15:13:12.241Z",
  "company_id": 398610
}
```

- [ ] **Step 7: Failing tests на HTTP-ядро (кейсы 1–6, 8 + невалидный base_url)**

Переписать `crates/kaiten-client/tests/client_test.rs` целиком (тест Display из Step 1 сохраняется):

```rust
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

#[test]
fn new_rejects_invalid_base_url() {
    let err = KaitenClient::new("not a url", "test-token").unwrap_err();
    assert!(matches!(err, KaitenError::InvalidBaseUrl(_)), "got: {err:?}");
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
    let value = client.raw(Method::GET, "/users/current", None).await.unwrap();

    assert_eq!(value["id"], 1068514);
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
        KaitenError::Api { status, message, body } => {
            assert_eq!(status, 400);
            assert_eq!(message, "Card should have required property 'board_id'");
            assert_eq!(body, r#"{"message":"Card should have required property 'board_id'"}"#);
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
    let err = client.raw(Method::GET, "/cards/999", None).await.unwrap_err();

    match err {
        KaitenError::Api { status, message, body } => {
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
    let err = client.raw(Method::GET, "/users/current", None).await.unwrap_err();

    match err {
        KaitenError::Api { status, message, body } => {
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
    let value = client.raw(Method::GET, "/users/current", None).await.unwrap();

    assert_eq!(value["id"], 1068514);
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
    let err = client.raw(Method::GET, "/users/current", None).await.unwrap_err();

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
        .and(body_json(json!({"board_id": 1826109, "title": "from raw"})))
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
            Some(json!({"board_id": 1826109, "title": "from raw"})),
        )
        .await
        .unwrap();

    assert_eq!(value["id"], 67089469);
    assert_eq!(value["title"], "from raw");
}
```

- [ ] **Step 8: Убедиться, что тесты падают**

Run: `cargo test -p kaiten-client --test client_test`
Expected: FAIL (не компилируется), `error[E0432]: unresolved import kaiten_client::KaitenClient`.

- [ ] **Step 9: Реализовать client.rs и подключить в lib.rs**

`crates/kaiten-client/src/client.rs` (полностью):

```rust
use std::time::{Duration, Instant};

use reqwest::Method;
use serde::de::DeserializeOwned;

use crate::error::{KaitenError, Result};

const MAX_RETRIES: u32 = 3;

/// HTTP client for the Kaiten API.
pub struct KaitenClient {
    http: reqwest::Client,
    base_url: url::Url,
    token: String,
}

impl KaitenClient {
    /// `base_url` WITHOUT a trailing slash, e.g. "https://mycompany.kaiten.ru/api/latest".
    pub fn new(base_url: &str, token: &str) -> Result<Self> {
        let parsed = url::Url::parse(base_url)
            .map_err(|e| KaitenError::InvalidBaseUrl(format!("{base_url}: {e}")))?;
        Ok(Self {
            http: reqwest::Client::builder().build()?,
            base_url: parsed,
            token: token.to_string(),
        })
    }

    /// Raw request for `kaiten api`: `path` starts with "/", query is already in `path`.
    pub async fn raw(
        &self,
        method: Method,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        self.request(method, path, None, body).await
    }

    /// Retry-and-trace core shared by ALL requests (JSON and empty responses alike).
    /// Returns `(status, raw response body)` on 2xx; maps 4xx/5xx (except 429) to
    /// `Api { status, message, body }` and an exhausted 429 to `RateLimited`.
    /// `request_empty` (Task 8) also builds on this core.
    pub(crate) async fn send_with_retry(
        &self,
        method: Method,
        path: &str,
        query: Option<Vec<(String, String)>>,
        body: Option<serde_json::Value>,
    ) -> Result<(u16, String)> {
        let url = format!("{}{}", self.base_url.as_str().trim_end_matches('/'), path);
        let mut retries = 0u32;
        loop {
            let mut req = self
                .http
                .request(method.clone(), url.as_str())
                .bearer_auth(&self.token);
            if let Some(q) = &query {
                req = req.query(q);
            }
            if let Some(b) = &body {
                tracing::trace!(body = %b, "request body");
                req = req.json(b);
            }

            let started = Instant::now();
            let resp = req.send().await?;
            let status = resp.status();
            tracing::debug!(
                method = %method,
                path,
                status = status.as_u16(),
                elapsed_ms = started.elapsed().as_millis() as u64,
                "http request"
            );

            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                // The error carries the ACTUAL header value (missing/garbage -> 1);
                // only the sleep below is clamped to <=5s.
                let reset_secs = resp
                    .headers()
                    .get("X-RateLimit-Reset")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(1);
                retries += 1;
                if retries > MAX_RETRIES {
                    return Err(KaitenError::RateLimited {
                        retry_after_secs: reset_secs,
                    });
                }
                let wait_secs = if reset_secs <= 5 { reset_secs } else { 1 };
                tracing::debug!(wait_secs, retry = retries, "rate limited, retrying");
                tokio::time::sleep(Duration::from_secs(wait_secs)).await;
                continue;
            }

            let text = resp.text().await?;
            tracing::trace!(body = %text, "response body");

            if !status.is_success() {
                let message = serde_json::from_str::<serde_json::Value>(&text)
                    .ok()
                    .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(str::to_owned))
                    .unwrap_or_else(|| {
                        if text.trim().is_empty() {
                            status
                                .canonical_reason()
                                .unwrap_or("unknown error")
                                .to_owned()
                        } else {
                            text.clone()
                        }
                    });
                return Err(KaitenError::Api {
                    status: status.as_u16(),
                    message,
                    body: text,
                });
            }

            return Ok((status.as_u16(), text));
        }
    }

    pub(crate) async fn request<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        query: Option<Vec<(String, String)>>,
        body: Option<serde_json::Value>,
    ) -> Result<T> {
        let (_status, text) = self.send_with_retry(method, path, query, body).await?;
        let mut de = serde_json::Deserializer::from_str(&text);
        serde_path_to_error::deserialize(&mut de).map_err(|e| KaitenError::Decode {
            path: e.path().to_string(),
            source: e.into_inner(),
        })
    }
}
```

`crates/kaiten-client/src/lib.rs` (полностью):

```rust
//! Typed client for the Kaiten API (<https://developers.kaiten.ru>).

mod client;
mod error;

pub use client::KaitenClient;
pub use error::{KaitenError, Result};
```

- [ ] **Step 10: Убедиться, что интеграционные тесты проходят**

Run: `cargo test -p kaiten-client --test client_test`
Expected: PASS, `9 passed; 0 failed` (прогон ~1–2 c: тест ретрая спит 1 секунду по X-RateLimit-Reset).

- [ ] **Step 11: Юнит-тест на Decode с путём до поля (кейс 7)**

Добавить в КОНЕЦ файла `crates/kaiten-client/src/client.rs` (после закрывающей скобки `impl KaitenClient`) следующий блок целиком:

```rust
#[cfg(test)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::KaitenClient;
    use crate::error::KaitenError;

    #[derive(Debug, serde::Deserialize)]
    struct Probe {
        #[allow(dead_code)]
        id: u64,
    }

    #[tokio::test]
    async fn decode_error_reports_field_path() {
        let server = MockServer::start().await;
        // id приходит строкой вместо числа — Decode должен указать поле "id".
        Mock::given(method("GET"))
            .and(path("/probe"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                r#"{"id":"not-a-number","title":"x","extra":true}"#,
                "application/json",
            ))
            .expect(1)
            .mount(&server)
            .await;

        let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
        let err = client
            .request::<Probe>(reqwest::Method::GET, "/probe", None, None)
            .await
            .unwrap_err();

        match err {
            KaitenError::Decode { path, .. } => assert_eq!(path, "id"),
            other => panic!("expected Decode error, got {other:?}"),
        }
    }
}
```

- [ ] **Step 12: Полный прогон тестов крейта**

Run: `cargo test -p kaiten-client`
Expected: PASS; юнит-тесты либы — `1 passed` (`decode_error_reports_field_path`), интеграционные (`client_test`) — `9 passed`.

- [ ] **Step 13: Линт и форматирование**

Run: `cargo clippy --all-targets -- -D warnings && cargo fmt --all -- --check`
Expected: оба — без ворнингов и без diff, exit code 0.

- [ ] **Step 14: Commit (второй)**

Run: `git add crates/kaiten-client/src/client.rs crates/kaiten-client/src/lib.rs crates/kaiten-client/tests/client_test.rs crates/kaiten-client/tests/fixtures/user_current.json && git commit -m "feat(client): http core with auth, retry and tracing"`

---

### Task 3: User model + users API

**Files:**
- Create: crates/kaiten-client/src/models.rs
- Create: crates/kaiten-client/src/api/mod.rs
- Create: crates/kaiten-client/src/api/users.rs
- Create: crates/kaiten-client/tests/fixtures/users_list.json
- Modify: crates/kaiten-client/src/lib.rs:добавить `pub mod api; pub mod models;` и `pub use models::*;`
- Modify: crates/kaiten-client/src/client.rs:добавить метод `users()` в `impl KaitenClient`
- Test: crates/kaiten-client/tests/users_test.rs

**Interfaces:**
- Consumes (из Task 2):
  - `KaitenClient::new(base_url: &str, token: &str) -> Result<Self>`
  - `pub(crate) async fn request<T: serde::de::DeserializeOwned>(&self, method: reqwest::Method, path: &str, query: Option<Vec<(String, String)>>, body: Option<serde_json::Value>) -> Result<T>`
  - фикстура `crates/kaiten-client/tests/fixtures/user_current.json` (создана в Task 2, здесь переиспользуется для теста `current()`)
  - dev-dependencies `tokio`/`wiremock` в `crates/kaiten-client/Cargo.toml` (заведены в Task 1)
- Produces:
  - `models::User` (реэкспорт `kaiten_client::User`)
  - `api::users::Users<'a>` с `current() -> Result<User>` и `list() -> Result<Vec<User>>`
  - `KaitenClient::users(&self) -> Users<'_>`

Фасады — тонкие структуры с единственным полем `client: &KaitenClient` (`pub(crate)`), конструируются только методами `KaitenClient::users()` и т.п. Каждый фасад — в своём файле `api/*.rs`; все модели — в общем `models.rs`, который дополняется в каждой задаче.

- [ ] **Step 1: Создать фикстуру users_list.json**

Для теста `current()` НЕ создаём новую фикстуру — переиспользуем существующую `crates/kaiten-client/tests/fixtures/user_current.json` из Task 2 (урезанный реальный ответ `GET /users/current`). Здесь создаётся только `users_list.json` — урезанная версия реального ответа `GET /users`: все поля модели `User` + лишние поля для проверки толерантности; email заменён на `user@example.com`. Второй элемент списка урезан до минимума — проверяем `Option = None`.

`crates/kaiten-client/tests/fixtures/users_list.json`:

```json
[
  {
    "id": 1068514,
    "uid": "0abd61ea-9dc5-40eb-b0a9-0d452ba1d8a6",
    "full_name": "dxmuser",
    "username": "dxmuser",
    "email": "user@example.com",
    "activated": true,
    "lng": "ru",
    "timezone": "UTC",
    "theme": "auto",
    "virtual": false,
    "role": 1
  },
  {
    "id": 1068515,
    "uid": "11111111-2222-3333-4444-555555555555",
    "username": "bot",
    "activated": false,
    "virtual": true
  }
]
```

- [ ] **Step 2: Написать failing test**

`crates/kaiten-client/tests/users_test.rs` (полностью):

```rust
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
```

- [ ] **Step 3: Убедиться, что тест падает**

Run: `cargo test -p kaiten-client --test users_test`
Expected: FAIL, `error[E0599]: no method named 'users' found for struct 'KaitenClient'`

- [ ] **Step 4: Создать models.rs с моделью User**

`crates/kaiten-client/src/models.rs` (полностью):

```rust
//! All Kaiten API models.
//!
//! Deserialization is tolerant: unknown fields are ignored (no
//! `deny_unknown_fields`), fields that may be absent in a particular
//! response are `Option<...>` with `#[serde(default)]`.
//! Dates are plain ISO strings (no chrono).

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct User {
    pub id: u64,
    pub uid: String,
    #[serde(default)]
    pub full_name: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub activated: Option<bool>,
}
```

- [ ] **Step 5: Создать фасад Users и подключить модули**

`crates/kaiten-client/src/api/mod.rs` (полностью):

```rust
pub mod users;
```

`crates/kaiten-client/src/api/users.rs` (полностью):

```rust
use crate::client::KaitenClient;
use crate::error::Result;
use crate::models::User;

/// Users resource facade. Construct via [`KaitenClient::users`].
pub struct Users<'a> {
    pub(crate) client: &'a KaitenClient,
}

impl Users<'_> {
    /// GET /users/current
    pub async fn current(&self) -> Result<User> {
        self.client
            .request(reqwest::Method::GET, "/users/current", None, None)
            .await
    }

    /// GET /users
    pub async fn list(&self) -> Result<Vec<User>> {
        self.client
            .request(reqwest::Method::GET, "/users", None, None)
            .await
    }
}
```

В `crates/kaiten-client/src/lib.rs` добавить (к существующим объявлениям модулей и реэкспортам из Task 1/2):

```rust
pub mod api;
pub mod models;

pub use models::*;
```

В `crates/kaiten-client/src/client.rs` добавить внутрь существующего блока `impl KaitenClient`:

```rust
    /// Users resource facade.
    pub fn users(&self) -> crate::api::users::Users<'_> {
        crate::api::users::Users { client: self }
    }
```

- [ ] **Step 6: Убедиться, что тест проходит**

Run: `cargo test -p kaiten-client --test users_test`
Expected: PASS, `test result: ok. 2 passed`

- [ ] **Step 7: Commit**

```
git add crates/kaiten-client/src/models.rs crates/kaiten-client/src/api/mod.rs crates/kaiten-client/src/api/users.rs crates/kaiten-client/src/lib.rs crates/kaiten-client/src/client.rs crates/kaiten-client/tests/users_test.rs crates/kaiten-client/tests/fixtures/users_list.json
git commit -m "feat(client): users api"
```

---

### Task 4: Space model + spaces().list()

**Files:**
- Create: crates/kaiten-client/src/api/spaces.rs
- Create: crates/kaiten-client/tests/fixtures/spaces_list.json
- Modify: crates/kaiten-client/src/models.rs:добавить struct Space
- Modify: crates/kaiten-client/src/api/mod.rs:добавить `pub mod spaces;`
- Modify: crates/kaiten-client/src/client.rs:добавить метод `spaces()`
- Test: crates/kaiten-client/tests/spaces_test.rs

**Interfaces:**
- Consumes: `KaitenClient::new`, `KaitenClient::request` (Task 2); паттерн фасада из Task 3
- Produces: `models::Space`; `api::spaces::Spaces<'a>` с `list() -> Result<Vec<Space>>`; `KaitenClient::spaces(&self) -> Spaces<'_>`

- [ ] **Step 1: Создать фикстуру spaces_list.json**

Урезано из `fixtures/spaces_list.json`; второй элемент — реальное пространство `kaiten-cli-test` (из card_get_full → path_data.space), урезанное до минимума полей: проверяем `archived = None`.

`crates/kaiten-client/tests/fixtures/spaces_list.json`:

```json
[
  {
    "id": 810669,
    "uid": "f52db47b-cbd9-4b50-98e7-19219cae0291",
    "title": "Первое пространство",
    "archived": false,
    "created": "2026-07-09T15:13:28.322Z",
    "updated": "2026-07-09T15:13:29.995Z",
    "company_id": 398610,
    "entity_type": "space",
    "access": "by_invite",
    "sort_order": 1.6486044425519069
  },
  {
    "id": 810671,
    "uid": "8d5463f5-0752-4a08-b074-99d9617fbd4e",
    "title": "kaiten-cli-test",
    "entity_type": "space",
    "path": "398610"
  }
]
```

- [ ] **Step 2: Написать failing test**

`crates/kaiten-client/tests/spaces_test.rs` (полностью):

```rust
use kaiten_client::KaitenClient;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SPACES_LIST: &str = include_str!("fixtures/spaces_list.json");

#[tokio::test]
async fn list_parses_spaces_and_tolerates_missing_fields() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/spaces"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(SPACES_LIST, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let spaces = client.spaces().list().await.unwrap();

    assert_eq!(spaces.len(), 2);
    assert_eq!(spaces[0].id, 810669);
    assert_eq!(spaces[0].title, "Первое пространство");
    assert_eq!(spaces[0].archived, Some(false));
    assert_eq!(spaces[1].title, "kaiten-cli-test");
    assert_eq!(spaces[1].archived, None);
}
```

- [ ] **Step 3: Убедиться, что тест падает**

Run: `cargo test -p kaiten-client --test spaces_test`
Expected: FAIL, `error[E0599]: no method named 'spaces' found for struct 'KaitenClient'`

- [ ] **Step 4: Добавить модель Space и фасад Spaces**

В конец `crates/kaiten-client/src/models.rs` добавить:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Space {
    pub id: u64,
    pub uid: String,
    pub title: String,
    #[serde(default)]
    pub archived: Option<bool>,
}
```

`crates/kaiten-client/src/api/mod.rs` (полностью, новое состояние):

```rust
pub mod spaces;
pub mod users;
```

`crates/kaiten-client/src/api/spaces.rs` (полностью):

```rust
use crate::client::KaitenClient;
use crate::error::Result;
use crate::models::Space;

/// Spaces resource facade. Construct via [`KaitenClient::spaces`].
pub struct Spaces<'a> {
    pub(crate) client: &'a KaitenClient,
}

impl Spaces<'_> {
    /// GET /spaces
    pub async fn list(&self) -> Result<Vec<Space>> {
        self.client
            .request(reqwest::Method::GET, "/spaces", None, None)
            .await
    }
}
```

В `impl KaitenClient` (`crates/kaiten-client/src/client.rs`) добавить:

```rust
    /// Spaces resource facade.
    pub fn spaces(&self) -> crate::api::spaces::Spaces<'_> {
        crate::api::spaces::Spaces { client: self }
    }
```

- [ ] **Step 5: Убедиться, что тест проходит**

Run: `cargo test -p kaiten-client --test spaces_test`
Expected: PASS, `test result: ok. 1 passed`

- [ ] **Step 6: Commit**

```
git add crates/kaiten-client/src/models.rs crates/kaiten-client/src/api/mod.rs crates/kaiten-client/src/api/spaces.rs crates/kaiten-client/src/client.rs crates/kaiten-client/tests/spaces_test.rs crates/kaiten-client/tests/fixtures/spaces_list.json
git commit -m "feat(client): spaces api"
```

---

### Task 5: Column/Lane/Board models + boards().list(space_id)/get(board_id)

**Files:**
- Create: crates/kaiten-client/src/api/boards.rs
- Create: crates/kaiten-client/tests/fixtures/board_get.json
- Create: crates/kaiten-client/tests/fixtures/boards_list.json
- Modify: crates/kaiten-client/src/models.rs:добавить Column, Lane, Board
- Modify: crates/kaiten-client/src/api/mod.rs:добавить `pub mod boards;`
- Modify: crates/kaiten-client/src/client.rs:добавить метод `boards()`
- Test: crates/kaiten-client/tests/boards_test.rs

**Interfaces:**
- Consumes: `KaitenClient::request` (Task 2)
- Produces: `models::{Column, Lane, Board}`; `api::boards::Boards<'a>` с `list(space_id: u64) -> Result<Vec<Board>>`, `get(board_id: u64) -> Result<Board>`; `KaitenClient::boards(&self) -> Boards<'_>`

Важно из фактов API: в `GET /boards/{id}` у колонок поле `type` — int (1/2/3), у lane есть лишний int-поле `condition`; вложенный `board` внутри карточки и элементы `GET /spaces/{id}/boards` НЕ содержат `columns`/`lanes` — поэтому у `Board` эти поля `Vec<_>` c `#[serde(default)]`, а не `Option`.

- [ ] **Step 1: Создать фикстуры board_get.json и boards_list.json**

`board_get.json` урезан из `fixtures/board_get.json` (все поля моделей + лишние `uid`, `email_key`, `col_count`, `rules`, `condition` для толерантности). `boards_list.json` урезан из вложенных boards в `fixtures/spaces_list.json` — форма без `columns`/`lanes`.

`crates/kaiten-client/tests/fixtures/board_get.json`:

```json
{
  "id": 1826109,
  "title": "test-board",
  "default_card_type_id": 1,
  "created": "2026-07-09T15:17:57.631Z",
  "updated": "2026-07-09T15:17:57.631Z",
  "uid": "06a433b5-3626-48d4-865b-398312b08c3c",
  "email_key": "82d0383fccb73049",
  "move_parents_to_done": true,
  "columns": [
    {
      "id": 6308511,
      "uid": "e8ba11a4-0832-49d6-b207-738582b09fde",
      "title": "To Do",
      "sort_order": 1,
      "col_count": 1,
      "type": 1,
      "board_id": 1826109,
      "rules": 0,
      "pause_sla": false
    }
  ],
  "lanes": [
    {
      "id": 2293584,
      "uid": "23343a06-a444-41c7-9a5b-d804fb31dfdc",
      "title": "Default Lane",
      "sort_order": 1,
      "board_id": 1826109,
      "condition": 1
    }
  ]
}
```

`crates/kaiten-client/tests/fixtures/boards_list.json`:

```json
[
  {
    "id": 1826105,
    "title": "Задачи",
    "default_card_type_id": 1,
    "created": "2026-07-09T15:13:28.677Z",
    "updated": "2026-07-09T15:13:28.677Z",
    "uid": "cffde9fe-f694-49fd-af65-74cb96817c64",
    "space_id": 810669,
    "sort_order": 657.8751055564335,
    "type": 1
  },
  {
    "id": 1826106,
    "title": "Полезные заметки",
    "default_card_type_id": 1,
    "uid": "91ff2974-1746-46a9-a5a4-2c3d67d36578",
    "space_id": 810669,
    "sort_order": 812.8977723550372,
    "type": 1
  }
]
```

- [ ] **Step 2: Написать failing test**

`crates/kaiten-client/tests/boards_test.rs` (полностью):

```rust
use kaiten_client::KaitenClient;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const BOARD_GET: &str = include_str!("fixtures/board_get.json");
const BOARDS_LIST: &str = include_str!("fixtures/boards_list.json");

#[tokio::test]
async fn get_parses_board_with_columns_and_lanes() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/boards/1826109"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(BOARD_GET, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let board = client.boards().get(1826109).await.unwrap();

    assert_eq!(board.id, 1826109);
    assert_eq!(board.title, "test-board");
    assert_eq!(board.default_card_type_id, Some(1));
    assert_eq!(board.columns.len(), 1);
    assert_eq!(board.columns[0].title, "To Do");
    // JSON-поле "type" (int) маппится в column_type
    assert_eq!(board.columns[0].column_type, Some(1));
    assert_eq!(board.columns[0].board_id, Some(1826109));
    assert_eq!(board.lanes.len(), 1);
    assert_eq!(board.lanes[0].title, "Default Lane");
}

#[tokio::test]
async fn list_parses_boards_without_columns_and_lanes() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/spaces/810669/boards"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(BOARDS_LIST, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let boards = client.boards().list(810669).await.unwrap();

    assert_eq!(boards.len(), 2);
    assert_eq!(boards[0].title, "Задачи");
    // ответ без ключей columns/lanes → #[serde(default)] даёт пустые векторы
    assert!(boards[0].columns.is_empty());
    assert!(boards[0].lanes.is_empty());
}
```

- [ ] **Step 3: Убедиться, что тест падает**

Run: `cargo test -p kaiten-client --test boards_test`
Expected: FAIL, `error[E0599]: no method named 'boards' found for struct 'KaitenClient'`

- [ ] **Step 4: Добавить модели Column/Lane/Board и фасад Boards**

В конец `crates/kaiten-client/src/models.rs` добавить:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Column {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub board_id: Option<u64>,
    /// 1 = queued, 2 = in progress, 3 = done
    #[serde(rename = "type", default)]
    pub column_type: Option<u8>,
    #[serde(default)]
    pub sort_order: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Lane {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub board_id: Option<u64>,
    #[serde(default)]
    pub sort_order: Option<f64>,
}

/// A nested `board` inside a card has no `columns`/`lanes` keys,
/// so both default to empty vectors.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Board {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub columns: Vec<Column>,
    #[serde(default)]
    pub lanes: Vec<Lane>,
    #[serde(default)]
    pub default_card_type_id: Option<u64>,
}
```

`crates/kaiten-client/src/api/mod.rs` (полностью, новое состояние):

```rust
pub mod boards;
pub mod spaces;
pub mod users;
```

`crates/kaiten-client/src/api/boards.rs` (полностью):

```rust
use crate::client::KaitenClient;
use crate::error::Result;
use crate::models::Board;

/// Boards resource facade. Construct via [`KaitenClient::boards`].
pub struct Boards<'a> {
    pub(crate) client: &'a KaitenClient,
}

impl Boards<'_> {
    /// GET /spaces/{space_id}/boards
    pub async fn list(&self, space_id: u64) -> Result<Vec<Board>> {
        self.client
            .request(
                reqwest::Method::GET,
                &format!("/spaces/{space_id}/boards"),
                None,
                None,
            )
            .await
    }

    /// GET /boards/{board_id}
    pub async fn get(&self, board_id: u64) -> Result<Board> {
        self.client
            .request(
                reqwest::Method::GET,
                &format!("/boards/{board_id}"),
                None,
                None,
            )
            .await
    }
}
```

В `impl KaitenClient` (`crates/kaiten-client/src/client.rs`) добавить:

```rust
    /// Boards resource facade.
    pub fn boards(&self) -> crate::api::boards::Boards<'_> {
        crate::api::boards::Boards { client: self }
    }
```

- [ ] **Step 5: Убедиться, что тест проходит**

Run: `cargo test -p kaiten-client --test boards_test`
Expected: PASS, `test result: ok. 2 passed`

- [ ] **Step 6: Commit**

```
git add crates/kaiten-client/src/models.rs crates/kaiten-client/src/api/mod.rs crates/kaiten-client/src/api/boards.rs crates/kaiten-client/src/client.rs crates/kaiten-client/tests/boards_test.rs crates/kaiten-client/tests/fixtures/board_get.json crates/kaiten-client/tests/fixtures/boards_list.json
git commit -m "feat(client): boards api with columns and lanes"
```

---

### Task 6: Card/CardType/CardTag/CardMember/Checklist/ChecklistItem models + CardFilter + cards().list()/get()

**Files:**
- Create: crates/kaiten-client/src/api/cards.rs
- Create: crates/kaiten-client/tests/fixtures/cards_list.json
- Create: crates/kaiten-client/tests/fixtures/card_get_full.json
- Modify: crates/kaiten-client/src/models.rs:добавить CardType, CardTag, CardMember, ChecklistItem, Checklist, Card
- Modify: crates/kaiten-client/src/api/mod.rs:добавить `pub mod cards;`
- Modify: crates/kaiten-client/src/client.rs:добавить метод `cards()`
- Modify: crates/kaiten-client/src/lib.rs:добавить `pub use api::cards::CardFilter;`
- Test: crates/kaiten-client/tests/cards_test.rs

**Interfaces:**
- Consumes: `KaitenClient::request` (Task 2); `models::{User, Board, Column, Lane}` (Tasks 3, 5)
- Produces:
  - `models::{Card, CardType, CardTag, CardMember, Checklist, ChecklistItem}`
  - `api::cards::CardFilter` c `to_query(&self) -> Vec<(String, String)>` (реэкспорт `kaiten_client::CardFilter`)
  - `api::cards::Cards<'a>` с `list(filter: &CardFilter) -> Result<Vec<Card>>`, `get(card_id: u64) -> Result<Card>`
  - `KaitenClient::cards(&self) -> Cards<'_>`

Факты API: `GET /cards` возвращает карточки БЕЗ `description`/`members`/`checklists` (но с вложенными `column`/`lane`/`board`/`type`); `GET /cards/{id}` — полная карточка. Тест обязан проверить, что обе формы парсятся одной моделью.

- [ ] **Step 1: Создать фикстуру cards_list.json**

Урезано из `fixtures/cards_list.json`: list-карточка — ключей `description`/`members`/`checklists`/`tags` НЕТ вовсе; вложенный `board` без `columns`/`lanes`; лишние поля `uid`, `sort_order`, `version`, `blocked`, `source`, `tag_ids`, `description_filled` для толерантности.

`crates/kaiten-client/tests/fixtures/cards_list.json`:

```json
[
  {
    "id": 67089469,
    "uid": "c78e313c-ab37-4456-9eb0-904681c4e309",
    "created": "2026-07-09T15:17:59.905Z",
    "updated": "2026-07-09T15:17:59.905Z",
    "archived": false,
    "title": "test card from cli",
    "asap": false,
    "due_date": null,
    "state": 1,
    "condition": 1,
    "board_id": 1826109,
    "column_id": 6308511,
    "lane_id": 2293584,
    "owner_id": 1068514,
    "type_id": 1,
    "comments_total": 0,
    "properties": null,
    "sort_order": 1.0689198905237203,
    "version": 1,
    "blocked": false,
    "source": "api",
    "tag_ids": null,
    "description_filled": false,
    "board": {
      "id": 1826109,
      "uid": "06a433b5-3626-48d4-865b-398312b08c3c",
      "title": "test-board"
    },
    "column": {
      "id": 6308511,
      "title": "To Do",
      "sort_order": 1,
      "col_count": 1,
      "type": 1,
      "board_id": 1826109
    },
    "lane": {
      "id": 2293584,
      "title": "Default Lane",
      "sort_order": 1,
      "board_id": 1826109,
      "condition": 1
    },
    "type": {
      "id": 1,
      "name": "Card",
      "color": 1,
      "letter": "C",
      "archived": false
    },
    "owner": {
      "id": 1068514,
      "uid": "0abd61ea-9dc5-40eb-b0a9-0d452ba1d8a6",
      "full_name": "dxmuser",
      "email": "user@example.com",
      "username": "dxmuser",
      "activated": true
    }
  }
]
```

- [ ] **Step 2: Создать фикстуру card_get_full.json**

Урезано из `fixtures/card_get_full.json` (полная карточка: description, members, checklists c items, tags); email заменён на `user@example.com`; лишние поля `uid`, `version`, `goals_total`, `goals_done`, `source`, `description_filled`, `comment_last_added_at`, у вложенных объектов — `card_id`, `checker_id`, `checked_at`, `deleted` и т.п.

`crates/kaiten-client/tests/fixtures/card_get_full.json`:

```json
{
  "id": 67089469,
  "uid": "c78e313c-ab37-4456-9eb0-904681c4e309",
  "created": "2026-07-09T15:17:59.905Z",
  "updated": "2026-07-09T15:18:07.303Z",
  "archived": false,
  "title": "test card from cli",
  "asap": true,
  "due_date": null,
  "description": "test **description**",
  "state": 1,
  "condition": 1,
  "board_id": 1826109,
  "column_id": 6308511,
  "lane_id": 2293584,
  "owner_id": 1068514,
  "type_id": 1,
  "version": 3,
  "comments_total": 1,
  "comment_last_added_at": "2026-07-09T15:18:03.341Z",
  "properties": null,
  "goals_total": 1,
  "goals_done": 1,
  "description_filled": true,
  "source": "api",
  "lane": {
    "id": 2293584,
    "title": "Default Lane",
    "sort_order": 1,
    "board_id": 1826109,
    "condition": 1
  },
  "board": {
    "id": 1826109,
    "uid": "06a433b5-3626-48d4-865b-398312b08c3c",
    "title": "test-board"
  },
  "column": {
    "id": 6308511,
    "title": "To Do",
    "sort_order": 1,
    "col_count": 1,
    "type": 1,
    "board_id": 1826109
  },
  "type": {
    "id": 1,
    "name": "Card",
    "color": 1,
    "letter": "C",
    "archived": false,
    "suggest_fields": true
  },
  "checklists": [
    {
      "id": 11747430,
      "name": "todo",
      "sort_order": 1.1522949931390465,
      "uid": "19d5b8ab-1baf-4537-8d42-5578949e75dd",
      "card_id": 67089469,
      "policy_id": null,
      "items": [
        {
          "id": 65658564,
          "text": "first item",
          "checked": true,
          "sort_order": 1.7468834610088972,
          "checklist_id": 11747430,
          "checker_id": 1068514,
          "checked_at": "2026-07-09T15:18:05.989Z",
          "deleted": false
        }
      ]
    }
  ],
  "members": [
    {
      "id": 1068514,
      "uid": "0abd61ea-9dc5-40eb-b0a9-0d452ba1d8a6",
      "full_name": "dxmuser",
      "email": "user@example.com",
      "username": "dxmuser",
      "activated": true,
      "card_id": 67089469,
      "user_id": 1068514,
      "type": 1
    }
  ],
  "tags": [
    {
      "id": 1110772,
      "name": "cli-test",
      "color": 15,
      "created": "2026-07-09T15:18:07.303Z",
      "card_id": 67089469,
      "tag_id": 1110772
    }
  ],
  "owner": {
    "id": 1068514,
    "uid": "0abd61ea-9dc5-40eb-b0a9-0d452ba1d8a6",
    "full_name": "dxmuser",
    "email": "user@example.com",
    "username": "dxmuser",
    "activated": true
  }
}
```

- [ ] **Step 3: Написать failing test**

`crates/kaiten-client/tests/cards_test.rs` (полностью):

```rust
use kaiten_client::{CardFilter, KaitenClient};
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const CARDS_LIST: &str = include_str!("fixtures/cards_list.json");
const CARD_GET_FULL: &str = include_str!("fixtures/card_get_full.json");

#[test]
fn to_query_skips_none_and_joins_member_ids() {
    let filter = CardFilter {
        space_id: Some(810671),
        query: Some("bug".to_string()),
        member_ids: vec![1, 2],
        archived: Some(false),
        ..Default::default()
    };
    let q = filter.to_query();
    assert_eq!(
        q,
        vec![
            ("space_id".to_string(), "810671".to_string()),
            ("query".to_string(), "bug".to_string()),
            ("member_ids".to_string(), "1,2".to_string()),
            ("archived".to_string(), "false".to_string()),
        ]
    );
}

#[test]
fn to_query_is_empty_for_default_filter() {
    assert!(CardFilter::default().to_query().is_empty());
}

#[tokio::test]
async fn list_sends_filter_query_params_and_parses_list_card() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cards"))
        .and(header("Authorization", "Bearer test-token"))
        .and(query_param("board_id", "1826109"))
        .and(query_param("member_ids", "1068514,42"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARDS_LIST, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let filter = CardFilter {
        board_id: Some(1826109),
        member_ids: vec![1068514, 42],
        limit: Some(50),
        ..Default::default()
    };
    let cards = client.cards().list(&filter).await.unwrap();

    assert_eq!(cards.len(), 1);
    let card = &cards[0];
    assert_eq!(card.id, 67089469);
    assert_eq!(card.title, "test card from cli");
    assert_eq!(card.state, Some(1));
    assert_eq!(card.condition, Some(1));
    // list-карточка приходит БЕЗ description/members/checklists/tags
    assert!(card.description.is_none());
    assert!(card.members.is_empty());
    assert!(card.checklists.is_empty());
    assert!(card.tags.is_empty());
    // вложенный board без columns/lanes → пустые векторы
    let board = card.board.as_ref().unwrap();
    assert_eq!(board.id, 1826109);
    assert!(board.columns.is_empty());
    assert_eq!(card.column.as_ref().unwrap().column_type, Some(1));
    assert_eq!(card.card_type.as_ref().unwrap().name, "Card");
    assert_eq!(card.owner.as_ref().unwrap().email.as_deref(), Some("user@example.com"));
}

#[tokio::test]
async fn get_parses_full_card() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cards/67089469"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARD_GET_FULL, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let card = client.cards().get(67089469).await.unwrap();

    assert_eq!(card.id, 67089469);
    assert_eq!(card.description.as_deref(), Some("test **description**"));
    assert_eq!(card.asap, Some(true));
    assert_eq!(card.comments_total, Some(1));
    assert_eq!(card.members.len(), 1);
    assert_eq!(card.members[0].user_id, Some(1068514));
    assert_eq!(card.members[0].member_type, Some(1));
    assert_eq!(card.tags.len(), 1);
    assert_eq!(card.tags[0].name, "cli-test");
    assert_eq!(card.tags[0].tag_id, Some(1110772));
    assert_eq!(card.checklists.len(), 1);
    assert_eq!(card.checklists[0].name, "todo");
    assert_eq!(card.checklists[0].items.len(), 1);
    assert_eq!(card.checklists[0].items[0].text, "first item");
    assert_eq!(card.checklists[0].items[0].checked, Some(true));
}
```

- [ ] **Step 4: Убедиться, что тест падает**

Run: `cargo test -p kaiten-client --test cards_test`
Expected: FAIL, `error[E0432]: unresolved import 'kaiten_client::CardFilter'`

- [ ] **Step 5: Добавить модели карточки в models.rs**

В конец `crates/kaiten-client/src/models.rs` добавить:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CardType {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub letter: Option<String>,
    #[serde(default)]
    pub color: Option<i64>,
    #[serde(default)]
    pub archived: Option<bool>,
}

/// A tag inside `card.tags`: `id` is the link id, `tag_id` is the company tag id.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CardTag {
    pub id: u64,
    #[serde(default)]
    pub tag_id: Option<u64>,
    pub name: String,
    #[serde(default)]
    pub color: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CardMember {
    /// User id.
    pub id: u64,
    #[serde(default)]
    pub user_id: Option<u64>,
    #[serde(default)]
    pub full_name: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    /// 2 = responsible
    #[serde(rename = "type", default)]
    pub member_type: Option<u8>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChecklistItem {
    pub id: u64,
    pub text: String,
    #[serde(default)]
    pub checked: Option<bool>,
    #[serde(default)]
    pub sort_order: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Checklist {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub items: Vec<ChecklistItem>,
    #[serde(default)]
    pub sort_order: Option<f64>,
}

/// GET /cards/{id} returns the full card; GET /cards returns cards
/// without `description`/`members`/`checklists` — the same model parses both.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Card {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub asap: Option<bool>,
    #[serde(default)]
    pub archived: Option<bool>,
    /// 1 = live, 2 = archived
    #[serde(default)]
    pub condition: Option<u8>,
    /// 1 = queued, 2 = in progress, 3 = done
    #[serde(default)]
    pub state: Option<u8>,
    #[serde(default)]
    pub board_id: Option<u64>,
    #[serde(default)]
    pub column_id: Option<u64>,
    #[serde(default)]
    pub lane_id: Option<u64>,
    #[serde(default)]
    pub type_id: Option<u64>,
    #[serde(default)]
    pub owner_id: Option<u64>,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default)]
    pub updated: Option<String>,
    #[serde(default)]
    pub due_date: Option<String>,
    #[serde(default)]
    pub comments_total: Option<u32>,
    /// Nested board has no `columns`/`lanes` keys → they default to empty.
    #[serde(default)]
    pub board: Option<Board>,
    #[serde(default)]
    pub column: Option<Column>,
    #[serde(default)]
    pub lane: Option<Lane>,
    #[serde(rename = "type", default)]
    pub card_type: Option<CardType>,
    #[serde(default)]
    pub owner: Option<User>,
    #[serde(default)]
    pub members: Vec<CardMember>,
    #[serde(default)]
    pub tags: Vec<CardTag>,
    #[serde(default)]
    pub checklists: Vec<Checklist>,
    /// Custom properties, read-only.
    #[serde(default)]
    pub properties: Option<serde_json::Value>,
}
```

- [ ] **Step 6: Создать api/cards.rs с CardFilter и фасадом Cards**

`crates/kaiten-client/src/api/cards.rs` (полностью):

```rust
use crate::client::KaitenClient;
use crate::error::Result;
use crate::models::Card;

/// Filter for GET /cards. `None`/empty fields are omitted from the query.
#[derive(Debug, Default, Clone)]
pub struct CardFilter {
    pub space_id: Option<u64>,
    pub board_id: Option<u64>,
    pub column_id: Option<u64>,
    pub lane_id: Option<u64>,
    pub query: Option<String>,
    /// Serialized as a comma-separated list: "1,2,3".
    pub member_ids: Vec<u64>,
    pub owner_id: Option<u64>,
    /// Tag name.
    pub tag: Option<String>,
    pub type_id: Option<u64>,
    pub archived: Option<bool>,
    pub condition: Option<u8>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl CardFilter {
    pub fn to_query(&self) -> Vec<(String, String)> {
        fn push<T: ToString>(q: &mut Vec<(String, String)>, key: &str, value: &Option<T>) {
            if let Some(v) = value {
                q.push((key.to_string(), v.to_string()));
            }
        }

        let mut q: Vec<(String, String)> = Vec::new();
        push(&mut q, "space_id", &self.space_id);
        push(&mut q, "board_id", &self.board_id);
        push(&mut q, "column_id", &self.column_id);
        push(&mut q, "lane_id", &self.lane_id);
        push(&mut q, "query", &self.query);
        if !self.member_ids.is_empty() {
            q.push((
                "member_ids".to_string(),
                self.member_ids
                    .iter()
                    .map(u64::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            ));
        }
        push(&mut q, "owner_id", &self.owner_id);
        push(&mut q, "tag", &self.tag);
        push(&mut q, "type_id", &self.type_id);
        push(&mut q, "archived", &self.archived);
        push(&mut q, "condition", &self.condition);
        push(&mut q, "limit", &self.limit);
        push(&mut q, "offset", &self.offset);
        q
    }
}

/// Cards resource facade. Construct via [`KaitenClient::cards`].
pub struct Cards<'a> {
    pub(crate) client: &'a KaitenClient,
}

impl Cards<'_> {
    /// GET /cards
    pub async fn list(&self, filter: &CardFilter) -> Result<Vec<Card>> {
        let q = filter.to_query();
        let query = if q.is_empty() { None } else { Some(q) };
        self.client
            .request(reqwest::Method::GET, "/cards", query, None)
            .await
    }

    /// GET /cards/{id}
    pub async fn get(&self, card_id: u64) -> Result<Card> {
        self.client
            .request(
                reqwest::Method::GET,
                &format!("/cards/{card_id}"),
                None,
                None,
            )
            .await
    }
}
```

`crates/kaiten-client/src/api/mod.rs` (полностью, новое состояние):

```rust
pub mod boards;
pub mod cards;
pub mod spaces;
pub mod users;
```

В `impl KaitenClient` (`crates/kaiten-client/src/client.rs`) добавить:

```rust
    /// Cards resource facade.
    pub fn cards(&self) -> crate::api::cards::Cards<'_> {
        crate::api::cards::Cards { client: self }
    }
```

В `crates/kaiten-client/src/lib.rs` добавить (после `pub use models::*;`):

```rust
pub use api::cards::CardFilter;
```

- [ ] **Step 7: Убедиться, что тест проходит**

Run: `cargo test -p kaiten-client --test cards_test`
Expected: PASS, `test result: ok. 4 passed`

- [ ] **Step 8: Commit**

```
git add crates/kaiten-client/src/models.rs crates/kaiten-client/src/api/mod.rs crates/kaiten-client/src/api/cards.rs crates/kaiten-client/src/client.rs crates/kaiten-client/src/lib.rs crates/kaiten-client/tests/cards_test.rs crates/kaiten-client/tests/fixtures/cards_list.json crates/kaiten-client/tests/fixtures/card_get_full.json
git commit -m "feat(client): cards list/get with filter"
```

---

### Task 7: CreateCard/UpdateCard + cards().create()/update()

**Files:**
- Create: crates/kaiten-client/tests/fixtures/card_create.json
- Create: crates/kaiten-client/tests/fixtures/card_update.json
- Modify: crates/kaiten-client/src/api/cards.rs:добавить CreateCard, UpdateCard, методы create()/update()
- Modify: crates/kaiten-client/src/lib.rs:реэкспорт CreateCard/UpdateCard
- Modify: crates/kaiten-client/tests/cards_test.rs:добавить тесты create/update
- Test: crates/kaiten-client/tests/cards_test.rs

**Interfaces:**
- Consumes: `Cards<'a>`, `models::Card` (Task 6); `KaitenError::Decode` (Task 1/2)
- Produces:
  - `api::cards::CreateCard`, `api::cards::UpdateCard` (реэкспорты `kaiten_client::{CreateCard, UpdateCard}`)
  - `Cards::create(req: &CreateCard) -> Result<Card>` (POST /cards)
  - `Cards::update(card_id: u64, req: &UpdateCard) -> Result<Card>` (PATCH /cards/{id}; move = update с column_id/lane_id/board_id; archive = update с condition=2)

Ключевая проверка: тело POST содержит `board_id` + `title` и НЕ содержит ключей незаполненных Option-полей (`skip_serializing_if = "Option::is_none"`). Отсутствие ключа проверяем кастомным wiremock-матчером `BodyLacksKey`, наличие — `body_partial_json`.

- [ ] **Step 1: Создать фикстуры card_create.json и card_update.json**

Урезано из `fixtures/card_create.json` (ответ POST /cards: есть `"description": null`, пустые `checklists`, вложенные `type`/`owner`) и `fixtures/card_update.json` (ответ PATCH: обновлённые `asap`/`description`, лишние `version`, `fts_version`, `description_filled`).

`crates/kaiten-client/tests/fixtures/card_create.json`:

```json
{
  "id": 67089469,
  "uid": "c78e313c-ab37-4456-9eb0-904681c4e309",
  "title": "test card from cli",
  "created": "2026-07-09T15:17:59.905Z",
  "updated": "2026-07-09T15:17:59.905Z",
  "archived": false,
  "asap": false,
  "due_date": null,
  "state": 1,
  "condition": 1,
  "board_id": 1826109,
  "column_id": 6308511,
  "lane_id": 2293584,
  "owner_id": 1068514,
  "type_id": 1,
  "comments_total": 0,
  "description": null,
  "properties": null,
  "version": 1,
  "sort_order": 1.0689198905237203,
  "source": "api",
  "checklists": [],
  "external_links": [],
  "type": {
    "id": 1,
    "name": "Card",
    "color": 1,
    "letter": "C",
    "archived": false
  },
  "owner": {
    "id": 1068514,
    "uid": "0abd61ea-9dc5-40eb-b0a9-0d452ba1d8a6",
    "full_name": "dxmuser",
    "email": "user@example.com",
    "username": "dxmuser",
    "activated": true
  }
}
```

`crates/kaiten-client/tests/fixtures/card_update.json`:

```json
{
  "id": 67089469,
  "uid": "c78e313c-ab37-4456-9eb0-904681c4e309",
  "title": "test card from cli",
  "created": "2026-07-09T15:17:59.905Z",
  "updated": "2026-07-09T15:18:02.423Z",
  "archived": false,
  "asap": true,
  "due_date": null,
  "description": "test **description**",
  "state": 1,
  "condition": 1,
  "board_id": 1826109,
  "column_id": 6308511,
  "lane_id": 2293584,
  "owner_id": 1068514,
  "type_id": 1,
  "comments_total": 0,
  "properties": null,
  "version": 2,
  "description_filled": true,
  "fts_version": "829174643",
  "source": "api"
}
```

- [ ] **Step 2: Дополнить cards_test.rs failing-тестами**

В `crates/kaiten-client/tests/cards_test.rs` заменить первые две строки импортов:

```rust
use kaiten_client::{CardFilter, KaitenClient};
use wiremock::matchers::{header, method, path, query_param};
```

на:

```rust
use kaiten_client::{CardFilter, CreateCard, KaitenClient, UpdateCard};
use wiremock::matchers::{body_partial_json, header, method, path, query_param};
```

и добавить в конец файла:

```rust
const CARD_CREATE: &str = include_str!("fixtures/card_create.json");
const CARD_UPDATE: &str = include_str!("fixtures/card_update.json");

/// Matches only if the JSON body is an object WITHOUT the given key.
struct BodyLacksKey(&'static str);

impl wiremock::Match for BodyLacksKey {
    fn matches(&self, request: &wiremock::Request) -> bool {
        serde_json::from_slice::<serde_json::Value>(&request.body)
            .ok()
            .and_then(|v| v.as_object().map(|o| !o.contains_key(self.0)))
            .unwrap_or(false)
    }
}

#[tokio::test]
async fn create_sends_board_id_and_title_and_omits_none_fields() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cards"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_partial_json(serde_json::json!({
            "board_id": 1826109,
            "title": "test card from cli"
        })))
        .and(BodyLacksKey("description"))
        .and(BodyLacksKey("column_id"))
        .and(BodyLacksKey("lane_id"))
        .and(BodyLacksKey("type_id"))
        .and(BodyLacksKey("asap"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARD_CREATE, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let req = CreateCard {
        board_id: 1826109,
        title: "test card from cli".to_string(),
        ..Default::default()
    };
    let card = client.cards().create(&req).await.unwrap();

    assert_eq!(card.id, 67089469);
    assert_eq!(card.board_id, Some(1826109));
    assert_eq!(card.column_id, Some(6308511));
    // "description": null в ответе → None
    assert!(card.description.is_none());
    assert!(card.checklists.is_empty());
}

#[tokio::test]
async fn update_with_column_id_is_move_and_omits_other_fields() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/cards/67089469"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_partial_json(serde_json::json!({ "column_id": 6308512 })))
        .and(BodyLacksKey("title"))
        .and(BodyLacksKey("description"))
        .and(BodyLacksKey("board_id"))
        .and(BodyLacksKey("condition"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARD_UPDATE, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let req = UpdateCard {
        column_id: Some(6308512),
        ..Default::default()
    };
    let card = client.cards().update(67089469, &req).await.unwrap();

    assert_eq!(card.id, 67089469);
    assert_eq!(card.asap, Some(true));
    assert_eq!(card.description.as_deref(), Some("test **description**"));
}
```

- [ ] **Step 3: Убедиться, что тест падает**

Run: `cargo test -p kaiten-client --test cards_test`
Expected: FAIL, `error[E0432]: unresolved import 'kaiten_client::CreateCard'`

- [ ] **Step 4: Добавить CreateCard/UpdateCard и методы create()/update()**

В `crates/kaiten-client/src/api/cards.rs` заменить строку импорта ошибок:

```rust
use crate::error::Result;
```

на:

```rust
use crate::error::{KaitenError, Result};
```

и добавить после `impl CardFilter { ... }` (перед `pub struct Cards`):

```rust
/// Body for POST /cards. All fields except `board_id`/`title` are optional
/// and omitted from JSON when `None`.
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct CreateCard {
    pub board_id: u64,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lane_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asap: Option<bool>,
}

/// Body for PATCH /cards/{id}. Move = update with `column_id`/`lane_id`/`board_id`;
/// archive = update with `condition = 2`. `None` fields are omitted from JSON.
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct UpdateCard {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asap: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lane_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub board_id: Option<u64>,
    /// 2 = archive the card.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<u8>,
}
```

и добавить внутрь `impl Cards<'_>` (после `get`):

```rust
    /// POST /cards
    pub async fn create(&self, req: &CreateCard) -> Result<Card> {
        let body = serde_json::to_value(req).map_err(|e| KaitenError::Decode {
            path: "CreateCard".to_string(),
            source: e,
        })?;
        self.client
            .request(reqwest::Method::POST, "/cards", None, Some(body))
            .await
    }

    /// PATCH /cards/{id}
    pub async fn update(&self, card_id: u64, req: &UpdateCard) -> Result<Card> {
        let body = serde_json::to_value(req).map_err(|e| KaitenError::Decode {
            path: "UpdateCard".to_string(),
            source: e,
        })?;
        self.client
            .request(
                reqwest::Method::PATCH,
                &format!("/cards/{card_id}"),
                None,
                Some(body),
            )
            .await
    }
```

В `crates/kaiten-client/src/lib.rs` заменить строку:

```rust
pub use api::cards::CardFilter;
```

на:

```rust
pub use api::cards::{CardFilter, CreateCard, UpdateCard};
```

- [ ] **Step 5: Убедиться, что тест проходит**

Run: `cargo test -p kaiten-client --test cards_test`
Expected: PASS, `test result: ok. 6 passed`

- [ ] **Step 6: Commit**

```
git add crates/kaiten-client/src/api/cards.rs crates/kaiten-client/src/lib.rs crates/kaiten-client/tests/cards_test.rs crates/kaiten-client/tests/fixtures/card_create.json crates/kaiten-client/tests/fixtures/card_update.json
git commit -m "feat(client): card create and update"
```

---

### Task 8: comments().list()/add() + members().add()/remove()

**Files:**
- Create: crates/kaiten-client/src/api/comments.rs
- Create: crates/kaiten-client/src/api/members.rs
- Create: crates/kaiten-client/tests/fixtures/comments_list.json
- Create: crates/kaiten-client/tests/fixtures/comment_add.json
- Create: crates/kaiten-client/tests/fixtures/card_members_add.json
- Modify: crates/kaiten-client/src/models.rs:добавить struct Comment
- Modify: crates/kaiten-client/src/api/mod.rs:добавить `pub mod comments; pub mod members;`
- Modify: crates/kaiten-client/src/client.rs:добавить `comments()`, `members()` и `pub(crate) request_empty`
- Test: crates/kaiten-client/tests/comments_test.rs
- Test: crates/kaiten-client/tests/members_test.rs

**Interfaces:**
- Consumes: `KaitenClient::request` и внутренний `pub(crate) async fn send_with_retry(&self, method: reqwest::Method, path: &str, query: Option<Vec<(String, String)>>, body: Option<serde_json::Value>) -> Result<(u16, String)>` (оба — Task 2); `models::{User, CardMember}` (Tasks 3, 6); dev-dependencies `tokio`/`wiremock` в `crates/kaiten-client/Cargo.toml` (заведены в Task 1)
- Produces:
  - `models::Comment`
  - `api::comments::Comments<'a>` с `list(card_id: u64) -> Result<Vec<Comment>>`, `add(card_id: u64, text: &str) -> Result<Comment>`
  - `api::members::Members<'a>` с `add(card_id: u64, user_id: u64) -> Result<CardMember>`, `remove(card_id: u64, user_id: u64) -> Result<()>`
  - `KaitenClient::comments(&self) -> Comments<'_>`, `KaitenClient::members(&self) -> Members<'_>`
  - `pub(crate) async fn KaitenClient::request_empty(&self, method: reqwest::Method, path: &str) -> Result<()>` — для DELETE-эндпоинтов, у которых тело ответа пустое или игнорируется (используется также в Task 9)

Факт API: DELETE возвращает JSON или ПУСТОЕ тело — `remove*` обязаны игнорировать тело и возвращать `Ok(())`. Ядро `request<T>` требует JSON-тело, поэтому добавляем узкий помощник `request_empty` — тонкую обёртку над `send_with_retry` из Task 2: 429-ретрай и tracing (method/path/status/elapsed_ms, тела на trace-уровне) он получает автоматически. Тело успешного (2xx) ответа игнорируется; не-2xx мапится в `KaitenError::Api` по тем же правилам, что и в `request<T>`: `message` — из JSON-поля `message`, иначе само тело как строка, иначе (пустое тело) — canonical reason; `body` — сырое тело целиком.

- [ ] **Step 1: Создать фикстуры**

Урезано из `fixtures/comments_list.json`, `fixtures/comment_add.json` (без `author` — проверка `Option = None`) и `fixtures/card_members_add.json` (в ответе add НЕТ `user_id`, есть `"type": 1`); email заменён на `user@example.com`.

`crates/kaiten-client/tests/fixtures/comments_list.json`:

```json
[
  {
    "id": 85523991,
    "uid": "ef4cb581-d2fd-4db9-ae85-aa352e27436d",
    "created": "2026-07-09T15:18:03.341Z",
    "updated": "2026-07-09T15:18:03.341Z",
    "text": "test comment",
    "type": 1,
    "edited": false,
    "card_id": 67089469,
    "author_id": 1068514,
    "internal": false,
    "deleted": false,
    "author": {
      "id": 1068514,
      "uid": "0abd61ea-9dc5-40eb-b0a9-0d452ba1d8a6",
      "full_name": "dxmuser",
      "email": "user@example.com",
      "username": "dxmuser",
      "activated": true
    }
  }
]
```

`crates/kaiten-client/tests/fixtures/comment_add.json`:

```json
{
  "id": 85523991,
  "uid": "ef4cb581-d2fd-4db9-ae85-aa352e27436d",
  "created": "2026-07-09T15:18:03.341Z",
  "updated": "2026-07-09T15:18:03.341Z",
  "text": "test comment",
  "type": 1,
  "edited": false,
  "card_id": 67089469,
  "author_id": 1068514,
  "internal": false,
  "deleted": false,
  "meta": null
}
```

`crates/kaiten-client/tests/fixtures/card_members_add.json`:

```json
{
  "id": 1068514,
  "uid": "0abd61ea-9dc5-40eb-b0a9-0d452ba1d8a6",
  "full_name": "dxmuser",
  "email": "user@example.com",
  "username": "dxmuser",
  "activated": true,
  "lng": "ru",
  "timezone": "UTC",
  "virtual": false,
  "type": 1
}
```

- [ ] **Step 2: Написать failing tests**

`crates/kaiten-client/tests/comments_test.rs` (полностью):

```rust
use kaiten_client::KaitenClient;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const COMMENTS_LIST: &str = include_str!("fixtures/comments_list.json");
const COMMENT_ADD: &str = include_str!("fixtures/comment_add.json");

#[tokio::test]
async fn list_parses_comments_with_author() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cards/67089469/comments"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(COMMENTS_LIST, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let comments = client.comments().list(67089469).await.unwrap();

    assert_eq!(comments.len(), 1);
    assert_eq!(comments[0].id, 85523991);
    assert_eq!(comments[0].text, "test comment");
    assert_eq!(comments[0].edited, Some(false));
    assert_eq!(comments[0].author_id, Some(1068514));
    let author = comments[0].author.as_ref().unwrap();
    assert_eq!(author.email.as_deref(), Some("user@example.com"));
}

#[tokio::test]
async fn add_posts_text_body_and_parses_comment_without_author() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cards/67089469/comments"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_json(serde_json::json!({ "text": "test comment" })))
        .respond_with(ResponseTemplate::new(200).set_body_raw(COMMENT_ADD, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let comment = client.comments().add(67089469, "test comment").await.unwrap();

    assert_eq!(comment.id, 85523991);
    assert_eq!(comment.text, "test comment");
    assert!(comment.author.is_none());
}
```

`crates/kaiten-client/tests/members_test.rs` (полностью):

```rust
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
        .and(body_json(serde_json::json!({ "user_id": 1068514 })))
        .respond_with(ResponseTemplate::new(200).set_body_raw(MEMBER_ADD, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let member = client.members().add(67089469, 1068514).await.unwrap();

    assert_eq!(member.id, 1068514);
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
    client.members().remove(67089469, 1068514).await.unwrap();
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
    let err = client.members().remove(67089469, 999).await.unwrap_err();

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
```

- [ ] **Step 3: Убедиться, что тесты падают**

Run: `cargo test -p kaiten-client --test comments_test --test members_test`
Expected: FAIL, `error[E0599]: no method named 'comments' found for struct 'KaitenClient'` (и аналогично `members`)

- [ ] **Step 4: Добавить модель Comment и фасады**

В конец `crates/kaiten-client/src/models.rs` добавить:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Comment {
    pub id: u64,
    pub text: String,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default)]
    pub updated: Option<String>,
    #[serde(default)]
    pub edited: Option<bool>,
    #[serde(default)]
    pub author: Option<User>,
    #[serde(default)]
    pub author_id: Option<u64>,
}
```

`crates/kaiten-client/src/api/comments.rs` (полностью):

```rust
use crate::client::KaitenClient;
use crate::error::Result;
use crate::models::Comment;

/// Comments resource facade. Construct via [`KaitenClient::comments`].
pub struct Comments<'a> {
    pub(crate) client: &'a KaitenClient,
}

impl Comments<'_> {
    /// GET /cards/{id}/comments
    pub async fn list(&self, card_id: u64) -> Result<Vec<Comment>> {
        self.client
            .request(
                reqwest::Method::GET,
                &format!("/cards/{card_id}/comments"),
                None,
                None,
            )
            .await
    }

    /// POST /cards/{id}/comments
    pub async fn add(&self, card_id: u64, text: &str) -> Result<Comment> {
        self.client
            .request(
                reqwest::Method::POST,
                &format!("/cards/{card_id}/comments"),
                None,
                Some(serde_json::json!({ "text": text })),
            )
            .await
    }
}
```

`crates/kaiten-client/src/api/members.rs` (полностью):

```rust
use crate::client::KaitenClient;
use crate::error::Result;
use crate::models::CardMember;

/// Card members resource facade. Construct via [`KaitenClient::members`].
pub struct Members<'a> {
    pub(crate) client: &'a KaitenClient,
}

impl Members<'_> {
    /// POST /cards/{id}/members
    pub async fn add(&self, card_id: u64, user_id: u64) -> Result<CardMember> {
        self.client
            .request(
                reqwest::Method::POST,
                &format!("/cards/{card_id}/members"),
                None,
                Some(serde_json::json!({ "user_id": user_id })),
            )
            .await
    }

    /// DELETE /cards/{id}/members/{user_id}; the response body is ignored.
    pub async fn remove(&self, card_id: u64, user_id: u64) -> Result<()> {
        self.client
            .request_empty(
                reqwest::Method::DELETE,
                &format!("/cards/{card_id}/members/{user_id}"),
            )
            .await
    }
}
```

`crates/kaiten-client/src/api/mod.rs` (полностью, новое состояние):

```rust
pub mod boards;
pub mod cards;
pub mod comments;
pub mod members;
pub mod spaces;
pub mod users;
```

- [ ] **Step 5: Добавить request_empty и конструкторы фасадов в client.rs**

В `impl KaitenClient` (`crates/kaiten-client/src/client.rs`) добавить:

```rust
    /// Comments resource facade.
    pub fn comments(&self) -> crate::api::comments::Comments<'_> {
        crate::api::comments::Comments { client: self }
    }

    /// Card members resource facade.
    pub fn members(&self) -> crate::api::members::Members<'_> {
        crate::api::members::Members { client: self }
    }

    /// Perform a request whose response body may be empty and is ignored
    /// (Kaiten DELETE endpoints return JSON or an empty body).
    ///
    /// Thin wrapper over `send_with_retry`, so the 429 retry loop and the
    /// request/response tracing are shared with `request<T>`.
    pub(crate) async fn request_empty(
        &self,
        method: reqwest::Method,
        path: &str,
    ) -> crate::error::Result<()> {
        let (status, body) = self.send_with_retry(method, path, None, None).await?;
        if (200..300).contains(&status) {
            // 2xx: the body (JSON or empty) is ignored by design.
            return Ok(());
        }
        // Same mapping rules as in `request<T>`: message = the JSON "message"
        // field, else the raw body, else the canonical reason for an empty body.
        let message = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(str::to_owned))
            .unwrap_or_else(|| {
                if body.trim().is_empty() {
                    reqwest::StatusCode::from_u16(status)
                        .ok()
                        .and_then(|s| s.canonical_reason())
                        .unwrap_or("unknown error")
                        .to_string()
                } else {
                    body.clone()
                }
            });
        Err(crate::error::KaitenError::Api {
            status,
            message,
            body,
        })
    }
```

- [ ] **Step 6: Убедиться, что тесты проходят**

Run: `cargo test -p kaiten-client --test comments_test --test members_test`
Expected: PASS, `2 passed` (comments) и `4 passed` (members)

- [ ] **Step 7: Commit**

```
git add crates/kaiten-client/src/models.rs crates/kaiten-client/src/api/mod.rs crates/kaiten-client/src/api/comments.rs crates/kaiten-client/src/api/members.rs crates/kaiten-client/src/client.rs crates/kaiten-client/tests/comments_test.rs crates/kaiten-client/tests/members_test.rs crates/kaiten-client/tests/fixtures/comments_list.json crates/kaiten-client/tests/fixtures/comment_add.json crates/kaiten-client/tests/fixtures/card_members_add.json
git commit -m "feat(client): comments and members api"
```

---

### Task 9: checklists().add()/add_item()/set_item_checked() + tags().list()/add_to_card()/remove_from_card()/card_types()

**Files:**
- Create: crates/kaiten-client/src/api/checklists.rs
- Create: crates/kaiten-client/src/api/tags.rs
- Create: crates/kaiten-client/tests/fixtures/checklist_add.json
- Create: crates/kaiten-client/tests/fixtures/checklist_item_add.json
- Create: crates/kaiten-client/tests/fixtures/checklist_item_check.json
- Create: crates/kaiten-client/tests/fixtures/tags_list.json
- Create: crates/kaiten-client/tests/fixtures/card_tag_add.json
- Create: crates/kaiten-client/tests/fixtures/card_types.json
- Modify: crates/kaiten-client/src/models.rs:добавить struct Tag
- Modify: crates/kaiten-client/src/api/mod.rs:добавить `pub mod checklists; pub mod tags;`
- Modify: crates/kaiten-client/src/client.rs:добавить `checklists()`, `tags()`
- Test: crates/kaiten-client/tests/checklists_test.rs
- Test: crates/kaiten-client/tests/tags_test.rs

**Interfaces:**
- Consumes: `KaitenClient::request` (Task 2); `KaitenClient::request_empty` (Task 8); `models::{Checklist, ChecklistItem, CardType}` (Task 6)
- Produces:
  - `models::Tag`
  - `api::checklists::Checklists<'a>` с `add(card_id, name) -> Result<Checklist>`, `add_item(card_id, checklist_id, text) -> Result<ChecklistItem>`, `set_item_checked(card_id, checklist_id, item_id, checked) -> Result<ChecklistItem>`
  - `api::tags::Tags<'a>` с `list() -> Result<Vec<Tag>>`, `add_to_card(card_id, name) -> Result<Tag>`, `remove_from_card(card_id, tag_id) -> Result<()>`, `card_types() -> Result<Vec<CardType>>`
  - `KaitenClient::checklists(&self) -> Checklists<'_>`, `KaitenClient::tags(&self) -> Tags<'_>`

ВАЖНО (факт API): `GET /cards/{id}/checklists` НЕ СУЩЕСТВУЕТ (сервер отвечает 405) — метода чтения чеклистов у фасада НЕТ и быть не должно. Чтение — только через `cards().get()` → `Card.checklists`. `POST /cards/{id}/tags` принимает `{"name": "..."}` и создаёт тег компании, если его нет.

- [ ] **Step 1: Создать фикстуры чеклистов**

Урезано из `fixtures/checklist_add.json` (в ответе НЕТ ключа `items` → проверка `#[serde(default)]`), `fixtures/checklist_item_add.json`, `fixtures/checklist_item_check.json`.

`crates/kaiten-client/tests/fixtures/checklist_add.json`:

```json
{
  "id": 11747430,
  "name": "todo",
  "created": "2026-07-09T15:18:04.519Z",
  "updated": "2026-07-09T15:18:04.519Z",
  "uid": "19d5b8ab-1baf-4537-8d42-5578949e75dd",
  "card_id": 67089469,
  "checklist_id": 11747430,
  "sort_order": 1.1522949931390465,
  "policy_id": null,
  "deleted": false
}
```

`crates/kaiten-client/tests/fixtures/checklist_item_add.json`:

```json
{
  "id": 65658564,
  "text": "first item",
  "checked": false,
  "sort_order": 1.7468834610088972,
  "created": "2026-07-09T15:18:05.386Z",
  "updated": "2026-07-09T15:18:05.386Z",
  "checklist_id": 11747430,
  "checker_id": null,
  "checked_at": null,
  "deleted": false,
  "uid": "700601dd-9e37-4103-95d1-28edb55ff2df"
}
```

`crates/kaiten-client/tests/fixtures/checklist_item_check.json`:

```json
{
  "id": 65658564,
  "text": "first item",
  "checked": true,
  "sort_order": 1.7468834610088972,
  "created": "2026-07-09T15:18:05.386Z",
  "updated": "2026-07-09T15:18:05.995Z",
  "checklist_id": 11747430,
  "checker_id": 1068514,
  "checked_at": "2026-07-09T15:18:05.989Z",
  "deleted": false,
  "uid": "700601dd-9e37-4103-95d1-28edb55ff2df"
}
```

- [ ] **Step 2: Создать фикстуры тегов и типов карточек**

`tags_list.json` и `card_tag_add.json` урезаны из `fixtures/card_tag_add.json` (реальный тег компании `cli-test`); `card_types.json` урезан из `fixtures/card_types.json` (email заменён — вложенный `author` выброшен как нерелевантный, оставлены лишние скалярные поля).

`crates/kaiten-client/tests/fixtures/tags_list.json`:

```json
[
  {
    "id": 1110772,
    "name": "cli-test",
    "color": 15,
    "created": "2026-07-09T15:18:07.303Z",
    "updated": "2026-07-09T15:18:07.303Z",
    "company_id": 398610,
    "archived": false,
    "uid": "f9ba3ae1-6227-4036-82ec-412ba30556e7"
  }
]
```

`crates/kaiten-client/tests/fixtures/card_tag_add.json`:

```json
{
  "id": 1110772,
  "name": "cli-test",
  "color": 15,
  "created": "2026-07-09T15:18:07.303Z",
  "updated": "2026-07-09T15:18:07.303Z",
  "company_id": 398610,
  "archived": false,
  "uid": "f9ba3ae1-6227-4036-82ec-412ba30556e7",
  "fts_version": "829174686"
}
```

`crates/kaiten-client/tests/fixtures/card_types.json`:

```json
[
  {
    "id": 1,
    "name": "Card",
    "color": 1,
    "letter": "C",
    "archived": false,
    "company_id": null,
    "uid": "64792c05-0d0d-4b19-a3a6-d3c34b0a197c",
    "suggest_fields": true,
    "created": "2014-11-13T22:20:14.374Z"
  },
  {
    "id": 692718,
    "name": "Bug",
    "color": 3,
    "letter": "B",
    "archived": false,
    "company_id": 398610,
    "uid": "38fd521d-7ab8-472a-978e-f0313732c5f2",
    "suggest_fields": true,
    "created": "2026-07-09T15:13:27.813Z"
  }
]
```

- [ ] **Step 3: Написать failing tests**

`crates/kaiten-client/tests/checklists_test.rs` (полностью):

```rust
use kaiten_client::KaitenClient;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const CHECKLIST_ADD: &str = include_str!("fixtures/checklist_add.json");
const CHECKLIST_ITEM_ADD: &str = include_str!("fixtures/checklist_item_add.json");
const CHECKLIST_ITEM_CHECK: &str = include_str!("fixtures/checklist_item_check.json");

#[tokio::test]
async fn add_posts_name_and_parses_checklist_without_items_key() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cards/67089469/checklists"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_json(serde_json::json!({ "name": "todo" })))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CHECKLIST_ADD, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let checklist = client.checklists().add(67089469, "todo").await.unwrap();

    assert_eq!(checklist.id, 11747430);
    assert_eq!(checklist.name, "todo");
    // ответ без ключа items → #[serde(default)] даёт пустой вектор
    assert!(checklist.items.is_empty());
}

#[tokio::test]
async fn add_item_posts_text() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cards/67089469/checklists/11747430/items"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_json(serde_json::json!({ "text": "first item" })))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(CHECKLIST_ITEM_ADD, "application/json"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let item = client
        .checklists()
        .add_item(67089469, 11747430, "first item")
        .await
        .unwrap();

    assert_eq!(item.id, 65658564);
    assert_eq!(item.text, "first item");
    assert_eq!(item.checked, Some(false));
}

#[tokio::test]
async fn set_item_checked_patches_checked_flag() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/cards/67089469/checklists/11747430/items/65658564"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_json(serde_json::json!({ "checked": true })))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(CHECKLIST_ITEM_CHECK, "application/json"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let item = client
        .checklists()
        .set_item_checked(67089469, 11747430, 65658564, true)
        .await
        .unwrap();

    assert_eq!(item.id, 65658564);
    assert_eq!(item.checked, Some(true));
}
```

`crates/kaiten-client/tests/tags_test.rs` (полностью):

```rust
use kaiten_client::KaitenClient;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TAGS_LIST: &str = include_str!("fixtures/tags_list.json");
const CARD_TAG_ADD: &str = include_str!("fixtures/card_tag_add.json");
const CARD_TYPES: &str = include_str!("fixtures/card_types.json");

#[tokio::test]
async fn list_parses_company_tags() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/tags"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(TAGS_LIST, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let tags = client.tags().list().await.unwrap();

    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].id, 1110772);
    assert_eq!(tags[0].name, "cli-test");
    assert_eq!(tags[0].color, Some(15));
}

#[tokio::test]
async fn add_to_card_posts_tag_name() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cards/67089469/tags"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_json(serde_json::json!({ "name": "cli-test" })))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARD_TAG_ADD, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let tag = client.tags().add_to_card(67089469, "cli-test").await.unwrap();

    assert_eq!(tag.id, 1110772);
    assert_eq!(tag.name, "cli-test");
}

#[tokio::test]
async fn remove_from_card_returns_ok_on_empty_body() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/cards/67089469/tags/1110772"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    client.tags().remove_from_card(67089469, 1110772).await.unwrap();
}

#[tokio::test]
async fn card_types_parses_type_list() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/card-types"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARD_TYPES, "application/json"))
        .expect(1)
        .mount(&server)
        .await;

    let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
    let types = client.tags().card_types().await.unwrap();

    assert_eq!(types.len(), 2);
    assert_eq!(types[0].name, "Card");
    assert_eq!(types[1].name, "Bug");
    assert_eq!(types[1].letter.as_deref(), Some("B"));
    assert_eq!(types[1].archived, Some(false));
}
```

- [ ] **Step 4: Убедиться, что тесты падают**

Run: `cargo test -p kaiten-client --test checklists_test --test tags_test`
Expected: FAIL, `error[E0599]: no method named 'checklists' found for struct 'KaitenClient'` (и аналогично `tags`)

- [ ] **Step 5: Добавить модель Tag и фасады Checklists/Tags**

В конец `crates/kaiten-client/src/models.rs` добавить:

```rust
/// Company-level tag (GET /tags, POST /cards/{id}/tags).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Tag {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub color: Option<i64>,
}
```

`crates/kaiten-client/src/api/checklists.rs` (полностью):

```rust
use crate::client::KaitenClient;
use crate::error::Result;
use crate::models::{Checklist, ChecklistItem};

/// Checklists resource facade. Construct via [`KaitenClient::checklists`].
///
/// NOTE: `GET /cards/{id}/checklists` does NOT exist (the API answers 405).
/// Read checklists from `Card.checklists` via `cards().get()`.
pub struct Checklists<'a> {
    pub(crate) client: &'a KaitenClient,
}

impl Checklists<'_> {
    /// POST /cards/{id}/checklists
    pub async fn add(&self, card_id: u64, name: &str) -> Result<Checklist> {
        self.client
            .request(
                reqwest::Method::POST,
                &format!("/cards/{card_id}/checklists"),
                None,
                Some(serde_json::json!({ "name": name })),
            )
            .await
    }

    /// POST /cards/{card_id}/checklists/{checklist_id}/items
    pub async fn add_item(
        &self,
        card_id: u64,
        checklist_id: u64,
        text: &str,
    ) -> Result<ChecklistItem> {
        self.client
            .request(
                reqwest::Method::POST,
                &format!("/cards/{card_id}/checklists/{checklist_id}/items"),
                None,
                Some(serde_json::json!({ "text": text })),
            )
            .await
    }

    /// PATCH /cards/{card_id}/checklists/{checklist_id}/items/{item_id}
    pub async fn set_item_checked(
        &self,
        card_id: u64,
        checklist_id: u64,
        item_id: u64,
        checked: bool,
    ) -> Result<ChecklistItem> {
        self.client
            .request(
                reqwest::Method::PATCH,
                &format!("/cards/{card_id}/checklists/{checklist_id}/items/{item_id}"),
                None,
                Some(serde_json::json!({ "checked": checked })),
            )
            .await
    }
}
```

`crates/kaiten-client/src/api/tags.rs` (полностью):

```rust
use crate::client::KaitenClient;
use crate::error::Result;
use crate::models::{CardType, Tag};

/// Tags and card types facade. Construct via [`KaitenClient::tags`].
pub struct Tags<'a> {
    pub(crate) client: &'a KaitenClient,
}

impl Tags<'_> {
    /// GET /tags — company tags.
    pub async fn list(&self) -> Result<Vec<Tag>> {
        self.client
            .request(reqwest::Method::GET, "/tags", None, None)
            .await
    }

    /// POST /cards/{id}/tags — adds by name; creates the company tag if missing.
    pub async fn add_to_card(&self, card_id: u64, name: &str) -> Result<Tag> {
        self.client
            .request(
                reqwest::Method::POST,
                &format!("/cards/{card_id}/tags"),
                None,
                Some(serde_json::json!({ "name": name })),
            )
            .await
    }

    /// DELETE /cards/{id}/tags/{tag_id}; the response body is ignored.
    pub async fn remove_from_card(&self, card_id: u64, tag_id: u64) -> Result<()> {
        self.client
            .request_empty(
                reqwest::Method::DELETE,
                &format!("/cards/{card_id}/tags/{tag_id}"),
            )
            .await
    }

    /// GET /card-types
    pub async fn card_types(&self) -> Result<Vec<CardType>> {
        self.client
            .request(reqwest::Method::GET, "/card-types", None, None)
            .await
    }
}
```

`crates/kaiten-client/src/api/mod.rs` (полностью, финальное состояние):

```rust
pub mod boards;
pub mod cards;
pub mod checklists;
pub mod comments;
pub mod members;
pub mod spaces;
pub mod tags;
pub mod users;
```

В `impl KaitenClient` (`crates/kaiten-client/src/client.rs`) добавить:

```rust
    /// Checklists resource facade.
    pub fn checklists(&self) -> crate::api::checklists::Checklists<'_> {
        crate::api::checklists::Checklists { client: self }
    }

    /// Tags and card types facade.
    pub fn tags(&self) -> crate::api::tags::Tags<'_> {
        crate::api::tags::Tags { client: self }
    }
```

- [ ] **Step 6: Убедиться, что тесты проходят**

Run: `cargo test -p kaiten-client --test checklists_test --test tags_test`
Expected: PASS, `3 passed` (checklists) и `4 passed` (tags)

- [ ] **Step 7: Финальная проверка вехи «ресурсы клиента»**

Run: `cargo test -p kaiten-client`
Expected: PASS, все тесты крейта зелёные (users 2, spaces 1, boards 2, cards 6, comments 2, members 4, checklists 3, tags 4 + тесты ядра из Task 2)

Run: `cargo clippy --all-targets -- -D warnings`
Expected: PASS, no warnings

Run: `cargo fmt --all -- --check`
Expected: PASS, no diff (если diff — выполнить `cargo fmt --all` и включить правки в коммит)

- [ ] **Step 8: Commit**

```
git add crates/kaiten-client/src/models.rs crates/kaiten-client/src/api/mod.rs crates/kaiten-client/src/api/checklists.rs crates/kaiten-client/src/api/tags.rs crates/kaiten-client/src/client.rs crates/kaiten-client/tests/checklists_test.rs crates/kaiten-client/tests/tags_test.rs crates/kaiten-client/tests/fixtures/checklist_add.json crates/kaiten-client/tests/fixtures/checklist_item_add.json crates/kaiten-client/tests/fixtures/checklist_item_check.json crates/kaiten-client/tests/fixtures/tags_list.json crates/kaiten-client/tests/fixtures/card_tag_add.json crates/kaiten-client/tests/fixtures/card_types.json
git commit -m "feat(client): checklists, tags and card types api"
```

## Часть 3: CLI — скелет, auth, space/board, card list/view/create/edit/move/archive (Tasks 10–15)

Предпосылка: Tasks 1–9 дали крейт `kaiten-client` со всем API из INTERFACES.md §2:
`KaitenClient::new(base_url: &str, token: &str) -> kaiten_client::Result<Self>`, фасады
`users()/spaces()/boards()/cards()/comments()/checklists()/tags()/members()`, модели
(`User`, `Space`, `Board`, `Column`, `Lane`, `Card`, `CardType`, `Tag`, `CardTag`,
`CardMember`, `Checklist`, `ChecklistItem`, `Comment`), типы `CardFilter`, `CreateCard`,
`UpdateCard`, ошибка `KaitenError` — всё реэкспортировано из `kaiten_client` (lib.rs `pub use`).
Корневой `Cargo.toml` уже содержит `members = ["crates/kaiten-client", "crates/kaiten"]`
и все `workspace.dependencies` из §1.

### Task 10: CLI-скелет: clap-дерево, CliError, config, output, main

Фиксируем полное дерево команд из §4 сразу (clap-структуры всех сабкоманд), но все
обработчики — заглушки, возвращающие `CliError::InvalidArg("not implemented yet")`.
Задачи 11–15 (и часть 4) заменяют заглушки реализациями, не трогая `cli.rs` и `main.rs`.
Резолв конфига — через чистую функцию `resolve_from(file, env)`, как зафиксировано
в контракте (INTERFACES.md §4, config.rs).

**Files:**
- Modify: crates/kaiten/Cargo.toml — перезапись файла из Task 1 (список зависимостей объединяется, добавляется `reqwest`)
- Modify: crates/kaiten/src/main.rs — замена заглушки из Task 1
- Create: crates/kaiten/src/cli.rs
- Create: crates/kaiten/src/error.rs
- Create: crates/kaiten/src/config.rs (юнит-тесты внутри файла)
- Create: crates/kaiten/src/output.rs
- Create: crates/kaiten/src/commands/mod.rs
- Create: crates/kaiten/src/commands/auth.rs, space.rs, board.rs, card.rs, tag.rs, card_type.rs, api.rs, completion.rs (заглушки)
- Create: crates/kaiten/src/mcp/mod.rs (заглушка)
- Test: crates/kaiten/tests/cli_skeleton_test.rs

**Interfaces:**
- Consumes: `kaiten_client::KaitenClient::new(&str, &str) -> kaiten_client::Result<KaitenClient>`, `kaiten_client::KaitenError`
- Produces:
  - `cli::{Cli, Commands, AuthCmd, SpaceCmd, BoardCmd, CardCmd, CardMemberCmd, CardCommentCmd, CardChecklistCmd, CardChecklistItemCmd, CardTagCmd, TagCmd, CardTypeCmd, McpCmd, Shell}`
  - `error::CliError`
  - `config::{FileConfig, Defaults, TokenSource, Resolved, resolve() -> Result<Resolved, CliError>, resolve_from(FileConfig, &HashMap<String, String>) -> Result<Resolved, CliError>}`
  - `output::{print_json<T: serde::Serialize>(&T) -> Result<(), CliError>, table(&[&str]) -> comfy_table::Table}`
  - Сигнатуры обработчиков (заглушки, задачи 11+ заменяют тела, сигнатуры финальные):
    - `commands::auth::run(cmd: AuthCmd, json: bool) -> Result<(), CliError>` (async)
    - `commands::space::run(cmd: SpaceCmd, client: &KaitenClient, json: bool) -> Result<(), CliError>` (async)
    - `commands::board::run(cmd: BoardCmd, client: &KaitenClient, defaults: &Defaults, json: bool) -> Result<(), CliError>` (async)
    - `commands::card::run(cmd: CardCmd, client: &KaitenClient, defaults: &Defaults, json: bool) -> Result<(), CliError>` (async)
    - `commands::tag::run(cmd: TagCmd, client: &KaitenClient, json: bool) -> Result<(), CliError>` (async)
    - `commands::card_type::run(cmd: CardTypeCmd, client: &KaitenClient, json: bool) -> Result<(), CliError>` (async)
    - `commands::api::run(client: &KaitenClient, method: &str, path: &str, data: Option<String>) -> Result<(), CliError>` (async)
    - `commands::completion::run(shell: Shell) -> Result<(), CliError>` (sync)
    - `mcp::run(cmd: McpCmd) -> Result<(), CliError>` (async)

- [ ] **Step 1: Cargo.toml крейта kaiten**

Переписать `crates/kaiten/Cargo.toml` (создан в Task 1) целиком. Список
`[dependencies]` — ОБЪЕДИНЕНИЕ списка Task 1 и новых зависимостей: `url` из Task 1
сохраняется, добавляется `reqwest`:

```toml
[package]
name = "kaiten"
version.workspace = true
edition.workspace = true

[[bin]]
name = "kaiten"
path = "src/main.rs"

[dependencies]
kaiten-client = { path = "../kaiten-client" }
clap = { workspace = true }
clap_complete = { workspace = true }
comfy-table = { workspace = true }
dirs = { workspace = true }
# reqwest::Method is constructed by the `api` command (Task 19)
reqwest = { workspace = true }
rpassword = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
toml = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
url = { workspace = true }

[dev-dependencies]
assert_cmd = { workspace = true }
insta = { workspace = true }
predicates = { workspace = true }
tempfile = { workspace = true }
wiremock = { workspace = true }
```

(`reqwest` нужен команде `api` (Task 19, часть 4) — она конструирует `reqwest::Method`
для `KaitenClient::raw`.)

- [ ] **Step 2: failing integration-тест скелета**

Создать `crates/kaiten/tests/cli_skeleton_test.rs`:

```rust
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn help_lists_all_subcommands() {
    let mut cmd = Command::cargo_bin("kaiten").unwrap();
    cmd.env("NO_COLOR", "1");
    let assert = cmd.arg("--help").assert().success();
    let out = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    for sub in [
        "auth",
        "space",
        "board",
        "card",
        "card-type",
        "tag",
        "api",
        "completion",
        "mcp",
    ] {
        assert!(out.contains(sub), "help must mention `{sub}`:\n{out}");
    }
}

#[test]
fn card_list_without_config_fails_with_no_token() {
    let tmp = tempfile::tempdir().unwrap();
    let mut cmd = Command::cargo_bin("kaiten").unwrap();
    cmd.env_remove("KAITEN_TOKEN")
        .env_remove("KAITEN_DOMAIN")
        .env_remove("KAITEN_BASE_URL")
        .env_remove("RUST_LOG")
        .env("KAITEN_CONFIG_DIR", tmp.path())
        .env("NO_COLOR", "1");
    cmd.args(["card", "list"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("no token"));
}
```

- [ ] **Step 3: убедиться, что тест падает**

Run: `cargo test -p kaiten`
Expected: FAIL — крейт собирается (в `src/main.rs` пока заглушка из Task 1, печатающая
"kaiten: work in progress" с кодом 0): `help_lists_all_subcommands` падает на assert
содержимого `--help` (stdout не содержит ни одной сабкоманды),
`card_list_without_config_fails_with_no_token` падает на `.failure()` — заглушка
выходит с кодом 0 и без "no token" в stderr.

- [ ] **Step 4: src/error.rs (точно из INTERFACES.md §4)**

```rust
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error(transparent)]
    Api(#[from] kaiten_client::KaitenError),
    #[error("config: {0}")]
    Config(String),
    #[error("{0}")]
    InvalidArg(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}
```

- [ ] **Step 5: src/output.rs**

```rust
use crate::error::CliError;

pub fn print_json<T: serde::Serialize>(value: &T) -> Result<(), CliError> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

pub fn table(headers: &[&str]) -> comfy_table::Table {
    let mut table = comfy_table::Table::new();
    table.load_preset(comfy_table::presets::UTF8_BORDERS_ONLY);
    table.set_header(
        headers
            .iter()
            .map(|h| comfy_table::Cell::new(h).add_attribute(comfy_table::Attribute::Bold))
            .collect::<Vec<_>>(),
    );
    table
}
```

(comfy-table сам отключает стили, когда stdout не tty, поэтому снапшоты в тестах — без ANSI-кодов.)

- [ ] **Step 6: src/config.rs с юнит-тестами**

Сигнатуры `resolve()`/`resolve_from()` — из контракта §4: `resolve()` — тонкая обёртка,
собирающая `std::env::vars()` в `HashMap`; юнит-тесты гоняют `resolve_from`.
`FileConfig::save` добавится в Task 11 (первое место использования).

```rust
use std::collections::HashMap;
use std::path::PathBuf;

use crate::error::CliError;

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct FileConfig {
    pub domain: Option<String>,
    pub token: Option<String>,
    #[serde(default)]
    pub defaults: Defaults,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Defaults {
    pub space: Option<u64>,
    pub board: Option<u64>,
}

#[derive(Debug)]
pub enum TokenSource {
    Env,
    File,
}

#[derive(Debug)]
pub struct Resolved {
    pub base_url: String,
    pub token: String,
    pub token_source: TokenSource,
    pub defaults: Defaults,
}

impl FileConfig {
    /// $KAITEN_CONFIG_DIR || $XDG_CONFIG_HOME/kaiten || ~/.config/kaiten
    pub fn dir() -> PathBuf {
        if let Ok(dir) = std::env::var("KAITEN_CONFIG_DIR") {
            return PathBuf::from(dir);
        }
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            return PathBuf::from(xdg).join("kaiten");
        }
        dirs::home_dir()
            .map(|home| home.join(".config").join("kaiten"))
            .unwrap_or_else(|| PathBuf::from(".config/kaiten"))
    }

    /// Отсутствие файла — не ошибка: возвращается Default.
    pub fn load() -> Result<FileConfig, CliError> {
        let path = Self::dir().join("config.toml");
        let body = match std::fs::read_to_string(&path) {
            Ok(body) => body,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Ok(FileConfig::default());
            }
            Err(err) => return Err(CliError::Io(err)),
        };
        toml::from_str(&body)
            .map_err(|err| CliError::Config(format!("invalid config {}: {err}", path.display())))
    }
}

pub fn resolve() -> Result<Resolved, CliError> {
    let env: HashMap<String, String> = std::env::vars().collect();
    resolve_from(FileConfig::load()?, &env)
}

/// Приоритет: env (KAITEN_TOKEN/KAITEN_DOMAIN/KAITEN_BASE_URL) → файл.
pub fn resolve_from(
    file: FileConfig,
    env: &HashMap<String, String>,
) -> Result<Resolved, CliError> {
    let (token, token_source) = match env.get("KAITEN_TOKEN").filter(|t| !t.is_empty()) {
        Some(token) => (token.clone(), TokenSource::Env),
        None => match file.token {
            Some(token) => (token, TokenSource::File),
            None => {
                return Err(CliError::Config(
                    "no token: run `kaiten auth login` or set KAITEN_TOKEN".into(),
                ));
            }
        },
    };
    let base_url = match env.get("KAITEN_BASE_URL").filter(|u| !u.is_empty()) {
        Some(url) => url.trim_end_matches('/').to_string(),
        None => {
            let domain = env
                .get("KAITEN_DOMAIN")
                .filter(|d| !d.is_empty())
                .cloned()
                .or(file.domain);
            match domain {
                Some(domain) => format!("https://{domain}.kaiten.ru/api/latest"),
                None => {
                    return Err(CliError::Config(
                        "no domain: run `kaiten auth login` or set KAITEN_DOMAIN".into(),
                    ));
                }
            }
        }
    };
    Ok(Resolved {
        base_url,
        token,
        token_source,
        defaults: file.defaults,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn env_overrides_file() {
        let file = FileConfig {
            domain: Some("filedomain".into()),
            token: Some("file-token".into()),
            defaults: Defaults::default(),
        };
        let env = env(&[("KAITEN_TOKEN", "env-token"), ("KAITEN_DOMAIN", "envdomain")]);
        let resolved = resolve_from(file, &env).unwrap();
        assert_eq!(resolved.token, "env-token");
        assert!(matches!(resolved.token_source, TokenSource::Env));
        assert_eq!(resolved.base_url, "https://envdomain.kaiten.ru/api/latest");
    }

    #[test]
    fn file_values_used_when_env_empty() {
        let file = FileConfig {
            domain: Some("mycompany".into()),
            token: Some("file-token".into()),
            defaults: Defaults::default(),
        };
        let resolved = resolve_from(file, &HashMap::new()).unwrap();
        assert_eq!(resolved.token, "file-token");
        assert!(matches!(resolved.token_source, TokenSource::File));
        assert_eq!(resolved.base_url, "https://mycompany.kaiten.ru/api/latest");
    }

    #[test]
    fn base_url_env_wins_over_domain_and_needs_no_domain() {
        let env = env(&[
            ("KAITEN_TOKEN", "t"),
            ("KAITEN_BASE_URL", "http://127.0.0.1:9999/"),
        ]);
        let resolved = resolve_from(FileConfig::default(), &env).unwrap();
        assert_eq!(resolved.base_url, "http://127.0.0.1:9999");
    }

    #[test]
    fn missing_token_is_clear_error() {
        let env = env(&[("KAITEN_DOMAIN", "mycompany")]);
        let err = resolve_from(FileConfig::default(), &env).unwrap_err();
        assert!(matches!(err, CliError::Config(_)));
        assert!(err.to_string().contains("no token"), "{err}");
    }

    #[test]
    fn missing_domain_is_clear_error() {
        let env = env(&[("KAITEN_TOKEN", "t")]);
        let err = resolve_from(FileConfig::default(), &env).unwrap_err();
        assert!(err.to_string().contains("no domain"), "{err}");
    }

    #[test]
    fn defaults_parse_from_toml() {
        let file: FileConfig = toml::from_str(
            "domain = \"mycompany\"\ntoken = \"t\"\n\n[defaults]\nspace = 123\nboard = 456\n",
        )
        .unwrap();
        assert_eq!(file.defaults.space, Some(123));
        assert_eq!(file.defaults.board, Some(456));
        let resolved = resolve_from(file, &HashMap::new()).unwrap();
        assert_eq!(resolved.defaults.board, Some(456));
    }
}
```

- [ ] **Step 7: src/cli.rs — полное clap-дерево из §4**

```rust
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "kaiten", version, about = "Kaiten tracker CLI", propagate_version = true)]
pub struct Cli {
    /// Print raw JSON instead of tables
    #[arg(long, global = true)]
    pub json: bool,

    /// Increase log verbosity (-v: debug, -vv: trace)
    #[arg(short = 'v', long = "verbose", action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Log in and inspect authentication
    #[command(subcommand)]
    Auth(AuthCmd),
    /// Work with spaces
    #[command(subcommand)]
    Space(SpaceCmd),
    /// Work with boards
    #[command(subcommand)]
    Board(BoardCmd),
    /// Work with cards
    #[command(subcommand)]
    Card(CardCmd),
    /// Work with company tags
    #[command(subcommand)]
    Tag(TagCmd),
    /// Work with card types
    #[command(name = "card-type", subcommand)]
    CardType(CardTypeCmd),
    /// Raw API request (like `gh api`)
    Api {
        /// HTTP method: GET|POST|PATCH|PUT|DELETE
        method: String,
        /// Path starting with '/', query string included
        path: String,
        /// JSON request body
        #[arg(long)]
        data: Option<String>,
    },
    /// Generate shell completion script
    Completion {
        #[arg(value_enum)]
        shell: Shell,
    },
    /// MCP server
    #[command(subcommand)]
    Mcp(McpCmd),
}

#[derive(Subcommand)]
pub enum AuthCmd {
    /// Verify credentials against /users/current and save config
    Login {
        /// Kaiten domain: <domain>.kaiten.ru
        #[arg(long)]
        domain: Option<String>,
        /// API token (from your Kaiten profile)
        #[arg(long)]
        token: Option<String>,
    },
    /// Show current authentication info
    Status,
}

#[derive(Subcommand)]
pub enum SpaceCmd {
    /// List spaces
    List,
}

#[derive(Subcommand)]
pub enum BoardCmd {
    /// List boards of a space
    List {
        /// Space id (default: defaults.space from config)
        #[arg(long)]
        space: Option<u64>,
    },
    /// Show board columns and lanes
    View { board_id: u64 },
}

#[derive(Subcommand)]
pub enum CardCmd {
    /// List cards
    List {
        /// Filter by space id
        #[arg(long)]
        space: Option<u64>,
        /// Filter by board id
        #[arg(long)]
        board: Option<u64>,
        /// Filter by column id
        #[arg(long)]
        column: Option<u64>,
        /// Only cards where I am a member
        #[arg(long)]
        mine: bool,
        /// Only cards where user <id> is a member
        #[arg(long)]
        member: Option<u64>,
        /// Full-text search query
        #[arg(long)]
        query: Option<String>,
        /// Filter by tag name
        #[arg(long)]
        tag: Option<String>,
        /// Filter by card type id
        #[arg(long = "type")]
        type_id: Option<u64>,
        /// Include only archived cards
        #[arg(long)]
        archived: bool,
        /// Max number of cards
        #[arg(long, default_value_t = 50)]
        limit: u32,
    },
    /// Show one card (accepts id or browser URL)
    View {
        card: String,
        /// Also fetch and print comments
        #[arg(long)]
        comments: bool,
    },
    /// Create a card
    Create {
        #[arg(long)]
        title: String,
        /// Board id (default: defaults.board from config)
        #[arg(long)]
        board: Option<u64>,
        #[arg(long)]
        column: Option<u64>,
        #[arg(long)]
        lane: Option<u64>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long = "type")]
        type_id: Option<u64>,
        /// Mark card as ASAP
        #[arg(long)]
        asap: bool,
    },
    /// Edit card fields
    Edit {
        card: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long = "type")]
        type_id: Option<u64>,
        /// true|false
        #[arg(long)]
        asap: Option<bool>,
    },
    /// Move card to another column/lane/board
    Move {
        card: String,
        #[arg(long)]
        column: u64,
        #[arg(long)]
        lane: Option<u64>,
        #[arg(long)]
        board: Option<u64>,
    },
    /// Archive card
    Archive { card: String },
    /// Card members
    #[command(subcommand)]
    Member(CardMemberCmd),
    /// Card comments
    #[command(subcommand)]
    Comment(CardCommentCmd),
    /// Card checklists
    #[command(subcommand)]
    Checklist(CardChecklistCmd),
    /// Card tags
    #[command(subcommand)]
    Tag(CardTagCmd),
}

#[derive(Subcommand)]
pub enum CardMemberCmd {
    /// Add member (user id or email)
    Add { card: String, user: String },
    /// Remove member (user id or email)
    Remove { card: String, user: String },
}

#[derive(Subcommand)]
pub enum CardCommentCmd {
    /// Add a comment
    Add {
        card: String,
        #[arg(long)]
        body: String,
    },
    /// List comments
    List { card: String },
}

#[derive(Subcommand)]
pub enum CardChecklistCmd {
    /// List checklists with items
    List { card: String },
    /// Add a checklist
    Add {
        card: String,
        #[arg(long)]
        name: String,
    },
    /// Checklist items
    #[command(subcommand)]
    Item(CardChecklistItemCmd),
}

#[derive(Subcommand)]
pub enum CardChecklistItemCmd {
    /// Add an item
    Add {
        card: String,
        checklist_id: u64,
        #[arg(long)]
        text: String,
    },
    /// Check an item
    Check {
        card: String,
        checklist_id: u64,
        item_id: u64,
    },
    /// Uncheck an item
    Uncheck {
        card: String,
        checklist_id: u64,
        item_id: u64,
    },
}

#[derive(Subcommand)]
pub enum CardTagCmd {
    /// Add tag by name
    Add { card: String, name: String },
    /// Remove tag by name
    Remove { card: String, name: String },
}

#[derive(Subcommand)]
pub enum TagCmd {
    /// List company tags
    List,
}

#[derive(Subcommand)]
pub enum CardTypeCmd {
    /// List card types
    List,
}

#[derive(Subcommand)]
pub enum McpCmd {
    /// Run MCP server on stdio
    Serve,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
}
```

- [ ] **Step 8: заглушки commands/* и mcp/mod.rs**

`crates/kaiten/src/commands/mod.rs`:

```rust
pub mod api;
pub mod auth;
pub mod board;
pub mod card;
pub mod card_type;
pub mod completion;
pub mod space;
pub mod tag;
```

`crates/kaiten/src/commands/auth.rs`:

```rust
use crate::cli::AuthCmd;
use crate::error::CliError;

pub async fn run(_cmd: AuthCmd, _json: bool) -> Result<(), CliError> {
    Err(CliError::InvalidArg("not implemented yet".into()))
}
```

`crates/kaiten/src/commands/space.rs`:

```rust
use kaiten_client::KaitenClient;

use crate::cli::SpaceCmd;
use crate::error::CliError;

pub async fn run(_cmd: SpaceCmd, _client: &KaitenClient, _json: bool) -> Result<(), CliError> {
    Err(CliError::InvalidArg("not implemented yet".into()))
}
```

`crates/kaiten/src/commands/board.rs`:

```rust
use kaiten_client::KaitenClient;

use crate::cli::BoardCmd;
use crate::config::Defaults;
use crate::error::CliError;

pub async fn run(
    _cmd: BoardCmd,
    _client: &KaitenClient,
    _defaults: &Defaults,
    _json: bool,
) -> Result<(), CliError> {
    Err(CliError::InvalidArg("not implemented yet".into()))
}
```

`crates/kaiten/src/commands/card.rs`:

```rust
use kaiten_client::KaitenClient;

use crate::cli::CardCmd;
use crate::config::Defaults;
use crate::error::CliError;

pub async fn run(
    _cmd: CardCmd,
    _client: &KaitenClient,
    _defaults: &Defaults,
    _json: bool,
) -> Result<(), CliError> {
    Err(CliError::InvalidArg("not implemented yet".into()))
}
```

`crates/kaiten/src/commands/tag.rs`:

```rust
use kaiten_client::KaitenClient;

use crate::cli::TagCmd;
use crate::error::CliError;

pub async fn run(_cmd: TagCmd, _client: &KaitenClient, _json: bool) -> Result<(), CliError> {
    Err(CliError::InvalidArg("not implemented yet".into()))
}
```

`crates/kaiten/src/commands/card_type.rs`:

```rust
use kaiten_client::KaitenClient;

use crate::cli::CardTypeCmd;
use crate::error::CliError;

pub async fn run(_cmd: CardTypeCmd, _client: &KaitenClient, _json: bool) -> Result<(), CliError> {
    Err(CliError::InvalidArg("not implemented yet".into()))
}
```

`crates/kaiten/src/commands/api.rs`:

```rust
use kaiten_client::KaitenClient;

use crate::error::CliError;

pub async fn run(
    _client: &KaitenClient,
    _method: &str,
    _path: &str,
    _data: Option<String>,
) -> Result<(), CliError> {
    Err(CliError::InvalidArg("not implemented yet".into()))
}
```

`crates/kaiten/src/commands/completion.rs`:

```rust
use crate::cli::Shell;
use crate::error::CliError;

pub fn run(_shell: Shell) -> Result<(), CliError> {
    Err(CliError::InvalidArg("not implemented yet".into()))
}
```

`crates/kaiten/src/mcp/mod.rs`:

```rust
use crate::cli::McpCmd;
use crate::error::CliError;

pub async fn run(cmd: McpCmd) -> Result<(), CliError> {
    match cmd {
        McpCmd::Serve => Err(CliError::InvalidArg("not implemented yet".into())),
    }
}
```

- [ ] **Step 9: src/main.rs — tracing в stderr, dispatch, ExitCode**

Заменить заглушку из Task 1 целиком:

```rust
mod cli;
mod commands;
mod config;
mod error;
mod mcp;
mod output;

use std::process::ExitCode;

use clap::Parser;

use crate::cli::{Cli, Commands};
use crate::error::CliError;

fn init_tracing(verbosity: u8) {
    let filter = match verbosity {
        0 => tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        1 => tracing_subscriber::EnvFilter::new("kaiten=debug,kaiten_client=debug"),
        _ => tracing_subscriber::EnvFilter::new("kaiten=trace,kaiten_client=trace,reqwest=debug"),
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}

async fn run(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Commands::Completion { shell } => commands::completion::run(shell),
        Commands::Auth(cmd) => commands::auth::run(cmd, cli.json).await,
        Commands::Mcp(cmd) => mcp::run(cmd).await,
        command => {
            let resolved = config::resolve()?;
            let client = kaiten_client::KaitenClient::new(&resolved.base_url, &resolved.token)?;
            match command {
                Commands::Space(cmd) => commands::space::run(cmd, &client, cli.json).await,
                Commands::Board(cmd) => {
                    commands::board::run(cmd, &client, &resolved.defaults, cli.json).await
                }
                Commands::Card(cmd) => {
                    commands::card::run(cmd, &client, &resolved.defaults, cli.json).await
                }
                Commands::Tag(cmd) => commands::tag::run(cmd, &client, cli.json).await,
                Commands::CardType(cmd) => {
                    commands::card_type::run(cmd, &client, cli.json).await
                }
                Commands::Api { method, path, data } => {
                    commands::api::run(&client, &method, &path, data).await
                }
                Commands::Completion { .. } | Commands::Auth(_) | Commands::Mcp(_) => {
                    unreachable!("handled above")
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    init_tracing(cli.verbose);
    match run(cli).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("kaiten: {err}");
            if let CliError::Api(kaiten_client::KaitenError::Api { message, body, .. }) = &err
                && !body.is_empty()
                && body != message
            {
                eprintln!("{body}");
            }
            ExitCode::FAILURE
        }
    }
}
```

`config::resolve()` вызывается ДО диспетчеризации API-команд — поэтому `card list` без
конфига падает с "no token" даже пока сама команда — заглушка.

`Display` у `KaitenError::Api` печатает только `message`; сырое тело ответа (`body`)
main печатает отдельной строкой в stderr, если оно непустое и не совпадает с `message` —
так пользователь видит и человекочитаемое сообщение, и полный ответ API. Поведение
проверяется CLI-тестом в Task 15 (`create_api_error_prints_message_and_body`).

- [ ] **Step 10: убедиться, что всё проходит**

Run: `cargo test -p kaiten`
Expected: PASS — 6 юнит-тестов `config::tests::*` + 2 integration-теста `cli_skeleton_test`.

- [ ] **Step 11: линты**

Run: `cargo clippy -p kaiten --all-targets -- -D warnings && cargo fmt --all -- --check`
Expected: без ошибок и диффов.

- [ ] **Step 12: commit**

```
git add crates/kaiten
git commit -m "feat(cli): skeleton, config and output"
```

### Task 11: auth login / auth status

Login: `--domain`/`--token` или интерактив (домен — `read_line` c промптом в stderr,
токен — `rpassword::prompt_password`); проверка через `client.users().current()`;
сохранение только после успешной проверки, файл 0600. `KAITEN_BASE_URL` уважается и в
login — так команда тестируется против wiremock (в тестах login гоняется ТОЛЬКО флагами).
Status: домен, base_url, username текущего юзера, источник токена (env/file).

**Files:**
- Modify: crates/kaiten/src/config.rs — добавить `FileConfig::save()`
- Modify: crates/kaiten/src/commands/auth.rs — заменить заглушку реализацией
- Create: crates/kaiten/tests/fixtures/users_current.json
- Test: crates/kaiten/tests/auth_test.rs

**Interfaces:**
- Consumes: `cli::AuthCmd`, `config::{FileConfig, resolve, TokenSource}`, `error::CliError`, `output::print_json`, `kaiten_client::{KaitenClient, User}`, `client.users().current() -> kaiten_client::Result<User>`
- Produces: `FileConfig::save(&self) -> Result<(), CliError>` (права 0600, создаёт каталог) — используется всеми будущими задачами, пишущими конфиг; рабочие `kaiten auth login|status`

- [ ] **Step 1: фикстура users_current.json**

Создать `crates/kaiten/tests/fixtures/users_current.json` — урезанный реальный ответ
`GET /users/current` (все поля модели `User` + 6 лишних для толерантности, email обезличен):

```json
{
  "id": 1068514,
  "uid": "0abd61ea-9dc5-40eb-b0a9-0d452ba1d8a6",
  "full_name": "dxmuser",
  "username": "dxmuser",
  "email": "user@example.com",
  "activated": true,
  "lng": "ru",
  "timezone": "UTC",
  "theme": "auto",
  "company_id": 398610,
  "role": 1,
  "ui_version": 2
}
```

- [ ] **Step 2: failing тесты auth_test.rs**

Создать `crates/kaiten/tests/auth_test.rs`:

```rust
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
        .args(["auth", "login", "--domain", "mycompany", "--token", "secret-token"])
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
        let mode = std::fs::metadata(&config_path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "config.toml must be 0600");
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
```

- [ ] **Step 3: убедиться, что тесты падают**

Run: `cargo test -p kaiten --test auth_test`
Expected: FAIL — все 4 теста: заглушка auth возвращает exit 1 c "not implemented yet",
`assert().success()` не проходит (а в bad-token-тесте stderr не содержит "401").

- [ ] **Step 4: FileConfig::save в config.rs**

Добавить метод в `impl FileConfig` (после `load()`), больше в файле ничего не менять:

```rust
    /// Создаёт каталог, пишет config.toml с правами 0600 (unix).
    pub fn save(&self) -> Result<(), CliError> {
        let dir = Self::dir();
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("config.toml");
        let body = toml::to_string_pretty(self)
            .map_err(|err| CliError::Config(format!("failed to serialize config: {err}")))?;
        std::fs::write(&path, body)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(())
    }
```

- [ ] **Step 5: реализация commands/auth.rs**

Заменить содержимое файла целиком:

```rust
use std::io::Write;

use kaiten_client::KaitenClient;

use crate::cli::AuthCmd;
use crate::config::{self, FileConfig, TokenSource};
use crate::error::CliError;
use crate::output;

pub async fn run(cmd: AuthCmd, json: bool) -> Result<(), CliError> {
    match cmd {
        AuthCmd::Login { domain, token } => login(domain, token).await,
        AuthCmd::Status => status(json).await,
    }
}

async fn login(domain: Option<String>, token: Option<String>) -> Result<(), CliError> {
    let domain = match domain {
        Some(domain) => domain,
        None => prompt_line("Kaiten domain (as in https://<domain>.kaiten.ru): ")?,
    };
    let domain = domain.trim().to_string();
    if domain.is_empty() {
        return Err(CliError::InvalidArg("domain must not be empty".into()));
    }
    let token = match token {
        Some(token) => token,
        None => rpassword::prompt_password("API token: ")?,
    };
    if token.is_empty() {
        return Err(CliError::InvalidArg("token must not be empty".into()));
    }
    // KAITEN_BASE_URL is honored so login can be pointed at a mock server in tests.
    let base_url = std::env::var("KAITEN_BASE_URL")
        .unwrap_or_else(|_| format!("https://{domain}.kaiten.ru/api/latest"));
    let client = KaitenClient::new(&base_url, &token)?;
    let user = client.users().current().await?;

    let mut file = FileConfig::load()?;
    file.domain = Some(domain.clone());
    file.token = Some(token);
    file.save()?;

    println!("Logged in to {domain}.kaiten.ru as {}", user_label(&user));
    Ok(())
}

async fn status(json: bool) -> Result<(), CliError> {
    let resolved = config::resolve()?;
    let file = FileConfig::load()?;
    let domain = std::env::var("KAITEN_DOMAIN").ok().or(file.domain);
    let client = KaitenClient::new(&resolved.base_url, &resolved.token)?;
    let user = client.users().current().await?;
    let source = match resolved.token_source {
        TokenSource::Env => "env",
        TokenSource::File => "file",
    };
    if json {
        return output::print_json(&serde_json::json!({
            "domain": domain,
            "base_url": resolved.base_url,
            "token_source": source,
            "user": user,
        }));
    }
    println!("domain:       {}", domain.as_deref().unwrap_or("-"));
    println!("base_url:     {}", resolved.base_url);
    println!("token source: {source}");
    println!("logged in as: {} (id {})", user_label(&user), user.id);
    Ok(())
}

fn user_label(user: &kaiten_client::User) -> String {
    user.username
        .clone()
        .or_else(|| user.full_name.clone())
        .unwrap_or_else(|| user.id.to_string())
}

fn prompt_line(prompt: &str) -> Result<String, CliError> {
    let mut stderr = std::io::stderr();
    write!(stderr, "{prompt}")?;
    stderr.flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    Ok(line.trim().to_string())
}
```

- [ ] **Step 6: убедиться, что тесты проходят**

Run: `cargo test -p kaiten --test auth_test`
Expected: PASS — 4 теста. Затем полный прогон: `cargo test -p kaiten` → PASS (скелетные тесты не сломаны).

- [ ] **Step 7: линты**

Run: `cargo clippy -p kaiten --all-targets -- -D warnings && cargo fmt --all -- --check`
Expected: чисто.

- [ ] **Step 8: commit**

```
git add crates/kaiten/src/config.rs crates/kaiten/src/commands/auth.rs crates/kaiten/tests/auth_test.rs crates/kaiten/tests/fixtures/users_current.json
git commit -m "feat(cli): auth login/status"
```

### Task 12: space list / board list / board view

Таблицы: spaces — `ID, TITLE`; boards — `ID, TITLE`; board view — заголовок и две таблицы:
Columns (`ID, TITLE, TYPE` где type 1/2/3 → queued/in progress/done) и Lanes (`ID, TITLE`).
`board list` без `--space` берёт `defaults.space`, иначе `InvalidArg`.

**Files:**
- Modify: crates/kaiten/src/commands/space.rs — заменить заглушку
- Modify: crates/kaiten/src/commands/board.rs — заменить заглушку
- Create: crates/kaiten/tests/fixtures/spaces_list.json, boards_list.json, board_get.json
- Test: crates/kaiten/tests/space_board_test.rs (+ снапшоты crates/kaiten/tests/snapshots/*.snap)

**Interfaces:**
- Consumes: `client.spaces().list() -> Result<Vec<Space>>`, `client.boards().list(space_id: u64) -> Result<Vec<Board>>`, `client.boards().get(board_id: u64) -> Result<Board>`, `output::{table, print_json}`, `config::Defaults`
- Produces: рабочие `kaiten space list`, `kaiten board list [--space]`, `kaiten board view <id>`

- [ ] **Step 1: фикстуры**

`crates/kaiten/tests/fixtures/spaces_list.json` — урезанный реальный `GET /spaces`
(поля модели `Space` + 4 лишних):

```json
[
  {
    "id": 810671,
    "uid": "8d5463f5-0752-4a08-b074-99d9617fbd4e",
    "title": "kaiten-cli-test",
    "archived": false,
    "entity_type": "space",
    "company_id": 398610,
    "access": "by_invite",
    "sort_order": 746.358756626496
  },
  {
    "id": 810669,
    "uid": "f52db47b-cbd9-4b50-98e7-19219cae0291",
    "title": "Первое пространство",
    "archived": false,
    "entity_type": "space",
    "company_id": 398610,
    "access": "by_invite",
    "sort_order": 1.6486044425519069
  }
]
```

`crates/kaiten/tests/fixtures/boards_list.json` — форма из реального `GET /spaces/{id}/boards`
(в списке колонки/дорожки не приходят → `#[serde(default)]` даёт пустые Vec; + 4 лишних поля):

```json
[
  {
    "id": 1826109,
    "title": "test-board",
    "default_card_type_id": 1,
    "space_id": 810671,
    "email_key": "82d0383fccb73049",
    "sort_order": 657.8751055564335,
    "type": 1
  },
  {
    "id": 1826105,
    "title": "Задачи",
    "default_card_type_id": 1,
    "space_id": 810671,
    "email_key": "d9e2d48f2e54d94c",
    "sort_order": 812.8977723550372,
    "type": 1
  }
]
```

`crates/kaiten/tests/fixtures/board_get.json` — урезанный реальный `GET /boards/{id}`;
к реальной единственной колонке добавлены ещё две той же формы с `type` 2 и 3, чтобы
покрыть весь маппинг queued/in progress/done:

```json
{
  "id": 1826109,
  "title": "test-board",
  "default_card_type_id": 1,
  "uid": "06a433b5-3626-48d4-865b-398312b08c3c",
  "email_key": "82d0383fccb73049",
  "created": "2026-07-09T15:17:57.631Z",
  "updated": "2026-07-09T15:17:57.631Z",
  "columns": [
    {
      "id": 6308511,
      "title": "To Do",
      "sort_order": 1,
      "type": 1,
      "board_id": 1826109,
      "col_count": 1,
      "rules": 0,
      "pause_sla": false
    },
    {
      "id": 6308512,
      "title": "In Progress",
      "sort_order": 2,
      "type": 2,
      "board_id": 1826109,
      "col_count": 1,
      "rules": 0,
      "pause_sla": false
    },
    {
      "id": 6308513,
      "title": "Done",
      "sort_order": 3,
      "type": 3,
      "board_id": 1826109,
      "col_count": 1,
      "rules": 0,
      "pause_sla": false
    }
  ],
  "lanes": [
    {
      "id": 2293584,
      "title": "Default Lane",
      "sort_order": 1,
      "board_id": 1826109,
      "condition": 1
    }
  ]
}
```

- [ ] **Step 2: failing тесты space_board_test.rs**

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SPACES: &str = include_str!("fixtures/spaces_list.json");
const BOARDS: &str = include_str!("fixtures/boards_list.json");
const BOARD: &str = include_str!("fixtures/board_get.json");

fn kaiten(config_dir: &std::path::Path, base_url: &str) -> Command {
    let mut cmd = Command::cargo_bin("kaiten").unwrap();
    cmd.env_remove("KAITEN_TOKEN")
        .env_remove("KAITEN_DOMAIN")
        .env_remove("KAITEN_BASE_URL")
        .env_remove("RUST_LOG")
        .env("KAITEN_CONFIG_DIR", config_dir)
        .env("KAITEN_BASE_URL", base_url)
        .env("KAITEN_TOKEN", "test-token")
        .env("NO_COLOR", "1");
    cmd
}

async fn mock_get(server: &MockServer, url_path: &str, body: &str) {
    Mock::given(method("GET"))
        .and(path(url_path))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "application/json"))
        .expect(1)
        .mount(server)
        .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn space_list_prints_table() {
    let server = MockServer::start().await;
    mock_get(&server, "/spaces", SPACES).await;
    let tmp = tempfile::tempdir().unwrap();

    let assert = kaiten(tmp.path(), &server.uri())
        .args(["space", "list"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    insta::assert_snapshot!("space_list", stdout);
}

#[tokio::test(flavor = "multi_thread")]
async fn space_list_json() {
    let server = MockServer::start().await;
    mock_get(&server, "/spaces", SPACES).await;
    let tmp = tempfile::tempdir().unwrap();

    let assert = kaiten(tmp.path(), &server.uri())
        .args(["space", "list", "--json"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value[0]["id"], 810671);
    assert_eq!(value[0]["title"], "kaiten-cli-test");
}

#[tokio::test(flavor = "multi_thread")]
async fn board_list_requires_space() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["board", "list"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("specify --space"));
}

#[tokio::test(flavor = "multi_thread")]
async fn board_list_with_flag() {
    let server = MockServer::start().await;
    mock_get(&server, "/spaces/810671/boards", BOARDS).await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["board", "list", "--space", "810671"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1826109"))
        .stdout(predicate::str::contains("test-board"));
}

#[tokio::test(flavor = "multi_thread")]
async fn board_list_uses_default_space_from_config() {
    let server = MockServer::start().await;
    mock_get(&server, "/spaces/810671/boards", BOARDS).await;
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("config.toml"),
        "domain = \"mycompany\"\ntoken = \"file-token\"\n\n[defaults]\nspace = 810671\n",
    )
    .unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["board", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("test-board"));
}

#[tokio::test(flavor = "multi_thread")]
async fn board_view_prints_columns_and_lanes() {
    let server = MockServer::start().await;
    mock_get(&server, "/boards/1826109", BOARD).await;
    let tmp = tempfile::tempdir().unwrap();

    let assert = kaiten(tmp.path(), &server.uri())
        .args(["board", "view", "1826109"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("queued"), "{stdout}");
    assert!(stdout.contains("in progress"), "{stdout}");
    assert!(stdout.contains("done"), "{stdout}");
    insta::assert_snapshot!("board_view", stdout);
}

#[tokio::test(flavor = "multi_thread")]
async fn board_view_json() {
    let server = MockServer::start().await;
    mock_get(&server, "/boards/1826109", BOARD).await;
    let tmp = tempfile::tempdir().unwrap();

    let assert = kaiten(tmp.path(), &server.uri())
        .args(["board", "view", "1826109", "--json"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["id"], 1826109);
    assert_eq!(value["columns"].as_array().unwrap().len(), 3);
    assert_eq!(value["lanes"].as_array().unwrap().len(), 1);
}
```

- [ ] **Step 3: убедиться, что тесты падают**

Run: `cargo test -p kaiten --test space_board_test`
Expected: FAIL — все 7 тестов: заглушки space/board дают exit 1 "not implemented yet"
(тест `board_list_requires_space` падает потому, что stderr содержит "not implemented yet", а не "specify --space").

- [ ] **Step 4: реализация commands/space.rs**

Заменить содержимое файла целиком:

```rust
use kaiten_client::KaitenClient;

use crate::cli::SpaceCmd;
use crate::error::CliError;
use crate::output;

pub async fn run(cmd: SpaceCmd, client: &KaitenClient, json: bool) -> Result<(), CliError> {
    match cmd {
        SpaceCmd::List => {
            let spaces = client.spaces().list().await?;
            if json {
                return output::print_json(&spaces);
            }
            let mut table = output::table(&["ID", "TITLE"]);
            for space in &spaces {
                table.add_row(vec![space.id.to_string(), space.title.clone()]);
            }
            println!("{table}");
            Ok(())
        }
    }
}
```

- [ ] **Step 5: реализация commands/board.rs**

Заменить содержимое файла целиком:

```rust
use kaiten_client::KaitenClient;

use crate::cli::BoardCmd;
use crate::config::Defaults;
use crate::error::CliError;
use crate::output;

pub async fn run(
    cmd: BoardCmd,
    client: &KaitenClient,
    defaults: &Defaults,
    json: bool,
) -> Result<(), CliError> {
    match cmd {
        BoardCmd::List { space } => {
            let space_id = space.or(defaults.space).ok_or_else(|| {
                CliError::InvalidArg("specify --space or set defaults.space in config".into())
            })?;
            let boards = client.boards().list(space_id).await?;
            if json {
                return output::print_json(&boards);
            }
            let mut table = output::table(&["ID", "TITLE"]);
            for board in &boards {
                table.add_row(vec![board.id.to_string(), board.title.clone()]);
            }
            println!("{table}");
            Ok(())
        }
        BoardCmd::View { board_id } => {
            let board = client.boards().get(board_id).await?;
            if json {
                return output::print_json(&board);
            }
            println!("Board {}: {}", board.id, board.title);
            println!();
            println!("Columns:");
            let mut columns = output::table(&["ID", "TITLE", "TYPE"]);
            for column in &board.columns {
                let type_label = match column.column_type {
                    Some(1) => "queued",
                    Some(2) => "in progress",
                    Some(3) => "done",
                    _ => "-",
                };
                columns.add_row(vec![
                    column.id.to_string(),
                    column.title.clone(),
                    type_label.to_string(),
                ]);
            }
            println!("{columns}");
            println!();
            println!("Lanes:");
            let mut lanes = output::table(&["ID", "TITLE"]);
            for lane in &board.lanes {
                lanes.add_row(vec![lane.id.to_string(), lane.title.clone()]);
            }
            println!("{lanes}");
            Ok(())
        }
    }
}
```

- [ ] **Step 6: зафиксировать insta-снапшоты**

Run: `INSTA_UPDATE=always cargo test -p kaiten --test space_board_test`
Expected: PASS, созданы `crates/kaiten/tests/snapshots/space_board_test__space_list.snap`
и `..._board_view.snap`.

Проверить снапшоты глазами (`cat crates/kaiten/tests/snapshots/*.snap`):
- `space_list`: таблица с заголовком `ID`/`TITLE`, строки `810671 kaiten-cli-test` и `810669 Первое пространство`;
- `board_view`: строка `Board 1826109: test-board`; таблица Columns с тремя строками
  `6308511 To Do queued`, `6308512 In Progress in progress`, `6308513 Done done`;
  таблица Lanes со строкой `2293584 Default Lane`; никаких ANSI-кодов.

- [ ] **Step 7: убедиться, что тесты проходят без INSTA_UPDATE**

Run: `cargo test -p kaiten --test space_board_test`
Expected: PASS — 7 тестов. Затем `cargo test -p kaiten` → PASS.

- [ ] **Step 8: линты**

Run: `cargo clippy -p kaiten --all-targets -- -D warnings && cargo fmt --all -- --check`
Expected: чисто.

- [ ] **Step 9: commit**

```
git add crates/kaiten/src/commands/space.rs crates/kaiten/src/commands/board.rs crates/kaiten/tests/space_board_test.rs crates/kaiten/tests/fixtures/spaces_list.json crates/kaiten/tests/fixtures/boards_list.json crates/kaiten/tests/fixtures/board_get.json crates/kaiten/tests/snapshots
git commit -m "feat(cli): space and board commands"
```

### Task 13: card list

Все флаги из §4. Область поиска: явные `--board`/`--space`; без них — `defaults.board` →
`board_id`, иначе `defaults.space` → `space_id`, иначе
`InvalidArg("specify --board/--space or set defaults in config")`. `--mine` → доп. запрос
`users().current()` → `member_ids=[me.id]`. Таблица: `ID, TITLE, COLUMN` (column.title),
`TYPE` (card_type.letter), `ASAP` (`!` или пусто), `UPDATED` (дата до `T`). `--limit`
(default 50) всегда уходит в query.

**Files:**
- Modify: crates/kaiten/src/commands/card.rs — заменить заглушку на match c реализованной веткой List (остальные ветки — заглушки)
- Create: crates/kaiten/tests/fixtures/cards_list.json
- Test: crates/kaiten/tests/card_list_test.rs (+ снапшот card_list)

**Interfaces:**
- Consumes: `client.cards().list(&CardFilter) -> Result<Vec<Card>>`, `client.users().current() -> Result<User>`, `kaiten_client::CardFilter` (поля из §2), `config::Defaults`, `output::{table, print_json}`
- Produces: рабочая `kaiten card list`; каркас `match cmd` в card.rs, который задачи 14–15 и часть 4 дополняют ветками

- [ ] **Step 1: фикстура cards_list.json**

Урезанный реальный `GET /cards` (без description/members/checklists — так отдаёт API;
поля модели + 4 лишних: `sort_order`, `version`, `source`, `uid`). Вторая карточка —
копия первой по форме с `asap: true` и другой колонкой, чтобы покрыть `!` и UPDATED:

```json
[
  {
    "id": 67089469,
    "uid": "c78e313c-ab37-4456-9eb0-904681c4e309",
    "created": "2026-07-09T15:17:59.905Z",
    "updated": "2026-07-09T15:17:59.905Z",
    "archived": false,
    "title": "test card from cli",
    "asap": false,
    "due_date": null,
    "state": 1,
    "condition": 1,
    "board_id": 1826109,
    "column_id": 6308511,
    "lane_id": 2293584,
    "owner_id": 1068514,
    "type_id": 1,
    "comments_total": 0,
    "sort_order": 1.0689198905237203,
    "version": 1,
    "source": "api",
    "column": {
      "id": 6308511,
      "title": "To Do",
      "sort_order": 1,
      "type": 1,
      "board_id": 1826109
    },
    "lane": {
      "id": 2293584,
      "title": "Default Lane",
      "sort_order": 1,
      "board_id": 1826109
    },
    "type": {
      "id": 1,
      "name": "Card",
      "color": 1,
      "letter": "C",
      "archived": false
    },
    "owner": {
      "id": 1068514,
      "uid": "0abd61ea-9dc5-40eb-b0a9-0d452ba1d8a6",
      "full_name": "dxmuser",
      "username": "dxmuser",
      "email": "user@example.com"
    }
  },
  {
    "id": 67089470,
    "uid": "d78e313c-ab37-4456-9eb0-904681c4e310",
    "created": "2026-07-09T16:02:11.001Z",
    "updated": "2026-07-10T09:30:00.123Z",
    "archived": false,
    "title": "urgent bugfix",
    "asap": true,
    "due_date": null,
    "state": 2,
    "condition": 1,
    "board_id": 1826109,
    "column_id": 6308512,
    "lane_id": 2293584,
    "owner_id": 1068514,
    "type_id": 1,
    "comments_total": 2,
    "sort_order": 2.5,
    "version": 4,
    "source": "web",
    "column": {
      "id": 6308512,
      "title": "In Progress",
      "sort_order": 2,
      "type": 2,
      "board_id": 1826109
    },
    "lane": {
      "id": 2293584,
      "title": "Default Lane",
      "sort_order": 1,
      "board_id": 1826109
    },
    "type": {
      "id": 1,
      "name": "Card",
      "color": 1,
      "letter": "C",
      "archived": false
    },
    "owner": {
      "id": 1068514,
      "uid": "0abd61ea-9dc5-40eb-b0a9-0d452ba1d8a6",
      "full_name": "dxmuser",
      "username": "dxmuser",
      "email": "user@example.com"
    }
  }
]
```

- [ ] **Step 2: failing тесты card_list_test.rs**

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const CARDS: &str = include_str!("fixtures/cards_list.json");
const USER_CURRENT: &str = include_str!("fixtures/users_current.json");

fn kaiten(config_dir: &std::path::Path, base_url: &str) -> Command {
    let mut cmd = Command::cargo_bin("kaiten").unwrap();
    cmd.env_remove("KAITEN_TOKEN")
        .env_remove("KAITEN_DOMAIN")
        .env_remove("KAITEN_BASE_URL")
        .env_remove("RUST_LOG")
        .env("KAITEN_CONFIG_DIR", config_dir)
        .env("KAITEN_BASE_URL", base_url)
        .env("KAITEN_TOKEN", "test-token")
        .env("NO_COLOR", "1");
    cmd
}

#[tokio::test(flavor = "multi_thread")]
async fn card_list_uses_board_flag_and_default_limit() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cards"))
        .and(header("Authorization", "Bearer test-token"))
        .and(query_param("board_id", "1826109"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARDS, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    let assert = kaiten(tmp.path(), &server.uri())
        .args(["card", "list", "--board", "1826109"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("67089469"), "{stdout}");
    assert!(stdout.contains("urgent bugfix"), "{stdout}");
    insta::assert_snapshot!("card_list", stdout);
}

#[tokio::test(flavor = "multi_thread")]
async fn card_list_falls_back_to_defaults_board() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cards"))
        .and(query_param("board_id", "1826109"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARDS, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("config.toml"),
        "domain = \"mycompany\"\ntoken = \"file-token\"\n\n[defaults]\nboard = 1826109\n",
    )
    .unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["card", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("test card from cli"));
}

#[tokio::test(flavor = "multi_thread")]
async fn card_list_falls_back_to_defaults_space() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cards"))
        .and(query_param("space_id", "810671"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARDS, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("config.toml"),
        "domain = \"mycompany\"\ntoken = \"file-token\"\n\n[defaults]\nspace = 810671\n",
    )
    .unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["card", "list"])
        .assert()
        .success();
}

#[tokio::test(flavor = "multi_thread")]
async fn card_list_without_scope_is_invalid_arg() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["card", "list"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("specify --board/--space"));
}

#[tokio::test(flavor = "multi_thread")]
async fn card_list_mine_resolves_current_user() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/current"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(USER_CURRENT, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/cards"))
        .and(query_param("board_id", "1826109"))
        .and(query_param("member_ids", "1068514"))
        .and(query_param("limit", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARDS, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["card", "list", "--board", "1826109", "--mine"])
        .assert()
        .success();
}

#[tokio::test(flavor = "multi_thread")]
async fn card_list_passes_all_filters() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cards"))
        .and(query_param("space_id", "810671"))
        .and(query_param("column_id", "6308511"))
        .and(query_param("member_ids", "42"))
        .and(query_param("query", "bug"))
        .and(query_param("tag", "cli-test"))
        .and(query_param("type_id", "1"))
        .and(query_param("archived", "true"))
        .and(query_param("limit", "10"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("[]", "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args([
            "card", "list", "--space", "810671", "--column", "6308511", "--member", "42",
            "--query", "bug", "--tag", "cli-test", "--type", "1", "--archived", "--limit", "10",
        ])
        .assert()
        .success();
}

#[tokio::test(flavor = "multi_thread")]
async fn card_list_json() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/cards"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARDS, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    let assert = kaiten(tmp.path(), &server.uri())
        .args(["card", "list", "--board", "1826109", "--json"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value.as_array().unwrap().len(), 2);
    assert_eq!(value[1]["asap"], true);
}
```

- [ ] **Step 3: убедиться, что тесты падают**

Run: `cargo test -p kaiten --test card_list_test`
Expected: FAIL — все 7 тестов (заглушка card даёт exit 1 "not implemented yet").

- [ ] **Step 4: реализация List в commands/card.rs**

Заменить содержимое файла целиком (List реализован, остальные ветки — заглушки для задач 14–15 и части 4):

```rust
use kaiten_client::{CardFilter, KaitenClient};

use crate::cli::CardCmd;
use crate::config::Defaults;
use crate::error::CliError;
use crate::output;

pub async fn run(
    cmd: CardCmd,
    client: &KaitenClient,
    defaults: &Defaults,
    json: bool,
) -> Result<(), CliError> {
    match cmd {
        CardCmd::List {
            space,
            board,
            column,
            mine,
            member,
            query,
            tag,
            type_id,
            archived,
            limit,
        } => {
            let mut filter = CardFilter {
                limit: Some(limit),
                ..Default::default()
            };
            if board.is_none() && space.is_none() {
                if let Some(b) = defaults.board {
                    filter.board_id = Some(b);
                } else if let Some(s) = defaults.space {
                    filter.space_id = Some(s);
                } else {
                    return Err(CliError::InvalidArg(
                        "specify --board/--space or set defaults in config".into(),
                    ));
                }
            } else {
                filter.board_id = board;
                filter.space_id = space;
            }
            filter.column_id = column;
            filter.query = query;
            filter.tag = tag;
            filter.type_id = type_id;
            if archived {
                filter.archived = Some(true);
            }
            if let Some(member_id) = member {
                filter.member_ids.push(member_id);
            }
            if mine {
                let me = client.users().current().await?;
                filter.member_ids.push(me.id);
            }
            let cards = client.cards().list(&filter).await?;
            if json {
                return output::print_json(&cards);
            }
            let mut table = output::table(&["ID", "TITLE", "COLUMN", "TYPE", "ASAP", "UPDATED"]);
            for card in &cards {
                table.add_row(vec![
                    card.id.to_string(),
                    card.title.clone(),
                    card.column
                        .as_ref()
                        .map(|c| c.title.clone())
                        .unwrap_or_else(|| "-".into()),
                    card.card_type
                        .as_ref()
                        .and_then(|t| t.letter.clone())
                        .unwrap_or_else(|| "-".into()),
                    if card.asap.unwrap_or(false) {
                        "!".into()
                    } else {
                        String::new()
                    },
                    card.updated
                        .as_deref()
                        .and_then(|d| d.split('T').next())
                        .unwrap_or("-")
                        .to_string(),
                ]);
            }
            println!("{table}");
            Ok(())
        }
        CardCmd::View { .. }
        | CardCmd::Create { .. }
        | CardCmd::Edit { .. }
        | CardCmd::Move { .. }
        | CardCmd::Archive { .. } => Err(CliError::InvalidArg("not implemented yet".into())),
        CardCmd::Member(_) | CardCmd::Comment(_) | CardCmd::Checklist(_) | CardCmd::Tag(_) => {
            Err(CliError::InvalidArg("not implemented yet".into()))
        }
    }
}
```

- [ ] **Step 5: зафиксировать снапшот таблицы**

Run: `INSTA_UPDATE=always cargo test -p kaiten --test card_list_test`
Expected: PASS, создан `crates/kaiten/tests/snapshots/card_list_test__card_list.snap`.

Проверить снапшот глазами: заголовки `ID TITLE COLUMN TYPE ASAP UPDATED`; строка
`67089469 test card from cli To Do C` с пустой ASAP и `2026-07-09`; строка
`67089470 urgent bugfix In Progress C !` и `2026-07-10`.

- [ ] **Step 6: убедиться, что тесты проходят**

Run: `cargo test -p kaiten --test card_list_test`
Expected: PASS — 7 тестов. Затем `cargo test -p kaiten` → PASS.

- [ ] **Step 7: линты**

Run: `cargo clippy -p kaiten --all-targets -- -D warnings && cargo fmt --all -- --check`
Expected: чисто.

- [ ] **Step 8: commit**

```
git add crates/kaiten/src/commands/card.rs crates/kaiten/tests/card_list_test.rs crates/kaiten/tests/fixtures/cards_list.json crates/kaiten/tests/snapshots
git commit -m "feat(cli): card list"
```

### Task 14: card view

`parse_card_ref`: число → id; URL → извлечь `card/(\d+)` (реализуется `find("card/")` +
`take_while(is_ascii_digit)` — та же семантика, что regex из контракта, но без нового
крейта: `regex` нет в workspace.dependencies); мусор → `InvalidArg`. Вывод: заголовок
`#id title`, поля board/column/lane/type/owner/members/tags/asap/created/updated,
description как есть, чеклисты с `[x]`/`[ ]`, блок `Properties:` (pretty JSON custom
properties), когда `card.properties` не null. `--comments` — отдельный запрос
`comments().list()`.

**Files:**
- Modify: crates/kaiten/src/commands/card.rs — добавить `parse_card_ref`, `print_card_details`, `user_display`, ветку View; юнит-тесты parse_card_ref в том же файле
- Create: crates/kaiten/tests/fixtures/card_get_full.json, comments_list.json
- Test: crates/kaiten/tests/card_view_test.rs (+ снапшот card_view)

**Interfaces:**
- Consumes: `client.cards().get(card_id: u64) -> Result<Card>`, `client.comments().list(card_id: u64) -> Result<Vec<Comment>>`, модели `Card/Checklist/ChecklistItem/CardMember/CardTag/Comment/User`
- Produces: `commands::card::parse_card_ref(s: &str) -> Result<u64, CliError>` (pub — используют задачи 15 и часть 4), `print_card_details(card: &Card)` (private helper), рабочая `kaiten card view <id|url> [--comments]`

- [ ] **Step 1: юнит-тесты parse_card_ref (failing)**

В конец `crates/kaiten/src/commands/card.rs` добавить:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_numeric_id() {
        assert_eq!(parse_card_ref("67089469").unwrap(), 67089469);
    }

    #[test]
    fn parses_browser_url() {
        let url = "https://mycompany.kaiten.ru/space/810671/boards/card/67089469";
        assert_eq!(parse_card_ref(url).unwrap(), 67089469);
    }

    #[test]
    fn parses_url_with_query_suffix() {
        let url = "https://mycompany.kaiten.ru/space/810671/card/67089469?focus=comments";
        assert_eq!(parse_card_ref(url).unwrap(), 67089469);
    }

    #[test]
    fn garbage_is_invalid_arg() {
        let err = parse_card_ref("definitely-not-a-card").unwrap_err();
        assert!(matches!(err, CliError::InvalidArg(_)));
        assert!(err.to_string().contains("invalid card reference"), "{err}");
    }

    #[test]
    fn url_without_digits_is_invalid_arg() {
        let err = parse_card_ref("https://mycompany.kaiten.ru/card/").unwrap_err();
        assert!(matches!(err, CliError::InvalidArg(_)));
    }
}
```

- [ ] **Step 2: убедиться, что юнит-тесты не компилируются**

Run: `cargo test -p kaiten parse`
Expected: FAIL — `error[E0425]: cannot find function \`parse_card_ref\``.

- [ ] **Step 3: реализовать parse_card_ref**

В `crates/kaiten/src/commands/card.rs` (над `pub async fn run`) добавить:

```rust
/// Accepts a numeric card id or a browser URL containing `card/<id>`.
pub fn parse_card_ref(s: &str) -> Result<u64, CliError> {
    if let Ok(id) = s.parse::<u64>() {
        return Ok(id);
    }
    if let Some(pos) = s.find("card/") {
        let digits: String = s[pos + "card/".len()..]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if let Ok(id) = digits.parse::<u64>() {
            return Ok(id);
        }
    }
    Err(CliError::InvalidArg(format!(
        "invalid card reference `{s}`: expected a numeric id or a card URL"
    )))
}
```

Run: `cargo test -p kaiten parse`
Expected: PASS — 5 юнит-тестов.

- [ ] **Step 4: фикстуры card_get_full.json и comments_list.json**

`crates/kaiten/tests/fixtures/card_get_full.json` — урезанный реальный `GET /cards/{id}`
(полная карточка: description, members, checklists c items, tags; + 5 лишних полей
`version/source/estimate_workload/card_permissions/email`; во второй пункт чеклиста —
`checked: false` для покрытия `[ ]`; `properties` — ненулевой объект `{"id_19": "S"}`
для покрытия блока Properties):

```json
{
  "id": 67089469,
  "uid": "c78e313c-ab37-4456-9eb0-904681c4e309",
  "title": "test card from cli",
  "description": "test **description**",
  "asap": true,
  "archived": false,
  "condition": 1,
  "state": 1,
  "board_id": 1826109,
  "column_id": 6308511,
  "lane_id": 2293584,
  "type_id": 1,
  "owner_id": 1068514,
  "created": "2026-07-09T15:17:59.905Z",
  "updated": "2026-07-09T15:18:07.303Z",
  "due_date": null,
  "comments_total": 1,
  "version": 3,
  "source": "api",
  "estimate_workload": 0,
  "board": {
    "id": 1826109,
    "title": "test-board",
    "uid": "06a433b5-3626-48d4-865b-398312b08c3c"
  },
  "column": {
    "id": 6308511,
    "title": "To Do",
    "type": 1,
    "board_id": 1826109,
    "sort_order": 1
  },
  "lane": {
    "id": 2293584,
    "title": "Default Lane",
    "board_id": 1826109,
    "sort_order": 1,
    "condition": 1
  },
  "type": {
    "id": 1,
    "name": "Card",
    "letter": "C",
    "color": 1,
    "archived": false
  },
  "owner": {
    "id": 1068514,
    "uid": "0abd61ea-9dc5-40eb-b0a9-0d452ba1d8a6",
    "full_name": "dxmuser",
    "username": "dxmuser",
    "email": "user@example.com",
    "activated": true
  },
  "members": [
    {
      "id": 1068514,
      "user_id": 1068514,
      "full_name": "dxmuser",
      "username": "dxmuser",
      "email": "user@example.com",
      "type": 2,
      "card_id": 67089469
    }
  ],
  "tags": [
    {
      "id": 1110772,
      "tag_id": 1110772,
      "name": "cli-test",
      "color": 15,
      "card_id": 67089469
    }
  ],
  "checklists": [
    {
      "id": 11747430,
      "name": "todo",
      "sort_order": 1.1522949931390465,
      "card_id": 67089469,
      "items": [
        {
          "id": 65658564,
          "text": "first item",
          "checked": true,
          "sort_order": 1.7468834610088972,
          "checklist_id": 11747430
        },
        {
          "id": 65658565,
          "text": "second item",
          "checked": false,
          "sort_order": 2.5,
          "checklist_id": 11747430
        }
      ]
    }
  ],
  "properties": {
    "id_19": "S"
  },
  "card_permissions": {
    "read": true,
    "update": true
  },
  "email": "a+co-027702355f2e2fa1-c67089469@a.kaiten.ru"
}
```

`crates/kaiten/tests/fixtures/comments_list.json` — урезанный реальный
`GET /cards/{id}/comments` (+ 4 лишних поля):

```json
[
  {
    "id": 85523991,
    "uid": "ef4cb581-d2fd-4db9-ae85-aa352e27436d",
    "text": "test comment",
    "created": "2026-07-09T15:18:03.341Z",
    "updated": "2026-07-09T15:18:03.341Z",
    "edited": false,
    "type": 1,
    "card_id": 67089469,
    "author_id": 1068514,
    "internal": false,
    "author": {
      "id": 1068514,
      "uid": "0abd61ea-9dc5-40eb-b0a9-0d452ba1d8a6",
      "full_name": "dxmuser",
      "username": "dxmuser",
      "email": "user@example.com",
      "activated": true
    }
  }
]
```

- [ ] **Step 5: failing тесты card_view_test.rs**

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const CARD: &str = include_str!("fixtures/card_get_full.json");
const COMMENTS: &str = include_str!("fixtures/comments_list.json");

fn kaiten(config_dir: &std::path::Path, base_url: &str) -> Command {
    let mut cmd = Command::cargo_bin("kaiten").unwrap();
    cmd.env_remove("KAITEN_TOKEN")
        .env_remove("KAITEN_DOMAIN")
        .env_remove("KAITEN_BASE_URL")
        .env_remove("RUST_LOG")
        .env("KAITEN_CONFIG_DIR", config_dir)
        .env("KAITEN_BASE_URL", base_url)
        .env("KAITEN_TOKEN", "test-token")
        .env("NO_COLOR", "1");
    cmd
}

async fn mock_card(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/cards/67089469"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARD, "application/json"))
        .expect(1)
        .mount(server)
        .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn card_view_by_id_prints_details() {
    let server = MockServer::start().await;
    mock_card(&server).await;
    let tmp = tempfile::tempdir().unwrap();

    let assert = kaiten(tmp.path(), &server.uri())
        .args(["card", "view", "67089469"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("[x] first item"), "{stdout}");
    assert!(stdout.contains("[ ] second item"), "{stdout}");
    assert!(stdout.contains("test **description**"), "{stdout}");
    assert!(stdout.contains("Properties:"), "{stdout}");
    assert!(stdout.contains("\"id_19\": \"S\""), "{stdout}");
    insta::assert_snapshot!("card_view", stdout);
}

#[tokio::test(flavor = "multi_thread")]
async fn card_view_by_url() {
    let server = MockServer::start().await;
    mock_card(&server).await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args([
            "card",
            "view",
            "https://mycompany.kaiten.ru/space/810671/card/67089469",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("#67089469 test card from cli"));
}

#[tokio::test(flavor = "multi_thread")]
async fn card_view_with_comments_makes_second_request() {
    let server = MockServer::start().await;
    mock_card(&server).await;
    Mock::given(method("GET"))
        .and(path("/cards/67089469/comments"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(COMMENTS, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["card", "view", "67089469", "--comments"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Comments:"))
        .stdout(predicate::str::contains("test comment"))
        .stdout(predicate::str::contains("2026-07-09 dxmuser:"));
}

#[tokio::test(flavor = "multi_thread")]
async fn card_view_json() {
    let server = MockServer::start().await;
    mock_card(&server).await;
    let tmp = tempfile::tempdir().unwrap();

    let assert = kaiten(tmp.path(), &server.uri())
        .args(["card", "view", "67089469", "--json"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["id"], 67089469);
    assert_eq!(value["checklists"][0]["items"][0]["checked"], true);
    assert_eq!(value["properties"]["id_19"], "S");
}

#[tokio::test(flavor = "multi_thread")]
async fn card_view_garbage_ref_fails() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["card", "view", "definitely-not-a-card"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("invalid card reference"));
}
```

Run: `cargo test -p kaiten --test card_view_test`
Expected: FAIL — первые 4 теста (ветка View — заглушка, exit 1 "not implemented yet");
`card_view_garbage_ref_fails` может уже проходить — `parse_card_ref` пока не вызывается,
поэтому падение остальных обязательно проверить.

- [ ] **Step 6: реализация ветки View и print_card_details**

В `crates/kaiten/src/commands/card.rs`:

1. Заменить ветку-заглушку

```rust
        CardCmd::View { .. }
        | CardCmd::Create { .. }
        | CardCmd::Edit { .. }
        | CardCmd::Move { .. }
        | CardCmd::Archive { .. } => Err(CliError::InvalidArg("not implemented yet".into())),
```

на:

```rust
        CardCmd::View { card, comments } => {
            let card_id = parse_card_ref(&card)?;
            let card = client.cards().get(card_id).await?;
            if json {
                if comments {
                    let list = client.comments().list(card_id).await?;
                    return output::print_json(&serde_json::json!({
                        "card": card,
                        "comments": list,
                    }));
                }
                return output::print_json(&card);
            }
            print_card_details(&card);
            if comments {
                let list = client.comments().list(card_id).await?;
                println!();
                println!("Comments:");
                for comment in &list {
                    let author = comment
                        .author
                        .as_ref()
                        .map(user_display)
                        .unwrap_or_else(|| "-".into());
                    let date = comment
                        .created
                        .as_deref()
                        .and_then(|d| d.split('T').next())
                        .unwrap_or("-");
                    println!("{date} {author}:");
                    println!("{}", comment.text);
                }
            }
            Ok(())
        }
        CardCmd::Create { .. }
        | CardCmd::Edit { .. }
        | CardCmd::Move { .. }
        | CardCmd::Archive { .. } => Err(CliError::InvalidArg("not implemented yet".into())),
```

2. После `parse_card_ref` добавить хелперы:

```rust
fn user_display(user: &kaiten_client::User) -> String {
    user.username
        .clone()
        .or_else(|| user.full_name.clone())
        .unwrap_or_else(|| user.id.to_string())
}

fn print_card_details(card: &kaiten_client::Card) {
    println!("#{} {}", card.id, card.title);
    println!();
    let dash = || "-".to_string();
    println!(
        "board:   {}",
        card.board
            .as_ref()
            .map(|b| format!("{} ({})", b.title, b.id))
            .unwrap_or_else(dash)
    );
    println!(
        "column:  {}",
        card.column
            .as_ref()
            .map(|c| format!("{} ({})", c.title, c.id))
            .unwrap_or_else(dash)
    );
    println!(
        "lane:    {}",
        card.lane
            .as_ref()
            .map(|l| format!("{} ({})", l.title, l.id))
            .unwrap_or_else(dash)
    );
    println!(
        "type:    {}",
        card.card_type
            .as_ref()
            .map(|t| t.name.clone())
            .unwrap_or_else(dash)
    );
    println!(
        "owner:   {}",
        card.owner.as_ref().map(user_display).unwrap_or_else(dash)
    );
    let members = card
        .members
        .iter()
        .map(|m| {
            m.username
                .clone()
                .or_else(|| m.full_name.clone())
                .unwrap_or_else(|| m.id.to_string())
        })
        .collect::<Vec<_>>()
        .join(", ");
    println!("members: {}", if members.is_empty() { dash() } else { members });
    let tags = card
        .tags
        .iter()
        .map(|t| t.name.clone())
        .collect::<Vec<_>>()
        .join(", ");
    println!("tags:    {}", if tags.is_empty() { dash() } else { tags });
    println!("asap:    {}", if card.asap.unwrap_or(false) { "yes" } else { "no" });
    println!(
        "created: {}",
        card.created
            .as_deref()
            .and_then(|d| d.split('T').next())
            .unwrap_or("-")
    );
    println!(
        "updated: {}",
        card.updated
            .as_deref()
            .and_then(|d| d.split('T').next())
            .unwrap_or("-")
    );
    if let Some(description) = &card.description {
        println!();
        println!("Description:");
        println!("{description}");
    }
    if !card.checklists.is_empty() {
        println!();
        println!("Checklists:");
        for checklist in &card.checklists {
            println!("{} ({})", checklist.name, checklist.id);
            for item in &checklist.items {
                let mark = if item.checked.unwrap_or(false) { "x" } else { " " };
                println!("  [{mark}] {} ({})", item.text, item.id);
            }
        }
    }
    if let Some(properties) = card.properties.as_ref().filter(|p| !p.is_null()) {
        println!();
        println!("Properties:");
        println!(
            "{}",
            serde_json::to_string_pretty(properties).unwrap_or_else(|_| properties.to_string())
        );
    }
}
```

- [ ] **Step 7: зафиксировать снапшот**

Run: `INSTA_UPDATE=always cargo test -p kaiten --test card_view_test`
Expected: PASS, создан `crates/kaiten/tests/snapshots/card_view_test__card_view.snap`.

Проверить снапшот глазами: `#67089469 test card from cli`; поля
`board: test-board (1826109)`, `column: To Do (6308511)`, `lane: Default Lane (2293584)`,
`type: Card`, `owner: dxmuser`, `members: dxmuser`, `tags: cli-test`, `asap: yes`;
блок `Description:` с `test **description**` как есть; блок `Checklists:` c
`todo (11747430)`, `[x] first item (65658564)`, `[ ] second item (65658565)`;
блок `Properties:` с pretty JSON из двух строк-скобок и `"id_19": "S"` между ними.

- [ ] **Step 8: убедиться, что всё проходит**

Run: `cargo test -p kaiten --test card_view_test && cargo test -p kaiten`
Expected: PASS — 5 тестов card_view + весь остальной прогон зелёный.

- [ ] **Step 9: линты**

Run: `cargo clippy -p kaiten --all-targets -- -D warnings && cargo fmt --all -- --check`
Expected: чисто.

- [ ] **Step 10: commit**

```
git add crates/kaiten/src/commands/card.rs crates/kaiten/tests/card_view_test.rs crates/kaiten/tests/fixtures/card_get_full.json crates/kaiten/tests/fixtures/comments_list.json crates/kaiten/tests/snapshots
git commit -m "feat(cli): card view"
```

### Task 15: card create / edit / move / archive

create: `--board` или `defaults.board`, иначе `InvalidArg`. edit: `--asap true|false` →
`Option<bool>` (клап-поле уже объявлено в Task 10); пустой edit без флагов — `InvalidArg`.
move: `--column` обязателен (u64 в clap). archive: `UpdateCard { condition: Some(2) }`.
После каждой мутации печатается карточка: таблица ключ-значение (или `--json`). Тесты
проверяют ТЕЛО отправленных POST/PATCH: `body_partial_json` для create/edit, точный
`body_json` для move/archive — он заодно доказывает, что `skip_serializing_if` не шлёт
лишние поля. Плюс негативный кейс: 400 от `POST /cards` → exit 1, в stderr и
человекочитаемое `message`, и сырое тело ответа (`body` печатает main.rs из Task 10).

**Files:**
- Modify: crates/kaiten/src/commands/card.rs — ветки Create/Edit/Move/Archive + `print_card_kv`
- Create: crates/kaiten/tests/fixtures/card_create.json, card_update.json, card_archive.json
- Test: crates/kaiten/tests/card_mutate_test.rs

**Interfaces:**
- Consumes: `client.cards().create(&CreateCard) -> Result<Card>`, `client.cards().update(card_id: u64, &UpdateCard) -> Result<Card>`, `kaiten_client::{CreateCard, UpdateCard}` (поля из §2), `parse_card_ref` (Task 14), `config::Defaults`, `output::{table, print_json}`
- Produces: рабочие `kaiten card create/edit/move/archive`; `print_card_kv(card: &Card)` (private helper для мутаций)

- [ ] **Step 1: фикстуры ответов мутаций**

`crates/kaiten/tests/fixtures/card_create.json` — урезанный реальный ответ `POST /cards`
(title подогнан под тест; + 5 лишних полей):

```json
{
  "id": 67089469,
  "uid": "c78e313c-ab37-4456-9eb0-904681c4e309",
  "created": "2026-07-09T15:17:59.905Z",
  "updated": "2026-07-09T15:17:59.905Z",
  "archived": false,
  "title": "new card",
  "asap": false,
  "state": 1,
  "condition": 1,
  "board_id": 1826109,
  "column_id": 6308511,
  "lane_id": 2293584,
  "owner_id": 1068514,
  "type_id": 1,
  "comments_total": 0,
  "description": null,
  "due_date": null,
  "version": 1,
  "source": "api",
  "sort_order": 1.0689198905237203,
  "estimate_workload": 0,
  "type": {
    "id": 1,
    "name": "Card",
    "letter": "C",
    "color": 1,
    "archived": false
  },
  "owner": {
    "id": 1068514,
    "uid": "0abd61ea-9dc5-40eb-b0a9-0d452ba1d8a6",
    "full_name": "dxmuser",
    "username": "dxmuser",
    "email": "user@example.com"
  },
  "checklists": [],
  "external_links": []
}
```

`crates/kaiten/tests/fixtures/card_update.json` — урезанный реальный ответ `PATCH /cards/{id}`
(asap true, description задан; + 4 лишних поля):

```json
{
  "id": 67089469,
  "uid": "c78e313c-ab37-4456-9eb0-904681c4e309",
  "created": "2026-07-09T15:17:59.905Z",
  "updated": "2026-07-09T15:18:02.423Z",
  "archived": false,
  "title": "test card from cli",
  "description": "test **description**",
  "asap": true,
  "state": 1,
  "condition": 1,
  "board_id": 1826109,
  "column_id": 6308511,
  "lane_id": 2293584,
  "owner_id": 1068514,
  "type_id": 1,
  "comments_total": 0,
  "due_date": null,
  "version": 2,
  "source": "api",
  "sort_order": 1.0689198905237203,
  "estimate_workload": 0,
  "owner": {
    "id": 1068514,
    "uid": "0abd61ea-9dc5-40eb-b0a9-0d452ba1d8a6",
    "full_name": "dxmuser",
    "username": "dxmuser",
    "email": "user@example.com"
  }
}
```

`crates/kaiten/tests/fixtures/card_archive.json` — тот же ответ PATCH, но карточка
заархивирована (`condition: 2`, `archived: true`):

```json
{
  "id": 67089469,
  "uid": "c78e313c-ab37-4456-9eb0-904681c4e309",
  "created": "2026-07-09T15:17:59.905Z",
  "updated": "2026-07-09T15:19:00.000Z",
  "archived": true,
  "title": "test card from cli",
  "description": "test **description**",
  "asap": false,
  "state": 1,
  "condition": 2,
  "board_id": 1826109,
  "column_id": 6308511,
  "lane_id": 2293584,
  "owner_id": 1068514,
  "type_id": 1,
  "comments_total": 0,
  "due_date": null,
  "version": 3,
  "source": "api",
  "sort_order": 1.0689198905237203,
  "estimate_workload": 0
}
```

- [ ] **Step 2: failing тесты card_mutate_test.rs**

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::json;
use wiremock::matchers::{body_json, body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const CARD_CREATE: &str = include_str!("fixtures/card_create.json");
const CARD_UPDATE: &str = include_str!("fixtures/card_update.json");
const CARD_ARCHIVE: &str = include_str!("fixtures/card_archive.json");

fn kaiten(config_dir: &std::path::Path, base_url: &str) -> Command {
    let mut cmd = Command::cargo_bin("kaiten").unwrap();
    cmd.env_remove("KAITEN_TOKEN")
        .env_remove("KAITEN_DOMAIN")
        .env_remove("KAITEN_BASE_URL")
        .env_remove("RUST_LOG")
        .env("KAITEN_CONFIG_DIR", config_dir)
        .env("KAITEN_BASE_URL", base_url)
        .env("KAITEN_TOKEN", "test-token")
        .env("NO_COLOR", "1");
    cmd
}

#[tokio::test(flavor = "multi_thread")]
async fn create_sends_board_and_title() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cards"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_partial_json(json!({
            "board_id": 1826109,
            "title": "new card"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARD_CREATE, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["card", "create", "--board", "1826109", "--title", "new card"])
        .assert()
        .success()
        .stdout(predicate::str::contains("67089469"))
        .stdout(predicate::str::contains("new card"));
}

#[tokio::test(flavor = "multi_thread")]
async fn create_sends_optional_fields() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cards"))
        .and(body_partial_json(json!({
            "board_id": 1826109,
            "title": "new card",
            "column_id": 6308511,
            "lane_id": 2293584,
            "description": "body",
            "type_id": 1,
            "asap": true
        })))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARD_CREATE, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args([
            "card", "create", "--board", "1826109", "--title", "new card", "--column",
            "6308511", "--lane", "2293584", "--description", "body", "--type", "1", "--asap",
        ])
        .assert()
        .success();
}

#[tokio::test(flavor = "multi_thread")]
async fn create_uses_defaults_board() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cards"))
        .and(body_partial_json(json!({"board_id": 1826109, "title": "new card"})))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARD_CREATE, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("config.toml"),
        "domain = \"mycompany\"\ntoken = \"file-token\"\n\n[defaults]\nboard = 1826109\n",
    )
    .unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["card", "create", "--title", "new card"])
        .assert()
        .success();
}

#[tokio::test(flavor = "multi_thread")]
async fn create_without_board_or_defaults_fails() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["card", "create", "--title", "new card"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("specify --board"));
}

#[tokio::test(flavor = "multi_thread")]
async fn edit_sends_patch_body() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/cards/67089469"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_partial_json(json!({
            "asap": true,
            "description": "test **description**"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARD_UPDATE, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args([
            "card",
            "edit",
            "67089469",
            "--asap",
            "true",
            "--description",
            "test **description**",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("67089469"));
}

#[tokio::test(flavor = "multi_thread")]
async fn edit_asap_false_is_sent_explicitly() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/cards/67089469"))
        .and(body_json(json!({"asap": false})))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARD_UPDATE, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["card", "edit", "67089469", "--asap", "false"])
        .assert()
        .success();
}

#[tokio::test(flavor = "multi_thread")]
async fn edit_without_changes_fails() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["card", "edit", "67089469"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("nothing to edit"));
}

#[tokio::test(flavor = "multi_thread")]
async fn move_sends_exactly_column_id() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/cards/67089469"))
        .and(body_json(json!({"column_id": 6308512})))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARD_UPDATE, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["card", "move", "67089469", "--column", "6308512"])
        .assert()
        .success();
}

#[tokio::test(flavor = "multi_thread")]
async fn move_with_lane_and_board() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/cards/67089469"))
        .and(body_json(json!({
            "column_id": 6308512,
            "lane_id": 2293584,
            "board_id": 1826109
        })))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARD_UPDATE, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args([
            "card", "move", "67089469", "--column", "6308512", "--lane", "2293584", "--board",
            "1826109",
        ])
        .assert()
        .success();
}

#[tokio::test(flavor = "multi_thread")]
async fn archive_sends_condition_2() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/cards/67089469"))
        .and(body_json(json!({"condition": 2})))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARD_ARCHIVE, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["card", "archive", "67089469"])
        .assert()
        .success()
        .stdout(predicate::str::contains("67089469"));
}

#[tokio::test(flavor = "multi_thread")]
async fn create_json_output() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/cards"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(CARD_CREATE, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let tmp = tempfile::tempdir().unwrap();

    let assert = kaiten(tmp.path(), &server.uri())
        .args([
            "card", "create", "--board", "1826109", "--title", "new card", "--json",
        ])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["id"], 67089469);
    assert_eq!(value["title"], "new card");
}

#[tokio::test(flavor = "multi_thread")]
async fn create_api_error_prints_message_and_body() {
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
    let tmp = tempfile::tempdir().unwrap();

    kaiten(tmp.path(), &server.uri())
        .args(["card", "create", "--board", "1826109", "--title", "new card"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("400"))
        .stderr(predicate::str::contains(
            "Card should have required property 'board_id'",
        ))
        .stderr(predicate::str::contains(
            r#"{"message":"Card should have required property 'board_id'"}"#,
        ));
}
```

- [ ] **Step 3: убедиться, что тесты падают**

Run: `cargo test -p kaiten --test card_mutate_test`
Expected: FAIL — все 12 тестов: ветки Create/Edit/Move/Archive — заглушки, exit 1
"not implemented yet"; `create_without_board_or_defaults_fails`,
`edit_without_changes_fails` и `create_api_error_prints_message_and_body` падают на
ассертах stderr — там "not implemented yet" вместо ожидаемых сообщений.

- [ ] **Step 4: реализация веток Create/Edit/Move/Archive**

В `crates/kaiten/src/commands/card.rs`:

1. В `use kaiten_client::...` добавить типы запросов:

```rust
use kaiten_client::{CardFilter, CreateCard, KaitenClient, UpdateCard};
```

2. Заменить ветку-заглушку

```rust
        CardCmd::Create { .. }
        | CardCmd::Edit { .. }
        | CardCmd::Move { .. }
        | CardCmd::Archive { .. } => Err(CliError::InvalidArg("not implemented yet".into())),
```

на:

```rust
        CardCmd::Create {
            title,
            board,
            column,
            lane,
            description,
            type_id,
            asap,
        } => {
            let board_id = board.or(defaults.board).ok_or_else(|| {
                CliError::InvalidArg("specify --board or set defaults.board in config".into())
            })?;
            let req = CreateCard {
                board_id,
                title,
                column_id: column,
                lane_id: lane,
                description,
                type_id,
                asap: if asap { Some(true) } else { None },
            };
            let card = client.cards().create(&req).await?;
            if json {
                return output::print_json(&card);
            }
            print_card_kv(&card);
            Ok(())
        }
        CardCmd::Edit {
            card,
            title,
            description,
            type_id,
            asap,
        } => {
            let card_id = parse_card_ref(&card)?;
            if title.is_none() && description.is_none() && type_id.is_none() && asap.is_none() {
                return Err(CliError::InvalidArg(
                    "nothing to edit: pass --title/--description/--type/--asap".into(),
                ));
            }
            let req = UpdateCard {
                title,
                description,
                type_id,
                asap,
                ..Default::default()
            };
            let card = client.cards().update(card_id, &req).await?;
            if json {
                return output::print_json(&card);
            }
            print_card_kv(&card);
            Ok(())
        }
        CardCmd::Move {
            card,
            column,
            lane,
            board,
        } => {
            let card_id = parse_card_ref(&card)?;
            let req = UpdateCard {
                column_id: Some(column),
                lane_id: lane,
                board_id: board,
                ..Default::default()
            };
            let card = client.cards().update(card_id, &req).await?;
            if json {
                return output::print_json(&card);
            }
            print_card_kv(&card);
            Ok(())
        }
        CardCmd::Archive { card } => {
            let card_id = parse_card_ref(&card)?;
            let req = UpdateCard {
                condition: Some(2),
                ..Default::default()
            };
            let card = client.cards().update(card_id, &req).await?;
            if json {
                return output::print_json(&card);
            }
            print_card_kv(&card);
            Ok(())
        }
```

3. Рядом с `print_card_details` добавить:

```rust
fn print_card_kv(card: &kaiten_client::Card) {
    let dash = || "-".to_string();
    let mut table = output::table(&["FIELD", "VALUE"]);
    table.add_row(vec!["id".to_string(), card.id.to_string()]);
    table.add_row(vec!["title".to_string(), card.title.clone()]);
    table.add_row(vec![
        "board".to_string(),
        card.board_id.map(|v| v.to_string()).unwrap_or_else(dash),
    ]);
    table.add_row(vec![
        "column".to_string(),
        card.column_id.map(|v| v.to_string()).unwrap_or_else(dash),
    ]);
    table.add_row(vec![
        "lane".to_string(),
        card.lane_id.map(|v| v.to_string()).unwrap_or_else(dash),
    ]);
    table.add_row(vec![
        "type".to_string(),
        card.type_id.map(|v| v.to_string()).unwrap_or_else(dash),
    ]);
    table.add_row(vec![
        "asap".to_string(),
        card.asap.map(|v| v.to_string()).unwrap_or_else(dash),
    ]);
    table.add_row(vec![
        "condition".to_string(),
        card.condition.map(|v| v.to_string()).unwrap_or_else(dash),
    ]);
    table.add_row(vec![
        "updated".to_string(),
        card.updated
            .as_deref()
            .and_then(|d| d.split('T').next())
            .unwrap_or("-")
            .to_string(),
    ]);
    println!("{table}");
}
```

- [ ] **Step 5: убедиться, что тесты проходят**

Run: `cargo test -p kaiten --test card_mutate_test`
Expected: PASS — 12 тестов (включая `create_api_error_prints_message_and_body` — main.rs
из Task 10 уже печатает body отдельной строкой, отдельной реализации не нужно).
Затем `cargo test -p kaiten` → PASS (весь крейт), `cargo test` → PASS (workspace целиком).

- [ ] **Step 6: линты**

Run: `cargo clippy -p kaiten --all-targets -- -D warnings && cargo fmt --all -- --check`
Expected: чисто.

- [ ] **Step 7: commit**

```
git add crates/kaiten/src/commands/card.rs crates/kaiten/tests/card_mutate_test.rs crates/kaiten/tests/fixtures/card_create.json crates/kaiten/tests/fixtures/card_update.json crates/kaiten/tests/fixtures/card_archive.json
git commit -m "feat(cli): card create/edit/move/archive"
```


<!-- Part 4: Tasks 16-20 (CLI, часть 2) -->

## Контекст части 4

Tasks 1-15 считаются выполненными. Эта часть опирается на них так:

- `kaiten-client` полностью реализован по INTERFACES.md §2 (фасады `users()`, `cards()`, `comments()`, `checklists()`, `tags()`, `members()`, метод `raw()`).
- CLI-скелет создан Task 10 (part3): `crates/kaiten/src/cli.rs` объявляет полное clap-дерево
  из §4 — enum-ы `Commands`, `AuthCmd`, `SpaceCmd`, `BoardCmd`, `CardCmd`, `CardMemberCmd`,
  `CardCommentCmd`, `CardChecklistCmd`, `CardChecklistItemCmd`, `CardTagCmd`, `TagCmd`,
  `CardTypeCmd`, `McpCmd`, `Shell`; варианты `Commands::Api { method: String, path: String,
  data: Option<String> }` и `Commands::Completion { shell: Shell }`.
- Архитектура команд (Task 10): каждый модуль `commands/*.rs` экспортирует РОВНО ОДНУ точку
  входа `pub async fn run(...)` (у `completion` — синхронную `pub fn run(shell: Shell)`)
  с `match` по сабкоманде внутри. Отдельных публичных функций на каждую подкоманду НЕ
  существует. Нереализованные ветки match и заглушки целых модулей возвращают
  `Err(CliError::InvalidArg("not implemented yet".into()))`.
- `main.rs` (Task 10) уже диспатчит все подкоманды в `run()` модулей и создаёт
  `KaitenClient` из `config::resolve()`; `Commands::Completion { shell }` обрабатывается
  ПЕРВОЙ веткой match — до резолва конфига и создания клиента. Задачи 16-20 НЕ меняют
  `main.rs` и `cli.rs`.
- После задач 13-15 в `crates/kaiten/src/commands/card.rs` реализованы ветки
  `List`/`View`/`Create`/`Edit`/`Move`/`Archive`, объявлены `pub fn parse_card_ref`
  (Task 14) и приватные хелперы `user_display`, `print_card_details` (Task 14),
  `print_card_kv` (Task 15); в конце файла — `#[cfg(test)] mod tests` с юнит-тестами
  `parse_card_ref`. Единственная оставшаяся ветка-заглушка в `run()`:

  ```rust
          CardCmd::Member(_) | CardCmd::Comment(_) | CardCmd::Checklist(_) | CardCmd::Tag(_) => {
              Err(CliError::InvalidArg("not implemented yet".into()))
          }
  ```

  Задачи 16-18 последовательно вынимают из этой ветки варианты и заменяют реализацией
  (вспомогательные приватные `fn`/`async fn` в том же файле разрешены).
- Все CLI-тесты: `assert_cmd` + `wiremock` + env `KAITEN_BASE_URL={mock.uri()}`,
  `KAITEN_TOKEN=test-token`, `KAITEN_CONFIG_DIR={tempdir}`, `NO_COLOR=1` (§6).
  Тестам нужен работающий async-runtime параллельно с блокирующим запуском бинарника,
  поэтому везде `#[tokio::test(flavor = "multi_thread")]`.
- Фикстуры CLI-тестов лежат в `crates/kaiten/tests/fixtures/` и названы по фиче
  (`member_*`, `checklist_*`, ...), чтобы не конфликтовать с фикстурами задач 12-15.
  Все фикстуры — урезанные реальные ответы API (все поля, которые парсит модель,
  плюс 3-5 лишних полей для проверки толерантности; email заменён на `user@example.com`).

Зависимости бинарника (включая `reqwest` в `[dependencies]` и dev-dependencies `wiremock`,
`assert_cmd`, `predicates`, `insta`, `tempfile`) объявлены в `crates/kaiten/Cargo.toml`
задачей 10 (Step 1); `tokio` и `serde_json` доступны интеграционным тестам из обычных
`[dependencies]`. Перед Task 16 ничего добавлять не нужно; проверка:

Run: `grep -A7 'dev-dependencies' crates/kaiten/Cargo.toml`
Expected: строки `wiremock`/`assert_cmd`/`predicates`/`insta`/`tempfile` c `{ workspace = true }`.

---

### Task 16: card member add/remove + card comment add/list

**Files:**
- Create: crates/kaiten/tests/fixtures/member_users.json
- Create: crates/kaiten/tests/fixtures/member_added.json
- Create: crates/kaiten/tests/fixtures/comment_created.json
- Create: crates/kaiten/tests/fixtures/comments_list_two.json
- Modify: crates/kaiten/src/commands/card.rs — заменить ветку-заглушку match внутри `run()` на реализованные `CardCmd::Member(...)` и `CardCmd::Comment(...)`; добавить приватные хелперы `resolve_user`, `truncate_text`, `date_cell`
- Test: crates/kaiten/tests/card_member_test.rs
- Test: crates/kaiten/tests/card_comment_test.rs

**Interfaces:**
- Consumes:
  - `kaiten_client::KaitenClient` и фасады (§2):
    `client.users().list() -> Result<Vec<User>, KaitenError>`,
    `client.members().add(card_id: u64, user_id: u64) -> Result<CardMember, KaitenError>`,
    `client.members().remove(card_id: u64, user_id: u64) -> Result<(), KaitenError>`,
    `client.comments().list(card_id: u64) -> Result<Vec<Comment>, KaitenError>`,
    `client.comments().add(card_id: u64, text: &str) -> Result<Comment, KaitenError>`
  - `commands::card::parse_card_ref(s: &str) -> Result<u64, CliError>` (Task 14)
  - `crate::output::print_json<T: serde::Serialize>(&T) -> Result<(), CliError>`,
    `crate::output::table(headers: &[&str]) -> comfy_table::Table` (Task 10);
    в `card.rs` модуль уже импортирован строкой `use crate::output;` (Task 13)
  - clap-структуры из `cli.rs` (Task 10, дословно):
    ```rust
    #[derive(Subcommand)]
    pub enum CardMemberCmd {
        /// Add member (user id or email)
        Add { card: String, user: String },
        /// Remove member (user id or email)
        Remove { card: String, user: String },
    }

    #[derive(Subcommand)]
    pub enum CardCommentCmd {
        /// Add a comment
        Add {
            card: String,
            #[arg(long)]
            body: String,
        },
        /// List comments
        List { card: String },
    }
    ```
  - точка входа в `commands/card.rs` (Task 10; сигнатура НЕ меняется):
    ```rust
    pub async fn run(
        cmd: CardCmd,
        client: &KaitenClient,
        defaults: &Defaults,
        json: bool,
    ) -> Result<(), CliError>
    ```
    и заменяемая ветка-заглушка её match (состояние после Task 15):
    ```rust
            CardCmd::Member(_) | CardCmd::Comment(_) | CardCmd::Checklist(_) | CardCmd::Tag(_) => {
                Err(CliError::InvalidArg("not implemented yet".into()))
            }
    ```
- Produces:
  - реализованные команды `kaiten card member add|remove`, `kaiten card comment add|list`
    (ветки `CardCmd::Member(...)` и `CardCmd::Comment(...)` внутри `commands::card::run`)
  - приватные хелперы в `commands/card.rs`:
    `async fn resolve_user(client: &KaitenClient, user: &str) -> Result<u64, CliError>`,
    `fn truncate_text(s: &str, max: usize) -> String`,
    `fn date_cell(value: Option<&str>) -> String`

Семантика аргумента `<user>`: строка целиком парсится как `u64` — это id; содержит `@` —
это email, резолвится через `users().list()` и точное совпадение по полю `email`;
иначе — `CliError::InvalidArg`. Email не найден — тоже `InvalidArg`.

`card comment list` печатает таблицу `ID, AUTHOR, CREATED, TEXT`: AUTHOR — `author.username`
(нет — `-`), CREATED — дата до `T`, TEXT — обрезка до 60 символов (по `char`) с `…` в конце,
если текст длиннее. `card comment add` печатает id созданного комментария (одно число).
`card member remove` при `--json` печатает `{"removed": true, "user_id": N}` (DELETE не
возвращает полезного тела, поэтому CLI формирует объект сам).

- [ ] **Step 1: Создать фикстуры для member-тестов**

Файл `crates/kaiten/tests/fixtures/member_users.json` (урезанный ответ `GET /users`; второй
пользователь добавлен для осмысленного поиска по email; лишние поля: `lng`, `timezone`,
`theme`, `role`, `company_id`):

```json
[
  {
    "id": 1068514,
    "uid": "0abd61ea-9dc5-40eb-b0a9-0d452ba1d8a6",
    "full_name": "dxmuser",
    "username": "dxmuser",
    "email": "user@example.com",
    "activated": true,
    "lng": "ru",
    "timezone": "UTC",
    "theme": "auto",
    "role": 1,
    "company_id": 398610
  },
  {
    "id": 555001,
    "uid": "7f9b2c31-56cd-4e0a-8f21-3d9a1b2c3d4e",
    "full_name": "Second User",
    "username": "seconduser",
    "email": "second@example.com",
    "activated": true,
    "lng": "en",
    "timezone": "UTC",
    "theme": "auto",
    "role": 2,
    "company_id": 398610
  }
]
```

Файл `crates/kaiten/tests/fixtures/member_added.json` (урезанный ответ
`POST /cards/{id}/members`; лишние поля: `uid`, `activated`, `lng`, `timezone`):

```json
{
  "id": 1068514,
  "uid": "0abd61ea-9dc5-40eb-b0a9-0d452ba1d8a6",
  "full_name": "dxmuser",
  "username": "dxmuser",
  "email": "user@example.com",
  "type": 1,
  "user_id": 1068514,
  "card_id": 67089469,
  "activated": true,
  "lng": "ru",
  "timezone": "UTC"
}
```

- [ ] **Step 2: Написать failing-тесты card member**

Файл `crates/kaiten/tests/card_member_test.rs` целиком:

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn kaiten(base_url: &str, config_dir: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("kaiten").unwrap();
    cmd.env("KAITEN_BASE_URL", base_url)
        .env("KAITEN_TOKEN", "test-token")
        .env("KAITEN_CONFIG_DIR", config_dir)
        .env("NO_COLOR", "1");
    cmd
}

#[tokio::test(flavor = "multi_thread")]
async fn member_add_by_id_posts_user_id() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("POST"))
        .and(path("/cards/67089469/members"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_json(serde_json::json!({"user_id": 1068514})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/member_added.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["card", "member", "add", "67089469", "1068514"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "added user 1068514 to card 67089469",
        ));
}

#[tokio::test(flavor = "multi_thread")]
async fn member_add_by_email_resolves_via_users_list() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/users"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/member_users.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/cards/67089469/members"))
        .and(body_json(serde_json::json!({"user_id": 555001})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/member_added.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    // card задан URL-ом — заодно проверяем parse_card_ref
    kaiten(&server.uri(), tmp.path())
        .args([
            "card",
            "member",
            "add",
            "https://mycompany.kaiten.ru/space/810671/card/67089469",
            "second@example.com",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "added user 555001 to card 67089469",
        ));
}

#[tokio::test(flavor = "multi_thread")]
async fn member_add_json_prints_model() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("POST"))
        .and(path("/cards/67089469/members"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/member_added.json")),
        )
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["--json", "card", "member", "add", "67089469", "1068514"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"user_id\": 1068514"));
}

#[tokio::test(flavor = "multi_thread")]
async fn member_add_unknown_email_fails_with_invalid_arg() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/users"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/member_users.json")),
        )
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["card", "member", "add", "67089469", "ghost@example.com"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "no user with email `ghost@example.com`",
        ));
}

#[tokio::test(flavor = "multi_thread")]
async fn member_add_garbage_user_fails_with_invalid_arg() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(&server.uri(), tmp.path())
        .args(["card", "member", "add", "67089469", "bob"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid user `bob`"));
}

#[tokio::test(flavor = "multi_thread")]
async fn member_remove_sends_delete() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("DELETE"))
        .and(path("/cards/67089469/members/1068514"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["card", "member", "remove", "67089469", "1068514"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "removed user 1068514 from card 67089469",
        ));
}

#[tokio::test(flavor = "multi_thread")]
async fn member_remove_json_prints_removed_object() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("DELETE"))
        .and(path("/cards/67089469/members/1068514"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["--json", "card", "member", "remove", "67089469", "1068514"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"removed\": true"))
        .stdout(predicate::str::contains("\"user_id\": 1068514"));
}
```

- [ ] **Step 3: Убедиться, что member-тесты падают**

Run: `cargo test -p kaiten --test card_member_test`
Expected: FAIL — все 7 тестов красные: ветка-заглушка `CardCmd::Member(_) | ...` возвращает
`InvalidArg("not implemented yet")`, процесс завершается с кодом 1
(stderr `kaiten: not implemented yet`), assert `success()` падает.

- [ ] **Step 4: Реализовать ветку Member и resolve_user**

В `crates/kaiten/src/commands/card.rs`:

1. Заменить строку импорта

```rust
use crate::cli::CardCmd;
```

на:

```rust
use crate::cli::{CardCmd, CardMemberCmd};
```

2. Заменить ветку-заглушку

```rust
        CardCmd::Member(_) | CardCmd::Comment(_) | CardCmd::Checklist(_) | CardCmd::Tag(_) => {
            Err(CliError::InvalidArg("not implemented yet".into()))
        }
```

на:

```rust
        CardCmd::Member(cmd) => match cmd {
            CardMemberCmd::Add { card, user } => {
                let card_id = parse_card_ref(&card)?;
                let user_id = resolve_user(client, &user).await?;
                let member = client.members().add(card_id, user_id).await?;
                if json {
                    return output::print_json(&member);
                }
                println!("added user {user_id} to card {card_id}");
                Ok(())
            }
            CardMemberCmd::Remove { card, user } => {
                let card_id = parse_card_ref(&card)?;
                let user_id = resolve_user(client, &user).await?;
                client.members().remove(card_id, user_id).await?;
                if json {
                    return output::print_json(&serde_json::json!({
                        "removed": true,
                        "user_id": user_id,
                    }));
                }
                println!("removed user {user_id} from card {card_id}");
                Ok(())
            }
        },
        CardCmd::Comment(_) | CardCmd::Checklist(_) | CardCmd::Tag(_) => {
            Err(CliError::InvalidArg("not implemented yet".into()))
        }
```

3. После `print_card_kv` (Task 15), перед блоком `#[cfg(test)] mod tests`, добавить хелпер:

```rust
/// Resolve a `<user>` CLI argument into a user id.
/// Numeric string -> id as is; contains `@` -> exact email match via GET /users.
async fn resolve_user(client: &KaitenClient, user: &str) -> Result<u64, CliError> {
    if let Ok(id) = user.parse::<u64>() {
        return Ok(id);
    }
    if user.contains('@') {
        let users = client.users().list().await?;
        return users
            .iter()
            .find(|u| u.email.as_deref() == Some(user))
            .map(|u| u.id)
            .ok_or_else(|| CliError::InvalidArg(format!("no user with email `{user}`")));
    }
    Err(CliError::InvalidArg(format!(
        "invalid user `{user}`: expected numeric id or email"
    )))
}
```

- [ ] **Step 5: Убедиться, что member-тесты проходят**

Run: `cargo test -p kaiten --test card_member_test`
Expected: PASS, 7 passed.

- [ ] **Step 6: Создать фикстуры для comment-тестов**

Файл `crates/kaiten/tests/fixtures/comment_created.json` (урезанный ответ
`POST /cards/{id}/comments`; лишние поля: `uid`, `type`, `card_id`, `deleted`, `internal`):

```json
{
  "id": 85523991,
  "uid": "ef4cb581-d2fd-4db9-ae85-aa352e27436d",
  "created": "2026-07-09T15:18:03.341Z",
  "updated": "2026-07-09T15:18:03.341Z",
  "text": "hello from cli",
  "type": 1,
  "edited": false,
  "card_id": 67089469,
  "author_id": 1068514,
  "deleted": false,
  "internal": false
}
```

Файл `crates/kaiten/tests/fixtures/comments_list_two.json` (урезанный ответ
`GET /cards/{id}/comments`; текст второго комментария — 99 символов, длиннее лимита 60;
лишние поля: `uid`, `type`, `card_id`, `deleted`, `internal`, у автора — `lng`, `timezone`):

```json
[
  {
    "id": 85523991,
    "uid": "ef4cb581-d2fd-4db9-ae85-aa352e27436d",
    "created": "2026-07-09T15:18:03.341Z",
    "updated": "2026-07-09T15:18:03.341Z",
    "text": "test comment",
    "type": 1,
    "edited": false,
    "card_id": 67089469,
    "author_id": 1068514,
    "deleted": false,
    "internal": false,
    "author": {
      "id": 1068514,
      "uid": "0abd61ea-9dc5-40eb-b0a9-0d452ba1d8a6",
      "full_name": "dxmuser",
      "username": "dxmuser",
      "email": "user@example.com",
      "activated": true,
      "lng": "ru",
      "timezone": "UTC"
    }
  },
  {
    "id": 85523992,
    "uid": "b2f7d3aa-9c11-4c6e-8a2f-77d0e1f2a3b4",
    "created": "2026-07-10T09:02:11.000Z",
    "updated": "2026-07-10T09:05:42.117Z",
    "text": "deployment pipeline configuration must be updated together with the runbook before the next release",
    "type": 1,
    "edited": true,
    "card_id": 67089469,
    "author_id": 555001,
    "deleted": false,
    "internal": false,
    "author": {
      "id": 555001,
      "uid": "7f9b2c31-56cd-4e0a-8f21-3d9a1b2c3d4e",
      "full_name": "Second User",
      "username": "seconduser",
      "email": "second@example.com",
      "activated": true,
      "lng": "en",
      "timezone": "UTC"
    }
  }
]
```

- [ ] **Step 7: Написать failing-тесты card comment**

Файл `crates/kaiten/tests/card_comment_test.rs` целиком:

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn kaiten(base_url: &str, config_dir: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("kaiten").unwrap();
    cmd.env("KAITEN_BASE_URL", base_url)
        .env("KAITEN_TOKEN", "test-token")
        .env("KAITEN_CONFIG_DIR", config_dir)
        .env("NO_COLOR", "1");
    cmd
}

#[tokio::test(flavor = "multi_thread")]
async fn comment_add_prints_created_id() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("POST"))
        .and(path("/cards/67089469/comments"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_json(serde_json::json!({"text": "hello from cli"})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/comment_created.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["card", "comment", "add", "67089469", "--body", "hello from cli"])
        .assert()
        .success()
        .stdout(predicate::str::contains("85523991"));
}

#[tokio::test(flavor = "multi_thread")]
async fn comment_add_json_prints_model() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("POST"))
        .and(path("/cards/67089469/comments"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/comment_created.json")),
        )
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args([
            "--json", "card", "comment", "add", "67089469", "--body", "hello from cli",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"text\": \"hello from cli\""));
}

#[tokio::test(flavor = "multi_thread")]
async fn comment_list_renders_table_with_truncated_text() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/cards/67089469/comments"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/comments_list_two.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["card", "comment", "list", "67089469"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ID"))
        .stdout(predicate::str::contains("AUTHOR"))
        .stdout(predicate::str::contains("CREATED"))
        .stdout(predicate::str::contains("TEXT"))
        .stdout(predicate::str::contains("85523991"))
        .stdout(predicate::str::contains("dxmuser"))
        .stdout(predicate::str::contains("2026-07-09"))
        .stdout(predicate::str::contains("test comment"))
        .stdout(predicate::str::contains("seconduser"))
        .stdout(predicate::str::contains("2026-07-10"))
        // 60 символов + "…": хвост исходного текста обрезан
        .stdout(predicate::str::contains(
            "deployment pipeline configuration must be updated together w…",
        ))
        .stdout(predicate::str::contains("the next release").not());
}

#[tokio::test(flavor = "multi_thread")]
async fn comment_list_json_prints_full_models() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/cards/67089469/comments"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/comments_list_two.json")),
        )
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["--json", "card", "comment", "list", "67089469"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"text\": \"test comment\""))
        .stdout(predicate::str::contains("the next release"));
}
```

- [ ] **Step 8: Убедиться, что comment-тесты падают**

Run: `cargo test -p kaiten --test card_comment_test`
Expected: FAIL — 4 теста красные (ветка `CardCmd::Comment(_) | ...` — всё ещё заглушка,
процесс завершается с кодом 1, stderr `kaiten: not implemented yet`).

- [ ] **Step 9: Реализовать ветку Comment и хелперы**

В `crates/kaiten/src/commands/card.rs`:

1. Заменить строку импорта

```rust
use crate::cli::{CardCmd, CardMemberCmd};
```

на:

```rust
use crate::cli::{CardCmd, CardCommentCmd, CardMemberCmd};
```

2. Заменить ветку-заглушку

```rust
        CardCmd::Comment(_) | CardCmd::Checklist(_) | CardCmd::Tag(_) => {
            Err(CliError::InvalidArg("not implemented yet".into()))
        }
```

на:

```rust
        CardCmd::Comment(cmd) => match cmd {
            CardCommentCmd::Add { card, body } => {
                let card_id = parse_card_ref(&card)?;
                let comment = client.comments().add(card_id, &body).await?;
                if json {
                    return output::print_json(&comment);
                }
                println!("{}", comment.id);
                Ok(())
            }
            CardCommentCmd::List { card } => {
                let card_id = parse_card_ref(&card)?;
                let comments = client.comments().list(card_id).await?;
                if json {
                    return output::print_json(&comments);
                }
                let mut table = output::table(&["ID", "AUTHOR", "CREATED", "TEXT"]);
                for comment in &comments {
                    let author = comment
                        .author
                        .as_ref()
                        .and_then(|a| a.username.as_deref())
                        .unwrap_or("-")
                        .to_string();
                    table.add_row(vec![
                        comment.id.to_string(),
                        author,
                        date_cell(comment.created.as_deref()),
                        truncate_text(&comment.text, 60),
                    ]);
                }
                println!("{table}");
                Ok(())
            }
        },
        CardCmd::Checklist(_) | CardCmd::Tag(_) => {
            Err(CliError::InvalidArg("not implemented yet".into()))
        }
```

3. Рядом с `resolve_user` (перед `#[cfg(test)] mod tests`) добавить два приватных хелпера:

```rust
/// Truncate to `max` chars, appending `…` when the text was longer.
fn truncate_text(s: &str, max: usize) -> String {
    let mut out: String = s.chars().take(max).collect();
    if s.chars().count() > max {
        out.push('…');
    }
    out
}

/// ISO datetime -> date part before 'T'; None -> "-".
fn date_cell(value: Option<&str>) -> String {
    match value {
        Some(s) => s.split('T').next().unwrap_or(s).to_string(),
        None => "-".to_string(),
    }
}
```

(Задача 13 в `card list` инлайнит `split('T')` без отдельного хелпера, поэтому `date_cell`
добавляется здесь впервые — конфликта имён нет.)

- [ ] **Step 10: Убедиться, что comment-тесты проходят**

Run: `cargo test -p kaiten --test card_comment_test`
Expected: PASS, 4 passed.

- [ ] **Step 11: Полный прогон и линт**

Run: `cargo test -p kaiten && cargo clippy --all-targets -- -D warnings`
Expected: PASS, ноль warnings.

- [ ] **Step 12: Commit**

```
git add crates/kaiten/src/commands/card.rs \
        crates/kaiten/tests/card_member_test.rs \
        crates/kaiten/tests/card_comment_test.rs \
        crates/kaiten/tests/fixtures/member_users.json \
        crates/kaiten/tests/fixtures/member_added.json \
        crates/kaiten/tests/fixtures/comment_created.json \
        crates/kaiten/tests/fixtures/comments_list_two.json
git commit -m "feat(cli): card member and comment commands"
```

---

### Task 17: card checklist list/add + item add/check/uncheck

**Files:**
- Create: crates/kaiten/tests/fixtures/card_with_checklists.json
- Create: crates/kaiten/tests/fixtures/checklist_created.json
- Create: crates/kaiten/tests/fixtures/checklist_item_created.json
- Create: crates/kaiten/tests/fixtures/checklist_item_checked.json
- Create: crates/kaiten/tests/fixtures/checklist_item_unchecked.json
- Modify: crates/kaiten/src/commands/card.rs — заменить ветку-заглушку match внутри `run()` на реализованную `CardCmd::Checklist(...)`; добавить приватный хелпер `set_item_checked`
- Test: crates/kaiten/tests/checklist_test.rs

**Interfaces:**
- Consumes:
  - фасады (§2): `client.cards().get(card_id: u64) -> Result<Card, KaitenError>`
    (чтение чеклистов ТОЛЬКО отсюда — `GET /cards/{id}/checklists` не существует, §3),
    `client.checklists().add(card_id: u64, name: &str) -> Result<Checklist, KaitenError>`,
    `client.checklists().add_item(card_id: u64, checklist_id: u64, text: &str) -> Result<ChecklistItem, KaitenError>`,
    `client.checklists().set_item_checked(card_id: u64, checklist_id: u64, item_id: u64, checked: bool) -> Result<ChecklistItem, KaitenError>`
  - `parse_card_ref` (Task 14), `crate::output::print_json` (Task 10)
  - clap-структуры из `cli.rs` (Task 10, дословно):
    ```rust
    #[derive(Subcommand)]
    pub enum CardChecklistCmd {
        /// List checklists with items
        List { card: String },
        /// Add a checklist
        Add {
            card: String,
            #[arg(long)]
            name: String,
        },
        /// Checklist items
        #[command(subcommand)]
        Item(CardChecklistItemCmd),
    }

    #[derive(Subcommand)]
    pub enum CardChecklistItemCmd {
        /// Add an item
        Add {
            card: String,
            checklist_id: u64,
            #[arg(long)]
            text: String,
        },
        /// Check an item
        Check {
            card: String,
            checklist_id: u64,
            item_id: u64,
        },
        /// Uncheck an item
        Uncheck {
            card: String,
            checklist_id: u64,
            item_id: u64,
        },
    }
    ```
  - заменяемая ветка-заглушка в `commands::card::run` (состояние после Task 16):
    ```rust
            CardCmd::Checklist(_) | CardCmd::Tag(_) => {
                Err(CliError::InvalidArg("not implemented yet".into()))
            }
    ```
- Produces: реализованные команды `kaiten card checklist list|add`,
  `kaiten card checklist item add|check|uncheck` (ветка `CardCmd::Checklist(...)` внутри
  `commands::card::run`); приватный хелпер `set_item_checked` в `card.rs`
  (общий для `Check`/`Uncheck`).

Формат `checklist list` (не таблица — вложенный список): для каждого чеклиста строка
`{name} ({id})`, затем по строке на пункт: два пробела, `[x]` или `[ ]`, id, текст.

- [ ] **Step 1: Создать фикстуры**

Файл `crates/kaiten/tests/fixtures/card_with_checklists.json` (урезанный ответ
`GET /cards/{id}`: полная карточка, интересны `checklists`; второй пункт добавлен
незачеканенным; лишние поля: `uid`, `version`, `goals_total`, `goals_done`, у чеклиста —
`card_id`, `checklist_id`, `created`, у пунктов — `checker_id`, `checked_at`, `deleted`):

```json
{
  "id": 67089469,
  "uid": "c78e313c-ab37-4456-9eb0-904681c4e309",
  "title": "test card from cli",
  "description": "test **description**",
  "state": 1,
  "condition": 1,
  "board_id": 1826109,
  "column_id": 6308511,
  "lane_id": 2293584,
  "type_id": 1,
  "owner_id": 1068514,
  "created": "2026-07-09T15:17:59.905Z",
  "updated": "2026-07-09T15:18:07.303Z",
  "version": 3,
  "sort_order": 1.0689198905237203,
  "comments_total": 1,
  "goals_total": 2,
  "goals_done": 1,
  "checklists": [
    {
      "id": 11747430,
      "uid": "19d5b8ab-1baf-4537-8d42-5578949e75dd",
      "name": "todo",
      "created": "2026-07-09T15:18:04.519Z",
      "updated": "2026-07-09T15:18:04.519Z",
      "sort_order": 1.1522949931390465,
      "card_id": 67089469,
      "checklist_id": 11747430,
      "items": [
        {
          "id": 65658564,
          "uid": "700601dd-9e37-4103-95d1-28edb55ff2df",
          "text": "first item",
          "checked": true,
          "sort_order": 1.7468834610088972,
          "checklist_id": 11747430,
          "checker_id": 1068514,
          "checked_at": "2026-07-09T15:18:05.989Z",
          "deleted": false
        },
        {
          "id": 65658565,
          "uid": "8a1c0f2e-4b4c-4d3f-9a56-0c9d1a2b3c4d",
          "text": "second item",
          "checked": false,
          "sort_order": 2.5,
          "checklist_id": 11747430,
          "checker_id": null,
          "checked_at": null,
          "deleted": false
        }
      ]
    }
  ]
}
```

Файл `crates/kaiten/tests/fixtures/checklist_created.json` (урезанный ответ
`POST /cards/{id}/checklists`; лишние поля: `uid`, `card_id`, `checklist_id`, `policy_id`,
`deleted`):

```json
{
  "id": 11747430,
  "uid": "19d5b8ab-1baf-4537-8d42-5578949e75dd",
  "name": "todo",
  "created": "2026-07-09T15:18:04.519Z",
  "updated": "2026-07-09T15:18:04.519Z",
  "sort_order": 1.1522949931390465,
  "card_id": 67089469,
  "checklist_id": 11747430,
  "policy_id": null,
  "deleted": false
}
```

Файл `crates/kaiten/tests/fixtures/checklist_item_created.json` (урезанный ответ
`POST /cards/{cid}/checklists/{clid}/items`; лишние поля: `uid`, `checklist_id`, `user_id`,
`checker_id`, `checked_at`, `deleted`, `due_date`):

```json
{
  "id": 65658564,
  "uid": "700601dd-9e37-4103-95d1-28edb55ff2df",
  "text": "first item",
  "checked": false,
  "sort_order": 1.7468834610088972,
  "checklist_id": 11747430,
  "user_id": 1068514,
  "checker_id": null,
  "checked_at": null,
  "deleted": false,
  "due_date": null
}
```

Файл `crates/kaiten/tests/fixtures/checklist_item_checked.json` (урезанный ответ
`PATCH .../items/{id}` c `checked=true`):

```json
{
  "id": 65658564,
  "uid": "700601dd-9e37-4103-95d1-28edb55ff2df",
  "text": "first item",
  "checked": true,
  "sort_order": 1.7468834610088972,
  "checklist_id": 11747430,
  "user_id": 1068514,
  "checker_id": 1068514,
  "checked_at": "2026-07-09T15:18:05.989Z",
  "deleted": false,
  "due_date": null
}
```

Файл `crates/kaiten/tests/fixtures/checklist_item_unchecked.json` (тот же пункт после
`checked=false`):

```json
{
  "id": 65658564,
  "uid": "700601dd-9e37-4103-95d1-28edb55ff2df",
  "text": "first item",
  "checked": false,
  "sort_order": 1.7468834610088972,
  "checklist_id": 11747430,
  "user_id": 1068514,
  "checker_id": null,
  "checked_at": null,
  "deleted": false,
  "due_date": null
}
```

- [ ] **Step 2: Написать failing-тесты**

Файл `crates/kaiten/tests/checklist_test.rs` целиком:

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn kaiten(base_url: &str, config_dir: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("kaiten").unwrap();
    cmd.env("KAITEN_BASE_URL", base_url)
        .env("KAITEN_TOKEN", "test-token")
        .env("KAITEN_CONFIG_DIR", config_dir)
        .env("NO_COLOR", "1");
    cmd
}

#[tokio::test(flavor = "multi_thread")]
async fn checklist_list_prints_items_with_marks() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/cards/67089469"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/card_with_checklists.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["card", "checklist", "list", "67089469"])
        .assert()
        .success()
        .stdout(predicate::str::contains("todo (11747430)"))
        .stdout(predicate::str::contains("[x] 65658564 first item"))
        .stdout(predicate::str::contains("[ ] 65658565 second item"));
}

#[tokio::test(flavor = "multi_thread")]
async fn checklist_list_json_prints_checklists_array() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/cards/67089469"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/card_with_checklists.json")),
        )
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["--json", "card", "checklist", "list", "67089469"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"name\": \"todo\""))
        .stdout(predicate::str::contains("\"text\": \"second item\""));
}

#[tokio::test(flavor = "multi_thread")]
async fn checklist_add_posts_name() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("POST"))
        .and(path("/cards/67089469/checklists"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_json(serde_json::json!({"name": "todo"})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/checklist_created.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["card", "checklist", "add", "67089469", "--name", "todo"])
        .assert()
        .success()
        .stdout(predicate::str::contains("created checklist 11747430"));
}

#[tokio::test(flavor = "multi_thread")]
async fn checklist_item_add_posts_text() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("POST"))
        .and(path("/cards/67089469/checklists/11747430/items"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_json(serde_json::json!({"text": "first item"})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/checklist_item_created.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args([
            "card", "checklist", "item", "add", "67089469", "11747430", "--text", "first item",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("created item 65658564"));
}

#[tokio::test(flavor = "multi_thread")]
async fn checklist_item_check_sends_checked_true() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("PATCH"))
        .and(path("/cards/67089469/checklists/11747430/items/65658564"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_json(serde_json::json!({"checked": true})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/checklist_item_checked.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args([
            "card", "checklist", "item", "check", "67089469", "11747430", "65658564",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("item 65658564 checked"));
}

#[tokio::test(flavor = "multi_thread")]
async fn checklist_item_uncheck_sends_checked_false() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("PATCH"))
        .and(path("/cards/67089469/checklists/11747430/items/65658564"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_json(serde_json::json!({"checked": false})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/checklist_item_unchecked.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args([
            "card", "checklist", "item", "uncheck", "67089469", "11747430", "65658564",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("item 65658564 unchecked"));
}
```

- [ ] **Step 3: Убедиться, что тесты падают**

Run: `cargo test -p kaiten --test checklist_test`
Expected: FAIL — все 6 тестов красные (ветка `CardCmd::Checklist(_) | CardCmd::Tag(_)` —
заглушка, процесс завершается с кодом 1, stderr `kaiten: not implemented yet`).

- [ ] **Step 4: Реализовать ветку Checklist**

В `crates/kaiten/src/commands/card.rs`:

1. Заменить строку импорта

```rust
use crate::cli::{CardCmd, CardCommentCmd, CardMemberCmd};
```

на:

```rust
use crate::cli::{CardChecklistCmd, CardChecklistItemCmd, CardCmd, CardCommentCmd, CardMemberCmd};
```

2. Заменить ветку-заглушку

```rust
        CardCmd::Checklist(_) | CardCmd::Tag(_) => {
            Err(CliError::InvalidArg("not implemented yet".into()))
        }
```

на:

```rust
        CardCmd::Checklist(cmd) => match cmd {
            CardChecklistCmd::List { card } => {
                let card_id = parse_card_ref(&card)?;
                let card = client.cards().get(card_id).await?;
                if json {
                    return output::print_json(&card.checklists);
                }
                if card.checklists.is_empty() {
                    println!("no checklists on card {card_id}");
                    return Ok(());
                }
                for checklist in &card.checklists {
                    println!("{} ({})", checklist.name, checklist.id);
                    for item in &checklist.items {
                        let mark = if item.checked.unwrap_or(false) { "x" } else { " " };
                        println!("  [{mark}] {} {}", item.id, item.text);
                    }
                }
                Ok(())
            }
            CardChecklistCmd::Add { card, name } => {
                let card_id = parse_card_ref(&card)?;
                let checklist = client.checklists().add(card_id, &name).await?;
                if json {
                    return output::print_json(&checklist);
                }
                println!("created checklist {}", checklist.id);
                Ok(())
            }
            CardChecklistCmd::Item(cmd) => match cmd {
                CardChecklistItemCmd::Add {
                    card,
                    checklist_id,
                    text,
                } => {
                    let card_id = parse_card_ref(&card)?;
                    let item = client
                        .checklists()
                        .add_item(card_id, checklist_id, &text)
                        .await?;
                    if json {
                        return output::print_json(&item);
                    }
                    println!("created item {}", item.id);
                    Ok(())
                }
                CardChecklistItemCmd::Check {
                    card,
                    checklist_id,
                    item_id,
                } => set_item_checked(client, json, &card, checklist_id, item_id, true).await,
                CardChecklistItemCmd::Uncheck {
                    card,
                    checklist_id,
                    item_id,
                } => set_item_checked(client, json, &card, checklist_id, item_id, false).await,
            },
        },
        CardCmd::Tag(_) => Err(CliError::InvalidArg("not implemented yet".into())),
```

3. Рядом с `resolve_user` (перед `#[cfg(test)] mod tests`) добавить приватный хелпер:

```rust
async fn set_item_checked(
    client: &KaitenClient,
    json: bool,
    card: &str,
    checklist_id: u64,
    item_id: u64,
    checked: bool,
) -> Result<(), CliError> {
    let card_id = parse_card_ref(card)?;
    let item = client
        .checklists()
        .set_item_checked(card_id, checklist_id, item_id, checked)
        .await?;
    if json {
        return crate::output::print_json(&item);
    }
    println!(
        "item {} {}",
        item.id,
        if checked { "checked" } else { "unchecked" }
    );
    Ok(())
}
```

- [ ] **Step 5: Убедиться, что тесты проходят**

Run: `cargo test -p kaiten --test checklist_test`
Expected: PASS, 6 passed.

- [ ] **Step 6: Полный прогон и линт**

Run: `cargo test -p kaiten && cargo clippy --all-targets -- -D warnings`
Expected: PASS, ноль warnings.

- [ ] **Step 7: Commit**

```
git add crates/kaiten/src/commands/card.rs \
        crates/kaiten/tests/checklist_test.rs \
        crates/kaiten/tests/fixtures/card_with_checklists.json \
        crates/kaiten/tests/fixtures/checklist_created.json \
        crates/kaiten/tests/fixtures/checklist_item_created.json \
        crates/kaiten/tests/fixtures/checklist_item_checked.json \
        crates/kaiten/tests/fixtures/checklist_item_unchecked.json
git commit -m "feat(cli): checklist commands"
```

---

### Task 18: card tag add/remove + tag list + card-type list

**Files:**
- Create: crates/kaiten/tests/fixtures/tag_added.json
- Create: crates/kaiten/tests/fixtures/card_with_tags.json
- Create: crates/kaiten/tests/fixtures/tags_list.json
- Create: crates/kaiten/tests/fixtures/card_types_list.json
- Modify: crates/kaiten/src/commands/card.rs — заменить последнюю ветку-заглушку match внутри `run()` на реализованную `CardCmd::Tag(...)`
- Modify: crates/kaiten/src/commands/tag.rs — заменить заглушку тела `run()` (файл целиком ниже; сигнатура из Task 10 сохраняется)
- Modify: crates/kaiten/src/commands/card_type.rs — заменить заглушку тела `run()` (файл целиком ниже; сигнатура из Task 10 сохраняется)
- Test: crates/kaiten/tests/tag_test.rs
- Test: crates/kaiten/tests/card_type_test.rs

**Interfaces:**
- Consumes:
  - фасады (§2): `client.tags().list() -> Result<Vec<Tag>, KaitenError>`,
    `client.tags().add_to_card(card_id: u64, name: &str) -> Result<Tag, KaitenError>`,
    `client.tags().remove_from_card(card_id: u64, tag_id: u64) -> Result<(), KaitenError>`,
    `client.tags().card_types() -> Result<Vec<CardType>, KaitenError>`,
    `client.cards().get(card_id: u64) -> Result<Card, KaitenError>`
  - модель `CardTag { id: u64, tag_id: Option<u64>, name: String, color: Option<i64> }` (§2):
    `id` — id связи карточка-тег, `tag_id` — id тега компании
  - `parse_card_ref` (Task 14), `crate::output::{print_json, table}` (Task 10)
  - clap-структуры из `cli.rs` (Task 10, дословно):
    ```rust
    #[derive(Subcommand)]
    pub enum CardTagCmd {
        /// Add tag by name
        Add { card: String, name: String },
        /// Remove tag by name
        Remove { card: String, name: String },
    }

    #[derive(Subcommand)]
    pub enum TagCmd {
        /// List company tags
        List,
    }

    #[derive(Subcommand)]
    pub enum CardTypeCmd {
        /// List card types
        List,
    }
    ```
  - заменяемая ветка-заглушка в `commands::card::run` (состояние после Task 17):
    ```rust
            CardCmd::Tag(_) => Err(CliError::InvalidArg("not implemented yet".into())),
    ```
  - заглушки модулей из Task 10, тела которых заменяются (сигнатуры финальные):
    ```rust
    // commands/tag.rs
    pub async fn run(_cmd: TagCmd, _client: &KaitenClient, _json: bool) -> Result<(), CliError> {
        Err(CliError::InvalidArg("not implemented yet".into()))
    }

    // commands/card_type.rs
    pub async fn run(_cmd: CardTypeCmd, _client: &KaitenClient, _json: bool) -> Result<(), CliError> {
        Err(CliError::InvalidArg("not implemented yet".into()))
    }
    ```
    `main.rs` (Task 10) уже вызывает
    `commands::tag::run(cmd, &client, cli.json)` и `commands::card_type::run(cmd, &client, cli.json)` —
    менять его не нужно.
- Produces: реализованные команды `kaiten card tag add|remove` (ветка `CardCmd::Tag(...)`
  внутри `commands::card::run`; после этой задачи в `run()` не остаётся заглушек),
  `kaiten tag list`, `kaiten card-type list`.

Семантика `card tag remove <name>`: `cards().get(card_id)` → поиск в `card.tags` по точному
`name` → DELETE по `tag_id` из `CardTag`, а если `tag_id == None` — по полю `id` (id связи).
Тега с таким именем нет → `CliError::InvalidArg` с перечислением имеющихся имён тегов
карточки (или `(none)`, если тегов нет). При `--json` команда `card tag remove` печатает
`{"removed": true, "tag": "<name>"}`. Таблицы: `tag list` — `ID, NAME`;
`card-type list` — `ID, NAME, LETTER` (пустой letter — `-`).

- [ ] **Step 1: Создать фикстуры**

Файл `crates/kaiten/tests/fixtures/tag_added.json` (урезанный ответ `POST /cards/{id}/tags`;
лишние поля: `uid`, `created`, `updated`, `company_id`, `archived`):

```json
{
  "id": 1110772,
  "uid": "f9ba3ae1-6227-4036-82ec-412ba30556e7",
  "name": "cli-test",
  "color": 15,
  "created": "2026-07-09T15:18:07.303Z",
  "updated": "2026-07-09T15:18:07.303Z",
  "company_id": 398610,
  "archived": false
}
```

Файл `crates/kaiten/tests/fixtures/card_with_tags.json` (урезанный ответ `GET /cards/{id}`;
у первого тега есть `tag_id`, у второго `tag_id` отсутствует — проверяем фолбэк на `id`;
лишние поля: `uid`, `version`, `comments_total`, у тегов — `card_id`, `created`):

```json
{
  "id": 67089469,
  "uid": "c78e313c-ab37-4456-9eb0-904681c4e309",
  "title": "test card from cli",
  "state": 1,
  "condition": 1,
  "board_id": 1826109,
  "column_id": 6308511,
  "created": "2026-07-09T15:17:59.905Z",
  "updated": "2026-07-09T15:18:07.303Z",
  "version": 4,
  "comments_total": 1,
  "tags": [
    {
      "id": 1110772,
      "tag_id": 1110772,
      "name": "cli-test",
      "color": 15,
      "card_id": 67089469,
      "created": "2026-07-09T15:18:07.303Z"
    },
    {
      "id": 2220001,
      "name": "legacy-link",
      "color": 3,
      "card_id": 67089469,
      "created": "2026-07-09T15:20:00.000Z"
    }
  ]
}
```

Файл `crates/kaiten/tests/fixtures/tags_list.json` (урезанный ответ `GET /tags`; лишние
поля: `uid`, `company_id`, `archived`, `created`):

```json
[
  {
    "id": 1110772,
    "uid": "f9ba3ae1-6227-4036-82ec-412ba30556e7",
    "name": "cli-test",
    "color": 15,
    "company_id": 398610,
    "archived": false,
    "created": "2026-07-09T15:18:07.303Z"
  },
  {
    "id": 1110773,
    "uid": "5b0a9a10-77aa-4bb3-9c44-d16273849a01",
    "name": "backend",
    "color": 7,
    "company_id": 398610,
    "archived": false,
    "created": "2026-07-08T10:00:00.000Z"
  }
]
```

Файл `crates/kaiten/tests/fixtures/card_types_list.json` (урезанный ответ `GET /card-types`;
лишние поля: `uid`, `company_id`, `suggest_fields`, `created`):

```json
[
  {
    "id": 1,
    "uid": "64792c05-0d0d-4b19-a3a6-d3c34b0a197c",
    "name": "Card",
    "color": 1,
    "letter": "C",
    "archived": false,
    "company_id": null,
    "suggest_fields": true,
    "created": "2014-11-13T22:20:14.374Z"
  },
  {
    "id": 692717,
    "uid": "dc9a097b-eb33-48bd-89fe-56faee4f9cbb",
    "name": "Feature",
    "color": 2,
    "letter": "F",
    "archived": false,
    "company_id": 398610,
    "suggest_fields": true,
    "created": "2026-07-09T15:13:27.790Z"
  },
  {
    "id": 692718,
    "uid": "38fd521d-7ab8-472a-978e-f0313732c5f2",
    "name": "Bug",
    "color": 3,
    "letter": "B",
    "archived": false,
    "company_id": 398610,
    "suggest_fields": true,
    "created": "2026-07-09T15:13:27.813Z"
  }
]
```

- [ ] **Step 2: Написать failing-тесты tag**

Файл `crates/kaiten/tests/tag_test.rs` целиком:

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn kaiten(base_url: &str, config_dir: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("kaiten").unwrap();
    cmd.env("KAITEN_BASE_URL", base_url)
        .env("KAITEN_TOKEN", "test-token")
        .env("KAITEN_CONFIG_DIR", config_dir)
        .env("NO_COLOR", "1");
    cmd
}

#[tokio::test(flavor = "multi_thread")]
async fn card_tag_add_posts_name() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("POST"))
        .and(path("/cards/67089469/tags"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_json(serde_json::json!({"name": "cli-test"})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/tag_added.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["card", "tag", "add", "67089469", "cli-test"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "added tag cli-test (1110772) to card 67089469",
        ));
}

#[tokio::test(flavor = "multi_thread")]
async fn card_tag_remove_deletes_by_tag_id() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/cards/67089469"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/card_with_tags.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("DELETE"))
        .and(path("/cards/67089469/tags/1110772"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["card", "tag", "remove", "67089469", "cli-test"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "removed tag cli-test from card 67089469",
        ));
}

#[tokio::test(flavor = "multi_thread")]
async fn card_tag_remove_json_prints_removed_object() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/cards/67089469"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/card_with_tags.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("DELETE"))
        .and(path("/cards/67089469/tags/1110772"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["--json", "card", "tag", "remove", "67089469", "cli-test"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"removed\": true"))
        .stdout(predicate::str::contains("\"tag\": \"cli-test\""));
}

#[tokio::test(flavor = "multi_thread")]
async fn card_tag_remove_falls_back_to_link_id_when_no_tag_id() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/cards/67089469"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/card_with_tags.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("DELETE"))
        .and(path("/cards/67089469/tags/2220001"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["card", "tag", "remove", "67089469", "legacy-link"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "removed tag legacy-link from card 67089469",
        ));
}

#[tokio::test(flavor = "multi_thread")]
async fn card_tag_remove_unknown_name_lists_existing_tags() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/cards/67089469"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/card_with_tags.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["card", "tag", "remove", "67089469", "nope"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("has no tag `nope`"))
        .stderr(predicate::str::contains("cli-test, legacy-link"));
}

#[tokio::test(flavor = "multi_thread")]
async fn tag_list_renders_table() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/tags"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/tags_list.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["tag", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ID"))
        .stdout(predicate::str::contains("NAME"))
        .stdout(predicate::str::contains("1110772"))
        .stdout(predicate::str::contains("cli-test"))
        .stdout(predicate::str::contains("backend"));
}

#[tokio::test(flavor = "multi_thread")]
async fn tag_list_json_prints_models() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/tags"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/tags_list.json")),
        )
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["--json", "tag", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"name\": \"backend\""));
}
```

- [ ] **Step 3: Написать failing-тесты card-type**

Файл `crates/kaiten/tests/card_type_test.rs` целиком:

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn kaiten(base_url: &str, config_dir: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("kaiten").unwrap();
    cmd.env("KAITEN_BASE_URL", base_url)
        .env("KAITEN_TOKEN", "test-token")
        .env("KAITEN_CONFIG_DIR", config_dir)
        .env("NO_COLOR", "1");
    cmd
}

#[tokio::test(flavor = "multi_thread")]
async fn card_type_list_renders_table() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/card-types"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/card_types_list.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["card-type", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ID"))
        .stdout(predicate::str::contains("NAME"))
        .stdout(predicate::str::contains("LETTER"))
        .stdout(predicate::str::contains("692717"))
        .stdout(predicate::str::contains("Feature"))
        .stdout(predicate::str::contains("Bug"));
}

#[tokio::test(flavor = "multi_thread")]
async fn card_type_list_json_prints_models() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/card-types"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/card_types_list.json")),
        )
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["--json", "card-type", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"letter\": \"F\""));
}
```

- [ ] **Step 4: Убедиться, что тесты падают**

Run: `cargo test -p kaiten --test tag_test --test card_type_test`
Expected: FAIL — все 9 тестов красные (7 в tag_test + 2 в card_type_test; ветка
`CardCmd::Tag(_)` и заглушки `tag::run`/`card_type::run` возвращают
`InvalidArg("not implemented yet")`, exit code 1).

- [ ] **Step 5: Реализовать ветку Tag в commands/card.rs**

В `crates/kaiten/src/commands/card.rs`:

1. Заменить строку импорта

```rust
use crate::cli::{CardChecklistCmd, CardChecklistItemCmd, CardCmd, CardCommentCmd, CardMemberCmd};
```

на:

```rust
use crate::cli::{
    CardChecklistCmd, CardChecklistItemCmd, CardCmd, CardCommentCmd, CardMemberCmd, CardTagCmd,
};
```

2. Заменить последнюю ветку-заглушку

```rust
        CardCmd::Tag(_) => Err(CliError::InvalidArg("not implemented yet".into())),
```

на:

```rust
        CardCmd::Tag(cmd) => match cmd {
            CardTagCmd::Add { card, name } => {
                let card_id = parse_card_ref(&card)?;
                let tag = client.tags().add_to_card(card_id, &name).await?;
                if json {
                    return output::print_json(&tag);
                }
                println!("added tag {} ({}) to card {card_id}", tag.name, tag.id);
                Ok(())
            }
            CardTagCmd::Remove { card, name } => {
                let card_id = parse_card_ref(&card)?;
                let card = client.cards().get(card_id).await?;
                let Some(card_tag) = card.tags.iter().find(|t| t.name == name) else {
                    let existing = if card.tags.is_empty() {
                        "(none)".to_string()
                    } else {
                        card.tags
                            .iter()
                            .map(|t| t.name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    };
                    return Err(CliError::InvalidArg(format!(
                        "card {card_id} has no tag `{name}`; existing tags: {existing}"
                    )));
                };
                let tag_id = card_tag.tag_id.unwrap_or(card_tag.id);
                client.tags().remove_from_card(card_id, tag_id).await?;
                if json {
                    return output::print_json(&serde_json::json!({
                        "removed": true,
                        "tag": name,
                    }));
                }
                println!("removed tag {name} from card {card_id}");
                Ok(())
            }
        },
```

После этой замены в `match cmd` внутри `commands::card::run` не остаётся веток-заглушек.

- [ ] **Step 6: Реализовать tag list и card-type list**

Файл `crates/kaiten/src/commands/tag.rs` целиком (сигнатура `run` — та же, что в Task 10):

```rust
use kaiten_client::KaitenClient;

use crate::cli::TagCmd;
use crate::error::CliError;
use crate::output;

pub async fn run(cmd: TagCmd, client: &KaitenClient, json: bool) -> Result<(), CliError> {
    match cmd {
        TagCmd::List => {
            let tags = client.tags().list().await?;
            if json {
                return output::print_json(&tags);
            }
            let mut table = output::table(&["ID", "NAME"]);
            for tag in &tags {
                table.add_row(vec![tag.id.to_string(), tag.name.clone()]);
            }
            println!("{table}");
            Ok(())
        }
    }
}
```

Файл `crates/kaiten/src/commands/card_type.rs` целиком (сигнатура `run` — та же, что в Task 10):

```rust
use kaiten_client::KaitenClient;

use crate::cli::CardTypeCmd;
use crate::error::CliError;
use crate::output;

pub async fn run(cmd: CardTypeCmd, client: &KaitenClient, json: bool) -> Result<(), CliError> {
    match cmd {
        CardTypeCmd::List => {
            let types = client.tags().card_types().await?;
            if json {
                return output::print_json(&types);
            }
            let mut table = output::table(&["ID", "NAME", "LETTER"]);
            for t in &types {
                table.add_row(vec![
                    t.id.to_string(),
                    t.name.clone(),
                    t.letter.clone().unwrap_or_else(|| "-".to_string()),
                ]);
            }
            println!("{table}");
            Ok(())
        }
    }
}
```

- [ ] **Step 7: Убедиться, что тесты проходят**

Run: `cargo test -p kaiten --test tag_test --test card_type_test`
Expected: PASS — 7 passed в tag_test, 2 passed в card_type_test.

- [ ] **Step 8: Полный прогон и линт**

Run: `cargo test -p kaiten && cargo clippy --all-targets -- -D warnings`
Expected: PASS, ноль warnings.

- [ ] **Step 9: Commit**

```
git add crates/kaiten/src/commands/card.rs \
        crates/kaiten/src/commands/tag.rs \
        crates/kaiten/src/commands/card_type.rs \
        crates/kaiten/tests/tag_test.rs \
        crates/kaiten/tests/card_type_test.rs \
        crates/kaiten/tests/fixtures/tag_added.json \
        crates/kaiten/tests/fixtures/card_with_tags.json \
        crates/kaiten/tests/fixtures/tags_list.json \
        crates/kaiten/tests/fixtures/card_types_list.json
git commit -m "feat(cli): tag and card-type commands"
```

---

### Task 19: kaiten api METHOD PATH [--data]

**Files:**
- Create: crates/kaiten/tests/fixtures/api_user_current.json
- Create: crates/kaiten/tests/fixtures/api_card_created.json
- Modify: crates/kaiten/src/commands/api.rs — заменить заглушку реализацией (файл целиком ниже; сигнатура `run` из Task 10 сохраняется)
- Test: crates/kaiten/tests/api_test.rs

**Interfaces:**
- Consumes:
  - `client.raw(method: reqwest::Method, path: &str, body: Option<serde_json::Value>) -> Result<serde_json::Value, KaitenError>` (§2)
  - `CliError::{InvalidArg, Json}` (§4; `Json` — через `#[from] serde_json::Error`)
  - вариант enum `Commands` из `cli.rs` (Task 10, дословно):
    ```rust
    /// Raw API request (like `gh api`)
    Api {
        /// HTTP method: GET|POST|PATCH|PUT|DELETE
        method: String,
        /// Path starting with '/', query string included
        path: String,
        /// JSON request body
        #[arg(long)]
        data: Option<String>,
    },
    ```
  - заглушка из Task 10, тело которой заменяется (сигнатура финальная, D2):
    ```rust
    pub async fn run(
        _client: &KaitenClient,
        _method: &str,
        _path: &str,
        _data: Option<String>,
    ) -> Result<(), CliError>
    ```
    `main.rs` (Task 10) уже вызывает
    `Commands::Api { method, path, data } => commands::api::run(&client, &method, &path, data).await` —
    менять его не нужно.
  - `reqwest = { workspace = true }` уже в `[dependencies]` крейта `kaiten` (Task 10 Step 1 —
    добавлен именно ради `reqwest::Method` для этой команды)
- Produces: реализованная команда `kaiten api`; `fn parse_method(&str) -> Result<reqwest::Method, CliError>` (приватная).

Поведение: METHOD валидируется регистронезависимо по списку GET|POST|PATCH|PUT|DELETE →
`reqwest::Method`; иной метод → `InvalidArg("unsupported method ...")` → exit 1.
`--data` парсится `serde_json::from_str` → мусор даёт `CliError::Json` → exit 1.
Вывод ВСЕГДА `serde_json::to_string_pretty` (глобальный `--json` игнорируется — вывод и
так JSON).

- [ ] **Step 1: Создать фикстуры**

Файл `crates/kaiten/tests/fixtures/api_user_current.json` (урезанный ответ
`GET /users/current`; лишние поля: `lng`, `timezone`, `company_id`, `role`):

```json
{
  "id": 1068514,
  "uid": "0abd61ea-9dc5-40eb-b0a9-0d452ba1d8a6",
  "full_name": "dxmuser",
  "username": "dxmuser",
  "email": "user@example.com",
  "activated": true,
  "lng": "ru",
  "timezone": "UTC",
  "company_id": 398610,
  "role": 1
}
```

Файл `crates/kaiten/tests/fixtures/api_card_created.json` (урезанный ответ `POST /cards`;
лишние поля: `uid`, `version`, `sort_order`):

```json
{
  "id": 67089469,
  "uid": "c78e313c-ab37-4456-9eb0-904681c4e309",
  "title": "from raw api",
  "board_id": 1826109,
  "column_id": 6308511,
  "state": 1,
  "condition": 1,
  "version": 1,
  "sort_order": 1.5
}
```

- [ ] **Step 2: Написать failing-тесты**

Файл `crates/kaiten/tests/api_test.rs` целиком:

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn kaiten(base_url: &str, config_dir: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("kaiten").unwrap();
    cmd.env("KAITEN_BASE_URL", base_url)
        .env("KAITEN_TOKEN", "test-token")
        .env("KAITEN_CONFIG_DIR", config_dir)
        .env("NO_COLOR", "1");
    cmd
}

#[tokio::test(flavor = "multi_thread")]
async fn api_get_prints_pretty_json_and_accepts_lowercase_method() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/users/current"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/api_user_current.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    let assert = kaiten(&server.uri(), tmp.path())
        .args(["api", "get", "/users/current"])
        .assert()
        .success();

    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    // stdout — валидный JSON
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["id"], 1068514);
    // и он pretty-printed (двухпробельный отступ to_string_pretty)
    assert!(stdout.contains("  \"id\": 1068514"), "stdout: {stdout}");
}

#[tokio::test(flavor = "multi_thread")]
async fn api_post_sends_data_as_json_body() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("POST"))
        .and(path("/cards"))
        .and(header("Authorization", "Bearer test-token"))
        .and(body_json(serde_json::json!({
            "board_id": 1826109,
            "title": "from raw api"
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/api_card_created.json")),
        )
        .expect(1)
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args([
            "api",
            "POST",
            "/cards",
            "--data",
            "{\"board_id\":1826109,\"title\":\"from raw api\"}",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"id\": 67089469"))
        .stdout(predicate::str::contains("\"title\": \"from raw api\""));
}

#[tokio::test(flavor = "multi_thread")]
async fn api_unsupported_method_exits_with_error() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(&server.uri(), tmp.path())
        .args(["api", "FETCH", "/users/current"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unsupported method"));
}

#[tokio::test(flavor = "multi_thread")]
async fn api_garbage_data_exits_with_json_error() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    kaiten(&server.uri(), tmp.path())
        .args(["api", "POST", "/cards", "--data", "{not json"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("kaiten: json:"));
}

#[tokio::test(flavor = "multi_thread")]
async fn api_ignores_global_json_flag() {
    let server = MockServer::start().await;
    let tmp = tempfile::tempdir().unwrap();

    Mock::given(method("GET"))
        .and(path("/users/current"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(include_str!("fixtures/api_user_current.json")),
        )
        .mount(&server)
        .await;

    kaiten(&server.uri(), tmp.path())
        .args(["--json", "api", "GET", "/users/current"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"id\": 1068514"));
}
```

- [ ] **Step 3: Убедиться, что тесты падают**

Run: `cargo test -p kaiten --test api_test`
Expected: FAIL — все 5 тестов красные. Тесты про METHOD/`--data` падают, потому что
заглушка возвращает `not implemented yet`, а не сообщения `unsupported method` / `json:`.

- [ ] **Step 4: Проверить, что reqwest уже в зависимостях бинарника**

Run: `grep reqwest crates/kaiten/Cargo.toml`
Expected: строка `reqwest = { workspace = true }` уже есть — её добавил Task 10 (Step 1)
именно ради `reqwest::Method` для этой команды. Ничего добавлять не нужно.

- [ ] **Step 5: Реализовать команду api**

Файл `crates/kaiten/src/commands/api.rs` целиком:

```rust
use kaiten_client::KaitenClient;

use crate::error::CliError;

pub async fn run(
    client: &KaitenClient,
    method: &str,
    path: &str,
    data: Option<String>,
) -> Result<(), CliError> {
    let method = parse_method(method)?;
    let body = match data {
        Some(raw) => Some(serde_json::from_str::<serde_json::Value>(&raw)?),
        None => None,
    };
    let value = client.raw(method, path, body).await?;
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

fn parse_method(s: &str) -> Result<reqwest::Method, CliError> {
    match s.to_ascii_uppercase().as_str() {
        "GET" => Ok(reqwest::Method::GET),
        "POST" => Ok(reqwest::Method::POST),
        "PATCH" => Ok(reqwest::Method::PATCH),
        "PUT" => Ok(reqwest::Method::PUT),
        "DELETE" => Ok(reqwest::Method::DELETE),
        other => Err(CliError::InvalidArg(format!(
            "unsupported method `{other}`: expected GET|POST|PATCH|PUT|DELETE"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_method;

    #[test]
    fn parse_method_is_case_insensitive() {
        assert_eq!(parse_method("get").unwrap(), reqwest::Method::GET);
        assert_eq!(parse_method("Patch").unwrap(), reqwest::Method::PATCH);
        assert_eq!(parse_method("DELETE").unwrap(), reqwest::Method::DELETE);
    }

    #[test]
    fn parse_method_rejects_unknown() {
        let err = parse_method("FETCH").unwrap_err();
        assert!(err.to_string().contains("unsupported method"));
    }
}
```

- [ ] **Step 6: Убедиться, что тесты проходят**

Run: `cargo test -p kaiten --test api_test && cargo test -p kaiten --bin kaiten commands::api`
Expected: PASS — 5 интеграционных + 2 unit-теста зелёные (крейт `kaiten` — бинарник без
`[lib]`, поэтому unit-тесты гоняются таргетом `--bin kaiten`).

- [ ] **Step 7: Полный прогон и линт**

Run: `cargo test -p kaiten && cargo clippy --all-targets -- -D warnings`
Expected: PASS, ноль warnings.

- [ ] **Step 8: Commit**

```
git add crates/kaiten/src/commands/api.rs \
        crates/kaiten/tests/api_test.rs \
        crates/kaiten/tests/fixtures/api_user_current.json \
        crates/kaiten/tests/fixtures/api_card_created.json
git commit -m "feat(cli): raw api command"
```

---

### Task 20: kaiten completion <shell>

**Files:**
- Modify: crates/kaiten/src/commands/completion.rs — заменить заглушку реализацией (файл целиком ниже; сигнатура `run` из Task 10 сохраняется)
- Test: crates/kaiten/tests/completion_test.rs

**Interfaces:**
- Consumes:
  - `clap_complete::generate(shell, &mut cmd, bin_name, &mut impl std::io::Write)`
    (workspace-зависимость `clap_complete = "4"`, уже в `[dependencies]` крейта `kaiten`
    с Task 10)
  - `crate::cli::Cli` (clap derive, Task 10) и `clap::CommandFactory` (`Cli::command()`)
  - enum из `cli.rs` (Task 10, дословно; НЕ переименовывать и НЕ менять):
    ```rust
    #[derive(Debug, Clone, Copy, ValueEnum)]
    pub enum Shell {
        Bash,
        Zsh,
        Fish,
    }
    ```
  - заглушка из Task 10, тело которой заменяется (сигнатура финальная, синхронная):
    ```rust
    pub fn run(_shell: Shell) -> Result<(), CliError> {
        Err(CliError::InvalidArg("not implemented yet".into()))
    }
    ```
  - диспатч в `main.rs` (Task 10) уже стоит ПЕРВОЙ веткой match — до `config::resolve()`
    и создания `KaitenClient`:
    ```rust
    Commands::Completion { shell } => commands::completion::run(shell),
    ```
    `main.rs` и `cli.rs` в этой задаче НЕ меняются.
- Produces: реализованная команда `kaiten completion bash|zsh|fish`.

Команда пишет скрипт автодополнения в stdout и не требует ни токена, ни домена —
диспатч Task 10 обрабатывает её до резолва конфига.

- [ ] **Step 1: Написать failing-тест**

Файл `crates/kaiten/tests/completion_test.rs` целиком (env с токеном намеренно НЕ задаётся
— completion обязана работать без конфига; `KAITEN_*` вычищаются на случай реального
окружения разработчика):

```rust
use assert_cmd::Command;
use predicates::prelude::*;

fn kaiten_no_config() -> Command {
    let tmp = tempfile::tempdir().unwrap();
    let mut cmd = Command::cargo_bin("kaiten").unwrap();
    cmd.env("KAITEN_CONFIG_DIR", tmp.path())
        .env("NO_COLOR", "1")
        .env_remove("KAITEN_TOKEN")
        .env_remove("KAITEN_DOMAIN")
        .env_remove("KAITEN_BASE_URL");
    // tempdir удалится по выходу из функции — для completion конфиг всё равно не читается
    cmd
}

#[test]
fn completion_zsh_contains_function_and_subcommands() {
    kaiten_no_config()
        .args(["completion", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("_kaiten"))
        .stdout(predicate::str::contains("card"))
        .stdout(predicate::str::contains("board"))
        .stdout(predicate::str::contains("completion"));
}

#[test]
fn completion_bash_contains_function() {
    kaiten_no_config()
        .args(["completion", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("_kaiten"));
}

#[test]
fn completion_fish_mentions_binary() {
    kaiten_no_config()
        .args(["completion", "fish"])
        .assert()
        .success()
        .stdout(predicate::str::contains("kaiten"));
}

#[test]
fn completion_rejects_unknown_shell() {
    kaiten_no_config()
        .args(["completion", "powershell"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value"));
}
```

- [ ] **Step 2: Убедиться, что тесты падают**

Run: `cargo test -p kaiten --test completion_test`
Expected: FAIL — заглушка возвращает `InvalidArg("not implemented yet")` и ничего не пишет
в stdout, поэтому три теста на `success()`/`contains` красные;
`completion_rejects_unknown_shell` может проходить уже сейчас (ошибку даёт сам clap) —
обязательно проверить падение остальных трёх.

- [ ] **Step 3: Реализовать генерацию completion**

Файл `crates/kaiten/src/commands/completion.rs` целиком:

```rust
use clap::CommandFactory;

use crate::cli::{Cli, Shell};
use crate::error::CliError;

pub fn run(shell: Shell) -> Result<(), CliError> {
    let shell = match shell {
        Shell::Bash => clap_complete::Shell::Bash,
        Shell::Zsh => clap_complete::Shell::Zsh,
        Shell::Fish => clap_complete::Shell::Fish,
    };
    let mut cmd = Cli::command();
    clap_complete::generate(shell, &mut cmd, "kaiten", &mut std::io::stdout());
    Ok(())
}
```

`main.rs` менять не нужно: ветка `Commands::Completion { shell } =>
commands::completion::run(shell),` уже стоит первой в match (Task 10) и выполняется без
`config::resolve()`.

- [ ] **Step 4: Убедиться, что тесты проходят**

Run: `cargo test -p kaiten --test completion_test`
Expected: PASS, 4 passed.

- [ ] **Step 5: Полный прогон и линт**

Run: `cargo test -p kaiten && cargo clippy --all-targets -- -D warnings`
Expected: PASS, ноль warnings.

- [ ] **Step 6: Commit**

```
git add crates/kaiten/src/commands/completion.rs \
        crates/kaiten/tests/completion_test.rs
git commit -m "feat(cli): shell completion"
```

## Milestone 5: MCP-сервер, живой smoke, документация (Tasks 21–25)

Предпосылка: Tasks 1–20 выполнены — весь `kaiten-client` (§2 контракта) и CLI (§4) работают,
`cargo test` зелёный. С Task 10 существуют: полное clap-дерево в `cli.rs` (включая
`pub enum McpCmd { Serve }` и вариант `Commands::Mcp(McpCmd)`), декларация `mod mcp;`
в `main.rs`, ветка диспатча `Commands::Mcp(cmd) => mcp::run(cmd).await` и заглушка
`crates/kaiten/src/mcp/mod.rs` с `pub async fn run(cmd: McpCmd)`. Версии `rmcp` и
`schemars` пинованы в `[workspace.dependencies]` корневого Cargo.toml с Task 1.
MCP-инструменты, их имена и параметры — строго по §5 INTERFACES.md.

---

### Task 21: MCP-скелет + 8 read-only инструментов

**Files:**
- Modify: crates/kaiten/src/mcp/mod.rs — ЗАМЕНА заглушки Task 10 (`pub async fn run(cmd: McpCmd)` удаляется) на реализацию MCP-сервера
- Create: crates/kaiten/tests/fixtures/mcp_spaces.json
- Modify: crates/kaiten/Cargo.toml — добавить `rmcp = { workspace = true }` и `schemars = { workspace = true }` (версии пинованы в Task 1)
- Modify: crates/kaiten/src/main.rs — замена ветки диспатча `Commands::Mcp(cmd) => mcp::run(cmd).await` (Task 10) на ветку с резолвом конфига и `mcp::serve(client)`
- Test: unit-тесты внутри crates/kaiten/src/mcp/mod.rs (`#[cfg(test)] mod tests`) — бинарный крейт без lib-таргета, поэтому in-process тесты живут в модуле

`cli.rs` НЕ трогаем: `McpCmd { Serve }` и вариант `Commands::Mcp` существуют с Task 10
и не меняются; `mod mcp;` в `main.rs` объявлен там же.

**Interfaces:**
- Consumes:
  - `kaiten_client::KaitenClient::new(base_url: &str, token: &str) -> Result<Self, KaitenError>`
  - фасады: `client.users().current() -> Result<User>`, `client.spaces().list() -> Result<Vec<Space>>`, `client.boards().list(space_id: u64) -> Result<Vec<Board>>`, `client.boards().get(board_id: u64) -> Result<Board>`, `client.cards().list(&CardFilter) -> Result<Vec<Card>>`, `client.cards().get(card_id: u64) -> Result<Card>`, `client.comments().list(card_id: u64) -> Result<Vec<Comment>>`
  - `kaiten_client::CardFilter` (поля по §2), `kaiten_client::KaitenError`
  - `crate::config::resolve() -> Result<Resolved, CliError>`, `crate::error::CliError`
  - `crate::cli::McpCmd` (Task 10; используется только в ветке диспатча main.rs)
- Produces:
  - `pub struct KaitenMcp { client: Arc<KaitenClient>, tool_router: ToolRouter<Self> }`
  - `KaitenMcp::new(client: Arc<KaitenClient>) -> Self`
  - `pub async fn serve(client: KaitenClient) -> Result<(), CliError>` — точка входа для `kaiten mcp serve`; ЗАМЕНЯЕТ заглушку `mcp::run(cmd: McpCmd)` из Task 10 (старая функция удаляется)
  - сгенерированный макросом `KaitenMcp::tool_router() -> ToolRouter<KaitenMcp>` (нужен Task 22/23 для проверки списка инструментов)
  - 8 tool-методов: `current_user`, `list_spaces`, `list_boards`, `get_board`, `list_cards`, `get_card`, `list_comments`, `list_checklists`

- [ ] **Step 1: Добавить зависимости rmcp и schemars**

  В `crates/kaiten/Cargo.toml` в секцию `[dependencies]` добавить две строки (НЕ `cargo add`;
  версии и features пинованы в `[workspace.dependencies]` корневого Cargo.toml с Task 1:
  `rmcp = { version = "2", features = ["server", "macros", "transport-io"] }`, `schemars = "1"`):

  ```toml
  rmcp = { workspace = true }
  schemars = { workspace = true }
  ```

  `wiremock` уже есть в `[dev-dependencies]` крейта с Task 10 — ничего добавлять не нужно.

  Expected: обе строки в `crates/kaiten/Cargo.toml`; компиляция с rmcp 2.x проверяется
  на Step 5 — первый `cargo build` задачи идёт только ПОСЛЕ wiring'а main.rs в Step 4.

- [ ] **Step 2: Фикстура списка пространств (урезанный реальный ответ GET /spaces)**

  Создать `crates/kaiten/tests/fixtures/mcp_spaces.json` — поля модели `Space` (id, uid, title, archived) плюс 5 лишних полей для проверки толерантности:

  ```json
  [
    {
      "id": 810669,
      "uid": "f52db47b-cbd9-4b50-98e7-19219cae0291",
      "title": "Первое пространство",
      "archived": false,
      "access": "by_invite",
      "entity_type": "space",
      "company_id": 398610,
      "sort_order": 1.6486044425519069,
      "external_id": null
    }
  ]
  ```

- [ ] **Step 3: Failing test — дописать тестовый модуль в существующий mcp/mod.rs**

  Дописать в КОНЕЦ существующего `crates/kaiten/src/mcp/mod.rs` (заглушка Task 10 с
  `pub async fn run(cmd: McpCmd)` пока остаётся выше — её удалит Step 4) тестовый модуль:

  ```rust
  #[cfg(test)]
  mod tests {
      use std::sync::Arc;

      use kaiten_client::KaitenClient;
      use rmcp::handler::server::wrapper::Parameters;
      use rmcp::model::CallToolResult;
      use wiremock::matchers::{header, method, path};
      use wiremock::{Mock, MockServer, ResponseTemplate};

      use super::{GetCardParams, KaitenMcp};

      const SPACES_FIXTURE: &str = include_str!("../../tests/fixtures/mcp_spaces.json");

      fn mcp_for(server: &MockServer) -> KaitenMcp {
          let client = KaitenClient::new(&server.uri(), "test-token").unwrap();
          KaitenMcp::new(Arc::new(client))
      }

      fn tool_text(result: &CallToolResult) -> String {
          result.content[0]
              .as_text()
              .expect("tool result must be text content")
              .text
              .clone()
      }

      #[tokio::test]
      async fn list_spaces_returns_spaces_json() {
          let server = MockServer::start().await;
          Mock::given(method("GET"))
              .and(path("/spaces"))
              .and(header("Authorization", "Bearer test-token"))
              .respond_with(ResponseTemplate::new(200).set_body_string(SPACES_FIXTURE))
              .mount(&server)
              .await;

          let mcp = mcp_for(&server);
          let result = mcp.list_spaces().await.unwrap();
          let value: serde_json::Value = serde_json::from_str(&tool_text(&result)).unwrap();
          assert_eq!(value[0]["id"], 810669);
          assert_eq!(value[0]["title"], "Первое пространство");
      }

      #[tokio::test]
      async fn get_card_403_maps_to_tool_error_with_api_text() {
          let server = MockServer::start().await;
          // Kaiten returns 403 with an EMPTY body for foreign/missing cards.
          Mock::given(method("GET"))
              .and(path("/cards/999"))
              .respond_with(ResponseTemplate::new(403))
              .mount(&server)
              .await;

          let mcp = mcp_for(&server);
          let err = mcp
              .get_card(Parameters(GetCardParams { card_id: 999 }))
              .await
              .unwrap_err();
          assert!(
              err.message.contains("API error 403"),
              "expected KaitenError text in tool error, got: {}",
              err.message
          );
      }

      #[test]
      fn registers_exactly_8_read_only_tools() {
          let tools = KaitenMcp::tool_router().list_all();
          let mut names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();
          names.sort();
          assert_eq!(
              names,
              vec![
                  "current_user",
                  "get_board",
                  "get_card",
                  "list_boards",
                  "list_cards",
                  "list_checklists",
                  "list_comments",
                  "list_spaces",
              ]
          );
      }
  }
  ```

  (`mod mcp;` в `main.rs` уже объявлен с Task 10 — ничего подключать не нужно.)

  Run: `cargo test -p kaiten mcp -- --nocapture`
  Expected: FAIL — compile error `error[E0432]: unresolved import super::KaitenMcp` (и/или `E0433` про `GetCardParams`).

- [ ] **Step 4: Реализация 8 read-only инструментов + безусловный wiring main.rs**

  **(а)** Заменить в `crates/kaiten/src/mcp/mod.rs` ВСЁ, что стоит перед `#[cfg(test)] mod tests`,
  на блок ниже. Заглушка Task 10 (строки `use crate::cli::McpCmd;`, `use crate::error::CliError;`
  и вся `pub async fn run(cmd: McpCmd)`) при этом УДАЛЯЕТСЯ; итоговый файл = блок ниже +
  тестовый модуль из Step 3:

  ```rust
  use std::sync::Arc;

  use kaiten_client::{CardFilter, KaitenClient, KaitenError};
  use rmcp::handler::server::router::tool::ToolRouter;
  use rmcp::handler::server::wrapper::Parameters;
  use rmcp::model::{CallToolResult, Content, ServerCapabilities, ServerInfo};
  use rmcp::{ErrorData as McpError, ServerHandler, tool, tool_handler, tool_router};

  use crate::error::CliError;

  #[derive(Clone)]
  pub struct KaitenMcp {
      client: Arc<KaitenClient>,
      tool_router: ToolRouter<Self>,
  }

  fn to_mcp_error(err: KaitenError) -> McpError {
      // RateLimited already renders as "rate limited, retry after Ns".
      McpError::internal_error(err.to_string(), None)
  }

  fn json_result<T: serde::Serialize>(value: &T) -> Result<CallToolResult, McpError> {
      let text = serde_json::to_string_pretty(value)
          .map_err(|e| McpError::internal_error(e.to_string(), None))?;
      Ok(CallToolResult::success(vec![Content::text(text)]))
  }

  #[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
  pub struct ListBoardsParams {
      /// Space id to list boards from (see list_spaces)
      pub space_id: u64,
  }

  #[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
  pub struct GetBoardParams {
      /// Board id
      pub board_id: u64,
  }

  #[derive(Debug, Default, serde::Deserialize, schemars::JsonSchema)]
  pub struct ListCardsParams {
      /// Filter by space id
      pub space_id: Option<u64>,
      /// Filter by board id
      pub board_id: Option<u64>,
      /// Filter by column id
      pub column_id: Option<u64>,
      /// Full-text search query
      pub query: Option<String>,
      /// Filter by member user id
      pub member_id: Option<u64>,
      /// Filter by tag name
      pub tag: Option<String>,
      /// Filter by card type id
      pub type_id: Option<u64>,
      /// Include archived cards
      pub archived: Option<bool>,
      /// Max number of cards to return (default 50)
      pub limit: Option<u32>,
  }

  #[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
  pub struct GetCardParams {
      /// Card id
      pub card_id: u64,
  }

  #[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
  pub struct ListCommentsParams {
      /// Card id
      pub card_id: u64,
  }

  #[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
  pub struct ListChecklistsParams {
      /// Card id
      pub card_id: u64,
  }

  #[tool_router]
  impl KaitenMcp {
      pub fn new(client: Arc<KaitenClient>) -> Self {
          Self {
              client,
              tool_router: Self::tool_router(),
          }
      }

      #[tool(description = "Get the current authenticated Kaiten user (id, name, email).")]
      async fn current_user(&self) -> Result<CallToolResult, McpError> {
          let user = self.client.users().current().await.map_err(to_mcp_error)?;
          json_result(&user)
      }

      #[tool(description = "List all Kaiten spaces visible to the current user.")]
      async fn list_spaces(&self) -> Result<CallToolResult, McpError> {
          let spaces = self.client.spaces().list().await.map_err(to_mcp_error)?;
          json_result(&spaces)
      }

      #[tool(description = "List boards in a space.")]
      async fn list_boards(
          &self,
          Parameters(p): Parameters<ListBoardsParams>,
      ) -> Result<CallToolResult, McpError> {
          let boards = self
              .client
              .boards()
              .list(p.space_id)
              .await
              .map_err(to_mcp_error)?;
          json_result(&boards)
      }

      #[tool(
          description = "Get a board with its columns and lanes. Use it to discover column/lane ids before creating or moving cards."
      )]
      async fn get_board(
          &self,
          Parameters(p): Parameters<GetBoardParams>,
      ) -> Result<CallToolResult, McpError> {
          let board = self
              .client
              .boards()
              .get(p.board_id)
              .await
              .map_err(to_mcp_error)?;
          json_result(&board)
      }

      #[tool(
          description = "Search and list cards with optional filters. Returned cards have no description/members/checklists; call get_card for full details."
      )]
      async fn list_cards(
          &self,
          Parameters(p): Parameters<ListCardsParams>,
      ) -> Result<CallToolResult, McpError> {
          let filter = CardFilter {
              space_id: p.space_id,
              board_id: p.board_id,
              column_id: p.column_id,
              query: p.query,
              member_ids: p.member_id.into_iter().collect(),
              tag: p.tag,
              type_id: p.type_id,
              archived: p.archived,
              limit: Some(p.limit.unwrap_or(50)),
              ..Default::default()
          };
          let cards = self.client.cards().list(&filter).await.map_err(to_mcp_error)?;
          json_result(&cards)
      }

      #[tool(
          description = "Get a full card by id: description, members, tags, checklists with items, custom properties."
      )]
      async fn get_card(
          &self,
          Parameters(p): Parameters<GetCardParams>,
      ) -> Result<CallToolResult, McpError> {
          let card = self.client.cards().get(p.card_id).await.map_err(to_mcp_error)?;
          json_result(&card)
      }

      #[tool(description = "List all comments of a card.")]
      async fn list_comments(
          &self,
          Parameters(p): Parameters<ListCommentsParams>,
      ) -> Result<CallToolResult, McpError> {
          let comments = self
              .client
              .comments()
              .list(p.card_id)
              .await
              .map_err(to_mcp_error)?;
          json_result(&comments)
      }

      #[tool(description = "List checklists of a card, including their items.")]
      async fn list_checklists(
          &self,
          Parameters(p): Parameters<ListChecklistsParams>,
      ) -> Result<CallToolResult, McpError> {
          // GET /cards/{id}/checklists does not exist in the Kaiten API (405);
          // checklists come embedded in the full card.
          let card = self.client.cards().get(p.card_id).await.map_err(to_mcp_error)?;
          json_result(&card.checklists)
      }
  }

  #[tool_handler]
  impl ServerHandler for KaitenMcp {
      fn get_info(&self) -> ServerInfo {
          ServerInfo {
              instructions: Some(
                  "Kaiten tracker tools: browse spaces, boards and cards, create and edit \
                   cards, manage comments and checklists. Start with list_spaces to discover \
                   structure, or list_cards with filters to find work items."
                      .into(),
              ),
              capabilities: ServerCapabilities::builder().enable_tools().build(),
              ..Default::default()
          }
      }
  }

  pub async fn serve(client: KaitenClient) -> Result<(), CliError> {
      use rmcp::ServiceExt;

      let server = KaitenMcp::new(Arc::new(client));
      let service = server
          .serve(rmcp::transport::stdio())
          .await
          .map_err(|e| CliError::Io(std::io::Error::other(e)))?;
      service
          .waiting()
          .await
          .map_err(|e| CliError::Io(std::io::Error::other(e)))?;
      Ok(())
  }
  ```

  **(б)** Wiring в `crates/kaiten/src/main.rs` — БЕЗУСЛОВНО, в этом же шаге, ДО первого
  `cargo build` задачи (функции `mcp::run` больше нет — без этой правки крейт не собирается).
  Две правки:

  1. Заменить строку импорта:

  ```rust
  use crate::cli::{Cli, Commands};
  ```

  на:

  ```rust
  use crate::cli::{Cli, Commands, McpCmd};
  ```

  2. Заменить ветку `Commands::Mcp(cmd) => mcp::run(cmd).await,` в match функции `run` на:

  ```rust
          Commands::Mcp(McpCmd::Serve) => {
              let resolved = config::resolve()?;
              let client = kaiten_client::KaitenClient::new(&resolved.base_url, &resolved.token)?;
              mcp::serve(client).await
          }
  ```

  Остальной `main.rs` (включая ветку `Commands::Completion { .. } | Commands::Auth(_) |
  Commands::Mcp(_) => unreachable!("handled above")` во вложенном match) НЕ меняется.

- [ ] **Step 5: ADAPTATION POINT — первый cargo build задачи (после wiring'а)**

  Run: `cargo build -p kaiten`
  Expected: PASS — компиляция с rmcp 2.x.

  Макросы `#[tool]`/`#[tool_router]`/`#[tool_handler]` и обёртка `Parameters<T>` подтверждены
  для rmcp 2.2.0 по docs.rs; если минорная версия сместила идентификаторы — свериться с
  `https://docs.rs/rmcp/<версия из Cargo.lock>` и менять ТОЛЬКО rmcp-идентификаторы;
  имена и параметры 16 инструментов из контракта §5 неизменны.

- [ ] **Step 6: Прогнать тесты модуля**

  Run: `cargo test -p kaiten mcp -- --nocapture`
  Expected: PASS — 3 теста (`list_spaces_returns_spaces_json`, `get_card_403_maps_to_tool_error_with_api_text`, `registers_exactly_8_read_only_tools`).

- [ ] **Step 7: Smoke-проверка сабкоманды**

  Run: `cargo run -p kaiten -- mcp --help`
  Expected: PASS — help показывает сабкоманду `serve` (clap-дерево из Task 10, диспатч — из Step 4).

- [ ] **Step 8: Полный прогон и коммит**

  Run: `cargo test -p kaiten && cargo clippy --all-targets -- -D warnings`
  Expected: PASS, clippy чистый.

  Commit:
  ```
  git add crates/kaiten/Cargo.toml Cargo.lock crates/kaiten/src/mcp/mod.rs crates/kaiten/src/main.rs crates/kaiten/tests/fixtures/mcp_spaces.json
  git commit -m "feat(mcp): stdio MCP server with read-only kaiten tools"
  ```

---

### Task 22: MCP mutation-инструменты + `mine` в list_cards

**Files:**
- Modify: crates/kaiten/src/mcp/mod.rs — +8 инструментов, `mine` в `ListCardsParams`, замена теста «8 инструментов» на «16»
- Create: crates/kaiten/tests/fixtures/mcp_card_create.json
- Create: crates/kaiten/tests/fixtures/mcp_user_current.json
- Test: те же unit-тесты в crates/kaiten/src/mcp/mod.rs

**Interfaces:**
- Consumes (из Tasks 1–20, сигнатуры §2):
  - `client.cards().create(&CreateCard) -> Result<Card>`, `client.cards().update(card_id, &UpdateCard) -> Result<Card>`
  - `client.members().add(card_id, user_id) -> Result<CardMember>`, `client.members().remove(card_id, user_id) -> Result<()>`
  - `client.comments().add(card_id, text) -> Result<Comment>`
  - `client.checklists().add_item(card_id, checklist_id, text) -> Result<ChecklistItem>`, `client.checklists().set_item_checked(card_id, checklist_id, item_id, checked) -> Result<ChecklistItem>`
  - `client.users().current() -> Result<User>` (для `mine`)
  - `kaiten_client::{CreateCard, UpdateCard}`
- Produces: полный набор из 16 инструментов §5 (нужен Task 23 для stdio-проверки `tools/list`)

- [ ] **Step 1: Фикстуры (урезанные реальные ответы)**

  `crates/kaiten/tests/fixtures/mcp_card_create.json` — ответ `POST /cards` (все поля, которые парсит `Card`, присутствующие в реальном ответе, + 5 лишних: sort_order, fifo_order, version, expires_later, source):

  ```json
  {
    "id": 67089469,
    "uid": "c78e313c-ab37-4456-9eb0-904681c4e309",
    "title": "test card from cli",
    "description": null,
    "asap": false,
    "archived": false,
    "condition": 1,
    "state": 1,
    "board_id": 1826109,
    "column_id": 6308511,
    "lane_id": 2293584,
    "type_id": 1,
    "owner_id": 1068514,
    "created": "2026-07-09T15:17:59.905Z",
    "updated": "2026-07-09T15:17:59.905Z",
    "due_date": null,
    "comments_total": 0,
    "properties": null,
    "checklists": [],
    "sort_order": 1.0689198905237203,
    "fifo_order": null,
    "version": 1,
    "expires_later": false,
    "source": "api"
  }
  ```

  `crates/kaiten/tests/fixtures/mcp_user_current.json` — ответ `GET /users/current` (поля `User` + 5 лишних; email обезличен):

  ```json
  {
    "id": 1068514,
    "uid": "0abd61ea-9dc5-40eb-b0a9-0d452ba1d8a6",
    "full_name": "dxmuser",
    "username": "dxmuser",
    "email": "user@example.com",
    "activated": true,
    "lng": "ru",
    "timezone": "UTC",
    "theme": "auto",
    "company_id": 398610,
    "ui_version": 2
  }
  ```

- [ ] **Step 2: Failing tests — create_card, mine, 16 инструментов**

  В `crates/kaiten/src/mcp/mod.rs`, в модуле `tests`:

  1. Расширить импорты wiremock-матчеров (заменить строку `use wiremock::matchers::{header, method, path};`):

  ```rust
      use wiremock::matchers::{body_json, header, method, path, query_param};
  ```

  2. Заменить строку `use super::{GetCardParams, KaitenMcp};` на:

  ```rust
      use super::{CreateCardParams, GetCardParams, KaitenMcp, ListCardsParams};
  ```

  3. Добавить константы фикстур рядом с `SPACES_FIXTURE`:

  ```rust
      const CARD_CREATE_FIXTURE: &str =
          include_str!("../../tests/fixtures/mcp_card_create.json");
      const USER_CURRENT_FIXTURE: &str =
          include_str!("../../tests/fixtures/mcp_user_current.json");
  ```

  4. УДАЛИТЬ целиком тест `registers_exactly_8_read_only_tools` и добавить вместо него три новых теста:

  ```rust
      #[tokio::test]
      async fn create_card_sends_exact_body() {
          let server = MockServer::start().await;
          // None-fields must be skipped: the body is exactly board_id + title.
          Mock::given(method("POST"))
              .and(path("/cards"))
              .and(header("Authorization", "Bearer test-token"))
              .and(body_json(serde_json::json!({
                  "board_id": 1826109,
                  "title": "from mcp"
              })))
              .respond_with(ResponseTemplate::new(200).set_body_string(CARD_CREATE_FIXTURE))
              .expect(1)
              .mount(&server)
              .await;

          let mcp = mcp_for(&server);
          let result = mcp
              .create_card(Parameters(CreateCardParams {
                  board_id: 1826109,
                  title: "from mcp".to_string(),
                  column_id: None,
                  lane_id: None,
                  description: None,
                  type_id: None,
                  asap: None,
              }))
              .await
              .unwrap();
          let value: serde_json::Value = serde_json::from_str(&tool_text(&result)).unwrap();
          assert_eq!(value["id"], 67089469);
          assert_eq!(value["board_id"], 1826109);
      }

      #[tokio::test]
      async fn list_cards_mine_resolves_current_user_to_member_filter() {
          let server = MockServer::start().await;
          Mock::given(method("GET"))
              .and(path("/users/current"))
              .and(header("Authorization", "Bearer test-token"))
              .respond_with(ResponseTemplate::new(200).set_body_string(USER_CURRENT_FIXTURE))
              .expect(1)
              .mount(&server)
              .await;
          Mock::given(method("GET"))
              .and(path("/cards"))
              .and(query_param("member_ids", "1068514"))
              .and(query_param("limit", "50"))
              .respond_with(ResponseTemplate::new(200).set_body_string("[]"))
              .expect(1)
              .mount(&server)
              .await;

          let mcp = mcp_for(&server);
          let result = mcp
              .list_cards(Parameters(ListCardsParams {
                  mine: Some(true),
                  ..Default::default()
              }))
              .await
              .unwrap();
          assert_eq!(tool_text(&result).trim(), "[]");
      }

      #[test]
      fn registers_exactly_16_tools_with_spec_names() {
          let tools = KaitenMcp::tool_router().list_all();
          let mut names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();
          names.sort();
          let mut expected = vec![
              "current_user",
              "list_spaces",
              "list_boards",
              "get_board",
              "list_cards",
              "get_card",
              "create_card",
              "update_card",
              "move_card",
              "add_card_member",
              "remove_card_member",
              "list_comments",
              "add_comment",
              "list_checklists",
              "add_checklist_item",
              "set_checklist_item_checked",
          ];
          expected.sort_unstable();
          assert_eq!(names, expected);
      }
  ```

  Run: `cargo test -p kaiten mcp -- --nocapture`
  Expected: FAIL — compile error `error[E0432]: unresolved import ... CreateCardParams` (полей `mine` и методов-мутаторов ещё нет).

- [ ] **Step 3: Реализация мутаций**

  В `crates/kaiten/src/mcp/mod.rs`:

  1. Заменить строку импорта клиента:

  ```rust
  use kaiten_client::{CardFilter, CreateCard, KaitenClient, KaitenError, UpdateCard};
  ```

  2. В `ListCardsParams` добавить поле (после `member_id`):

  ```rust
      /// If true, only cards where the current user is a member
      pub mine: Option<bool>,
  ```

  3. Заменить тело метода `list_cards` целиком:

  ```rust
      async fn list_cards(
          &self,
          Parameters(p): Parameters<ListCardsParams>,
      ) -> Result<CallToolResult, McpError> {
          let mut member_ids: Vec<u64> = p.member_id.into_iter().collect();
          if p.mine == Some(true) {
              let me = self.client.users().current().await.map_err(to_mcp_error)?;
              if !member_ids.contains(&me.id) {
                  member_ids.push(me.id);
              }
          }
          let filter = CardFilter {
              space_id: p.space_id,
              board_id: p.board_id,
              column_id: p.column_id,
              query: p.query,
              member_ids,
              tag: p.tag,
              type_id: p.type_id,
              archived: p.archived,
              limit: Some(p.limit.unwrap_or(50)),
              ..Default::default()
          };
          let cards = self.client.cards().list(&filter).await.map_err(to_mcp_error)?;
          json_result(&cards)
      }
  ```

  4. Добавить params-структуры (после `ListChecklistsParams`):

  ```rust
  #[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
  pub struct CreateCardParams {
      /// Board id to create the card on
      pub board_id: u64,
      /// Card title
      pub title: String,
      /// Column id (defaults to the first column of the board)
      pub column_id: Option<u64>,
      /// Lane id (defaults to the first lane of the board)
      pub lane_id: Option<u64>,
      /// Card description (markdown)
      pub description: Option<String>,
      /// Card type id (see the board's default_card_type_id)
      pub type_id: Option<u64>,
      /// Mark the card as ASAP
      pub asap: Option<bool>,
  }

  #[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
  pub struct UpdateCardParams {
      /// Card id
      pub card_id: u64,
      /// New title
      pub title: Option<String>,
      /// New description (markdown)
      pub description: Option<String>,
      /// New card type id
      pub type_id: Option<u64>,
      /// Set or clear the ASAP flag
      pub asap: Option<bool>,
  }

  #[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
  pub struct MoveCardParams {
      /// Card id
      pub card_id: u64,
      /// Target column id (see get_board)
      pub column_id: u64,
      /// Target lane id
      pub lane_id: Option<u64>,
      /// Target board id (for cross-board moves)
      pub board_id: Option<u64>,
  }

  #[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
  pub struct CardMemberParams {
      /// Card id
      pub card_id: u64,
      /// User id (see current_user or list_cards owners)
      pub user_id: u64,
  }

  #[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
  pub struct AddCommentParams {
      /// Card id
      pub card_id: u64,
      /// Comment text (markdown)
      pub text: String,
  }

  #[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
  pub struct AddChecklistItemParams {
      /// Card id
      pub card_id: u64,
      /// Checklist id (see list_checklists)
      pub checklist_id: u64,
      /// Item text
      pub text: String,
  }

  #[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
  pub struct SetChecklistItemCheckedParams {
      /// Card id
      pub card_id: u64,
      /// Checklist id (see list_checklists)
      pub checklist_id: u64,
      /// Checklist item id
      pub item_id: u64,
      /// true to check, false to uncheck
      pub checked: bool,
  }
  ```

  5. Добавить методы в конец `#[tool_router] impl KaitenMcp` (перед закрывающей скобкой блока):

  ```rust
      #[tool(description = "Create a new card on a board. Returns the created card as JSON.")]
      async fn create_card(
          &self,
          Parameters(p): Parameters<CreateCardParams>,
      ) -> Result<CallToolResult, McpError> {
          let req = CreateCard {
              board_id: p.board_id,
              title: p.title,
              column_id: p.column_id,
              lane_id: p.lane_id,
              description: p.description,
              type_id: p.type_id,
              asap: p.asap,
          };
          let card = self.client.cards().create(&req).await.map_err(to_mcp_error)?;
          json_result(&card)
      }

      #[tool(
          description = "Update card title, description, type or ASAP flag. Only provided fields are changed."
      )]
      async fn update_card(
          &self,
          Parameters(p): Parameters<UpdateCardParams>,
      ) -> Result<CallToolResult, McpError> {
          let req = UpdateCard {
              title: p.title,
              description: p.description,
              type_id: p.type_id,
              asap: p.asap,
              ..Default::default()
          };
          let card = self
              .client
              .cards()
              .update(p.card_id, &req)
              .await
              .map_err(to_mcp_error)?;
          json_result(&card)
      }

      #[tool(
          description = "Move a card to another column (and optionally lane or board). Use get_board to discover column and lane ids."
      )]
      async fn move_card(
          &self,
          Parameters(p): Parameters<MoveCardParams>,
      ) -> Result<CallToolResult, McpError> {
          let req = UpdateCard {
              column_id: Some(p.column_id),
              lane_id: p.lane_id,
              board_id: p.board_id,
              ..Default::default()
          };
          let card = self
              .client
              .cards()
              .update(p.card_id, &req)
              .await
              .map_err(to_mcp_error)?;
          json_result(&card)
      }

      #[tool(description = "Add a user to the card members by user id.")]
      async fn add_card_member(
          &self,
          Parameters(p): Parameters<CardMemberParams>,
      ) -> Result<CallToolResult, McpError> {
          let member = self
              .client
              .members()
              .add(p.card_id, p.user_id)
              .await
              .map_err(to_mcp_error)?;
          json_result(&member)
      }

      #[tool(description = "Remove a user from the card members by user id.")]
      async fn remove_card_member(
          &self,
          Parameters(p): Parameters<CardMemberParams>,
      ) -> Result<CallToolResult, McpError> {
          self.client
              .members()
              .remove(p.card_id, p.user_id)
              .await
              .map_err(to_mcp_error)?;
          json_result(&serde_json::json!({ "removed": true, "user_id": p.user_id }))
      }

      #[tool(description = "Add a comment to a card.")]
      async fn add_comment(
          &self,
          Parameters(p): Parameters<AddCommentParams>,
      ) -> Result<CallToolResult, McpError> {
          let comment = self
              .client
              .comments()
              .add(p.card_id, &p.text)
              .await
              .map_err(to_mcp_error)?;
          json_result(&comment)
      }

      #[tool(description = "Add an item to an existing checklist on a card.")]
      async fn add_checklist_item(
          &self,
          Parameters(p): Parameters<AddChecklistItemParams>,
      ) -> Result<CallToolResult, McpError> {
          let item = self
              .client
              .checklists()
              .add_item(p.card_id, p.checklist_id, &p.text)
              .await
              .map_err(to_mcp_error)?;
          json_result(&item)
      }

      #[tool(description = "Check or uncheck a checklist item on a card.")]
      async fn set_checklist_item_checked(
          &self,
          Parameters(p): Parameters<SetChecklistItemCheckedParams>,
      ) -> Result<CallToolResult, McpError> {
          let item = self
              .client
              .checklists()
              .set_item_checked(p.card_id, p.checklist_id, p.item_id, p.checked)
              .await
              .map_err(to_mcp_error)?;
          json_result(&item)
      }
  ```

  6. Обновить `instructions` в `get_info` (заменить строковый литерал целиком):

  ```rust
              instructions: Some(
                  "Kaiten tracker tools: browse spaces, boards and cards, create and edit \
                   cards, manage members, comments and checklists. Start with list_spaces \
                   to discover structure, or list_cards with mine=true to see the current \
                   user's cards."
                      .into(),
              ),
  ```

- [ ] **Step 4: Прогнать тесты**

  Run: `cargo test -p kaiten mcp -- --nocapture`
  Expected: PASS — 5 тестов (`list_spaces_returns_spaces_json`, `get_card_403_maps_to_tool_error_with_api_text`, `create_card_sends_exact_body`, `list_cards_mine_resolves_current_user_to_member_filter`, `registers_exactly_16_tools_with_spec_names`).

- [ ] **Step 5: Линт и коммит**

  Run: `cargo test -p kaiten && cargo clippy --all-targets -- -D warnings`
  Expected: PASS, clippy чистый.

  Commit:
  ```
  git add crates/kaiten/src/mcp/mod.rs crates/kaiten/tests/fixtures/mcp_card_create.json crates/kaiten/tests/fixtures/mcp_user_current.json
  git commit -m "feat(mcp): mutation tools and mine filter, 16 tools total"
  ```

---

### Task 23: MCP smoke через stdio (реальный процесс, JSON-RPC)

**Files:**
- Create: crates/kaiten/tests/mcp_stdio_test.rs
- Test: он же (интеграционный)

**Interfaces:**
- Consumes: бинарник `kaiten` с рабочей сабкомандой `mcp serve` (Task 21–22); env-контракт конфига (`KAITEN_BASE_URL`, `KAITEN_TOKEN`, `KAITEN_CONFIG_DIR`)
- Produces: доказательство, что сервер отвечает на `initialize` и отдаёт все 16 инструментов через настоящий stdio-транспорт (защита от регрессий wiring'а в main.rs)

- [ ] **Step 1: Написать интеграционный тест**

  Тест сознательно НЕ использует tokio: только `std::process` + потоки + каналы, чтобы проверить сервер как чёрный ящик. `initialize`/`tools/list` не ходят в Kaiten API, поэтому base_url указывает на заведомо недоступный адрес.

  Создать `crates/kaiten/tests/mcp_stdio_test.rs`:

  ```rust
  //! End-to-end smoke test: spawns `kaiten mcp serve` as a real process and
  //! speaks JSON-RPC over stdio. Deliberately no tokio here.

  use std::io::{BufRead, BufReader, Write};
  use std::process::{Child, ChildStdin, Command, Stdio};
  use std::sync::mpsc;
  use std::thread;
  use std::time::Duration;

  const READ_TIMEOUT: Duration = Duration::from_secs(20);

  const EXPECTED_TOOLS: [&str; 16] = [
      "current_user",
      "list_spaces",
      "list_boards",
      "get_board",
      "list_cards",
      "get_card",
      "create_card",
      "update_card",
      "move_card",
      "add_card_member",
      "remove_card_member",
      "list_comments",
      "add_comment",
      "list_checklists",
      "add_checklist_item",
      "set_checklist_item_checked",
  ];

  struct McpProc {
      child: Child,
      stdin: ChildStdin,
      lines: mpsc::Receiver<String>,
  }

  impl McpProc {
      fn spawn() -> McpProc {
          let config_dir =
              std::env::temp_dir().join(format!("kaiten-mcp-smoke-{}", std::process::id()));
          std::fs::create_dir_all(&config_dir).unwrap();

          let mut child = Command::new(assert_cmd::cargo::cargo_bin("kaiten"))
              .args(["mcp", "serve"])
              // initialize/tools/list never call the Kaiten API,
              // so an unreachable base url is fine here.
              .env("KAITEN_BASE_URL", "http://127.0.0.1:9")
              .env("KAITEN_TOKEN", "test-token")
              .env("KAITEN_CONFIG_DIR", &config_dir)
              .env("NO_COLOR", "1")
              .env_remove("RUST_LOG")
              .stdin(Stdio::piped())
              .stdout(Stdio::piped())
              .stderr(Stdio::null())
              .spawn()
              .expect("failed to spawn `kaiten mcp serve`");

          let stdin = child.stdin.take().unwrap();
          let stdout = child.stdout.take().unwrap();
          let (tx, rx) = mpsc::channel();
          thread::spawn(move || {
              for line in BufReader::new(stdout).lines() {
                  match line {
                      Ok(l) => {
                          if tx.send(l).is_err() {
                              break;
                          }
                      }
                      Err(_) => break,
                  }
              }
          });

          McpProc { child, stdin, lines: rx }
      }

      fn send(&mut self, msg: &serde_json::Value) {
          let mut line = msg.to_string();
          line.push('\n');
          self.stdin.write_all(line.as_bytes()).unwrap();
          self.stdin.flush().unwrap();
      }

      /// Reads stdout lines until a JSON-RPC response with the given id arrives.
      fn read_response(&self, id: u64) -> serde_json::Value {
          loop {
              let line = self
                  .lines
                  .recv_timeout(READ_TIMEOUT)
                  .expect("timed out waiting for MCP response on stdout");
              let value: serde_json::Value = match serde_json::from_str(&line) {
                  Ok(v) => v,
                  Err(_) => continue, // ignore non-JSON noise
              };
              if value["id"] == serde_json::json!(id) {
                  return value;
              }
          }
      }
  }

  impl Drop for McpProc {
      fn drop(&mut self) {
          let _ = self.child.kill();
          let _ = self.child.wait();
      }
  }

  #[test]
  fn mcp_stdio_initialize_and_list_all_16_tools() {
      let mut mcp = McpProc::spawn();

      mcp.send(&serde_json::json!({
          "jsonrpc": "2.0",
          "id": 1,
          "method": "initialize",
          "params": {
              "protocolVersion": "2025-03-26",
              "capabilities": {},
              "clientInfo": { "name": "smoke-test", "version": "0.0.0" }
          }
      }));
      let init = mcp.read_response(1);
      assert!(
          init["result"]["capabilities"]["tools"].is_object(),
          "server must advertise tools capability, got: {init}"
      );

      mcp.send(&serde_json::json!({
          "jsonrpc": "2.0",
          "method": "notifications/initialized"
      }));

      mcp.send(&serde_json::json!({
          "jsonrpc": "2.0",
          "id": 2,
          "method": "tools/list"
      }));
      let listed = mcp.read_response(2);
      let tools = listed["result"]["tools"]
          .as_array()
          .unwrap_or_else(|| panic!("tools/list must return an array, got: {listed}"));

      let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
      assert_eq!(names.len(), 16, "expected exactly 16 tools, got: {names:?}");
      for expected in EXPECTED_TOOLS {
          assert!(names.contains(&expected), "missing tool `{expected}`, got: {names:?}");
      }

      for tool in tools {
          assert!(
              tool["description"].as_str().is_some_and(|d| !d.is_empty()),
              "tool without description: {tool}"
          );
          assert!(tool["inputSchema"].is_object(), "tool without inputSchema: {tool}");
      }
  }
  ```

- [ ] **Step 2: Прогнать тест**

  Run: `cargo test -p kaiten --test mcp_stdio_test -- --nocapture`
  Expected: PASS (это smoke поверх готовых Task 21–22; тест «падал» бы до них отсутствием сабкоманды). Если FAIL по таймауту — перезапустить с видимым stderr процесса: временно заменить `Stdio::null()` на `Stdio::inherit()` в строке `.stderr(...)`, диагностировать (обычно: сервер упал на конфиге или пишет логи в stdout), починить, вернуть `Stdio::null()`.

- [ ] **Step 3: Полный прогон и коммит**

  Run: `cargo test -p kaiten`
  Expected: PASS — все тесты крейта, включая новый.

  Commit:
  ```
  git add crates/kaiten/tests/mcp_stdio_test.rs
  git commit -m "test(mcp): stdio smoke test covering initialize and tools/list"
  ```

---

### Task 24: Живой smoke на dstest.kaiten.ru (сценарий, НЕ cargo-тест)

**Files:**
- Modify (ТОЛЬКО при обнаружении расхождений с реальным API): crates/kaiten-client/src/models.rs, crates/kaiten-client/tests/fixtures/*.json, crates/kaiten-client/tests/*_test.rs
- Test: ручной сценарий ниже; `.env.test` в корне репозитория (в .gitignore, содержит `KAITEN_DOMAIN=dstest` и реальный `KAITEN_TOKEN`)

**Interfaces:**
- Consumes: собранный release-бинарник со ВСЕЙ функциональностью Tasks 1–23
- Produces: подтверждение соответствия моделей реальному API; при расхождениях — исправленные модели/фикстуры/тесты

Реальные ID на dstest: user `1068514` (dxmuser), space `810671` (kaiten-cli-test), board `1826109`.

**Это РУЧНАЯ верификация ВНЕ TDD-цикла**: шаги выполняются последовательно оператором,
задача не добавляет cargo-тестов; failing test → PASS здесь не применяется.

**Протокол при ЛЮБОМ расхождении** (обязателен для каждого шага): если команда падает с
`failed to decode response at ...` — зафиксировать точный JSON-path из ошибки; если вывод
не совпадает с ожидаемым — перезапустить команду с `-vv` и изучить тело ответа в trace-логе.
Затем СТОП по сценарию. Разрешены ТОЛЬКО четыре класса правок моделей:

1. сделать поле `Option<...>`;
2. добавить `#[serde(default)]`;
3. поправить тип поля (например, `u32` ↔ `u64`/`i64`; кастомные десериализаторы НЕ вводить — только смена типа);
4. добавить/поправить `#[serde(rename = "...")]`.

Всё, что сложнее (новая структура, кастомный deserializer, изменение семантики поля,
другой формат ответа), — ОСТАНОВИТЬСЯ, код не править и доложить пользователю отдельным
сообщением с diff-предложением; продолжать только после его решения.

Каждая правка = модель в `crates/kaiten-client/src/models.rs` + фикстура в
`crates/kaiten-client/tests/fixtures/` (привести к реальному ответу) + wiremock-тест +
прогон `cargo test -p kaiten-client` (PASS) + ОТДЕЛЬНЫЙ коммит:
```
git add crates/kaiten-client/src/models.rs crates/kaiten-client/tests/
git commit -m "fix(client): align <Model> with real api"
```
(вместо `<Model>` — имя структуры, например `Card`; одна правка — один коммит). После этого пересобрать (`cargo build --release`) и ПОВТОРИТЬ упавший шаг.

Все команды сценария запускаются с `-v`: после каждой проверять, что stderr содержит debug-строку HTTP-запроса (method, path, status, elapsed_ms) и НЕ содержит токен.

- [ ] **Step 1: Сборка и окружение**

  Run:
  ```
  cargo build --release
  source .env.test
  ```
  Expected: бинарник `./target/release/kaiten`; env содержит `KAITEN_DOMAIN=dstest`, `KAITEN_TOKEN=<реальный токен>`.

- [ ] **Step 2: auth status**

  Run: `./target/release/kaiten -v auth status`
  Expected: stdout — домен `dstest`, пользователь `dxmuser` (id 1068514), источник токена `env`; stderr — debug-лог `GET /users/current` со статусом 200.

- [ ] **Step 3: space list**

  Run: `./target/release/kaiten -v space list`
  Expected: таблица пространств, среди строк — `810671 ... kaiten-cli-test`.

- [ ] **Step 4: board view**

  Run: `./target/release/kaiten -v board view 1826109`
  Expected: доска `test-board` с таблицами колонок и дорожек; как минимум колонка `To Do` (id `6308511`) и дорожка `Default Lane` (id `2293584`). Записать id первой колонки:
  ```
  COLUMN_ID=6308511
  ```
  (если на доске уже есть другие колонки — можно взять любой id из вывода).

- [ ] **Step 5: card create**

  Run:
  ```
  CARD_ID=$(./target/release/kaiten --json card create --board 1826109 --title "smoke-$(date +%s)" \
    | python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])')
  echo "$CARD_ID"
  ```
  Expected: числовой id новой карточки (без ошибок Decode).

- [ ] **Step 6: card view**

  Run: `./target/release/kaiten -v card view "$CARD_ID"`
  Expected: карточка с заголовком `smoke-<timestamp>`, board 1826109, column To Do; секции members/tags/checklists пустые.

- [ ] **Step 7: comment add + list**

  Run:
  ```
  ./target/release/kaiten -v card comment add "$CARD_ID" --body "smoke comment"
  ./target/release/kaiten -v card comment list "$CARD_ID"
  ```
  Expected: add — созданный комментарий; list — таблица с одной строкой `smoke comment`, автор dxmuser.

- [ ] **Step 8: checklist add + item add + check**

  Run:
  ```
  CL_ID=$(./target/release/kaiten --json card checklist add "$CARD_ID" --name "smoke checklist" \
    | python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])')
  ITEM_ID=$(./target/release/kaiten --json card checklist item add "$CARD_ID" "$CL_ID" --text "smoke item" \
    | python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])')
  ./target/release/kaiten -v card checklist item check "$CARD_ID" "$CL_ID" "$ITEM_ID"
  ./target/release/kaiten -v card checklist list "$CARD_ID"
  ```
  Expected: item check возвращает пункт с `checked: true`; checklist list показывает `smoke checklist` с отмеченным пунктом `smoke item`.

- [ ] **Step 9: tag add**

  Run:
  ```
  ./target/release/kaiten -v card tag add "$CARD_ID" smoke-tag
  ./target/release/kaiten --json card view "$CARD_ID" | python3 -c 'import json,sys; print([t["name"] for t in json.load(sys.stdin)["tags"]])'
  ```
  Expected: список тегов содержит `smoke-tag`.

- [ ] **Step 10: member add**

  Run: `./target/release/kaiten -v card member add "$CARD_ID" 1068514`
  Expected: участник dxmuser добавлен (успешный вывод, без Decode-ошибок).

- [ ] **Step 11: card move**

  Run: `./target/release/kaiten -v card move "$CARD_ID" --column "$COLUMN_ID"`
  Expected: успех; в выводе карточки `column_id` равен `$COLUMN_ID` (перемещение в ту же колонку допустимо — проверяем успешный PATCH и парсинг ответа).

- [ ] **Step 12: card archive + list --archived**

  Run:
  ```
  ./target/release/kaiten -v card archive "$CARD_ID"
  ./target/release/kaiten -v card list --board 1826109 --archived
  ```
  Expected: archive — карточка с `condition: 2`/archived; список архивных карточек доски содержит строку с `$CARD_ID`.

- [ ] **Step 13: raw api**

  Run: `./target/release/kaiten -v api GET /users/current`
  Expected: сырой JSON, содержит `"id": 1068514`.

- [ ] **Step 14: verbose-контроль логов и редактирование токена**

  Run:
  ```
  ./target/release/kaiten -vv card view "$CARD_ID" 2>debug.log >/dev/null
  grep -E "GET .*/cards/" debug.log | head -3
  grep -c "$KAITEN_TOKEN" debug.log
  rm debug.log
  ```
  Expected: первая grep-команда показывает debug/trace-строки запроса (method, path, status, elapsed); вторая печатает `0` — токен НИКОГДА не попадает в логи (если > 0 — это баг: СТОП, чинить редактирование заголовка в `crates/kaiten-client/src/client.rs` + тест, commit `fix(client): redact token in logs`).

- [ ] **Step 15: Итог**

  Если в Steps 2–14 были правки моделей/фикстур/логов: прогнать полный `cargo test` (Expected: PASS, все зелёные) и закоммитить финализацию:
  ```
  git add -A
  git commit -m "test: live smoke verified against real instance"
  ```
  Если расхождений не было — коммит не нужен; зафиксировать в выводе задачи, что все 14 шагов прошли без правок.

---

### Task 25: README.md + финальная полировка

**Files:**
- Create: README.md
- Modify (только если fmt/clippy найдут замечания): любые файлы `crates/**/*.rs` — форматирование и фиксы линта без изменения поведения
- Test: полный `cargo test` workspace

**Interfaces:**
- Consumes: всю готовую функциональность Tasks 1–24 (текст README описывает реально работающие команды)
- Produces: финальное состояние репозитория: README, чистые fmt/clippy, зелёный `cargo test`

- [ ] **Step 1: Написать README.md**

  Создать `README.md` в корне репозитория:

  ````markdown
  # kaiten

  Command-line client and MCP server for the [Kaiten](https://kaiten.ru) tracker,
  in the spirit of `gh` / `glab`.

  - Browse spaces, boards and cards from the terminal
  - Create, edit, move and archive cards; manage members, tags, comments and checklists
  - `--json` output on every command for scripting
  - Built-in MCP server (`kaiten mcp serve`) so coding agents can work with the tracker
  - Raw API escape hatch: `kaiten api GET /users/current`

  ## Install

  ```sh
  git clone <repo-url> kaiten-cli
  cd kaiten-cli
  cargo install --path crates/kaiten
  ```

  ## Authentication

  Create an API token in your Kaiten profile (`https://mycompany.kaiten.ru` →
  user profile → API tokens), then:

  ```sh
  kaiten auth login    # asks for the domain ("mycompany") and token, verifies them
  kaiten auth status   # shows domain, current user and where the token came from
  ```

  Environment variables override the config file:

  | Variable | Meaning |
  |---|---|
  | `KAITEN_TOKEN` | API token |
  | `KAITEN_DOMAIN` | company domain: `mycompany` → `https://mycompany.kaiten.ru/api/latest` |
  | `KAITEN_BASE_URL` | full API base URL (overrides the domain) |
  | `KAITEN_CONFIG_DIR` | config directory (default: `~/.config/kaiten`) |

  ## Configuration

  `~/.config/kaiten/config.toml` — created by `kaiten auth login` with mode 600:

  ```toml
  domain = "mycompany"
  token = "your-api-token"

  [defaults]      # optional: used when --space/--board flags are omitted
  space = 123
  board = 456
  ```

  ## Usage

  ```sh
  kaiten space list
  kaiten board list --space 123
  kaiten board view 456                    # columns and lanes (ids for `card move`)

  kaiten card list --mine
  kaiten card list --board 456 --query "deploy" --limit 20
  kaiten card view 67089469 --comments     # a full card URL works too
  kaiten card create --board 456 --title "Fix the flaky test" --description "..."
  kaiten card edit 67089469 --title "New title" --asap true
  kaiten card move 67089469 --column 6308511
  kaiten card archive 67089469

  kaiten card member add 67089469 user@example.com   # user id or email
  kaiten card comment add 67089469 --body "Done, please review"
  kaiten card checklist add 67089469 --name "Release steps"
  kaiten card checklist item add 67089469 91011 --text "Bump version"
  kaiten card checklist item check 67089469 91011 121314
  kaiten card tag add 67089469 backend

  kaiten tag list
  kaiten card-type list

  kaiten api GET "/cards?query=deploy&limit=5"                    # raw API access
  kaiten api POST /cards --data '{"board_id":456,"title":"Raw"}'
  ```

  Add `--json` to any command to print the raw JSON of the API response.

  ## Shell completion

  ```sh
  # zsh
  kaiten completion zsh > "${fpath[1]}/_kaiten"
  # bash
  kaiten completion bash > ~/.local/share/bash-completion/completions/kaiten
  # fish
  kaiten completion fish > ~/.config/fish/completions/kaiten.fish
  ```

  ## MCP server

  The same binary is an MCP server (stdio transport, 16 tools mirroring the CLI).

  Claude Code:

  ```sh
  claude mcp add kaiten -- kaiten mcp serve
  ```

  Any other MCP client:

  ```json
  {
    "mcpServers": {
      "kaiten": {
        "command": "kaiten",
        "args": ["mcp", "serve"]
      }
    }
  }
  ```

  Authentication is shared with the CLI: run `kaiten auth login` once, or export
  `KAITEN_DOMAIN` / `KAITEN_TOKEN` in the client configuration. Logs go to stderr
  only — stdout carries the MCP protocol.

  ## Debugging

  - `-v` — debug logs to stderr: every HTTP request with method, path, status, duration
  - `-vv` — trace logs including request/response bodies (the token is always redacted)
  - `RUST_LOG=kaiten_client=trace kaiten ...` — fine-grained filtering without flags
  - decode errors report the exact JSON path that failed to parse
  - `kaiten api <METHOD> <path> [--data <json>]` — raw access when a typed command is not enough
  - API error bodies are printed as-is together with the HTTP status

  ## Development

  ```sh
  cargo test
  cargo clippy --all-targets -- -D warnings
  cargo fmt --all -- --check
  ```
  ````

- [ ] **Step 2: Сверить примеры README с реальным CLI**

  Run:
  ```
  ./target/release/kaiten card --help
  ./target/release/kaiten card checklist item --help
  ./target/release/kaiten api --help
  ```
  Expected: флаги и порядок аргументов в README совпадают с help-текстами (`--body`, `--name`, `--text`, `--data`, позиционные `<checklist_id> <item_id>`). Любое расхождение — поправить README (не CLI).

- [ ] **Step 3: Форматирование**

  Run: `cargo fmt --all -- --check`
  Если есть diff — Run: `cargo fmt --all`, затем повторить `cargo fmt --all -- --check`.
  Expected: PASS (пустой вывод).

- [ ] **Step 4: Clippy начисто**

  Run: `cargo clippy --all-targets -- -D warnings`
  Если есть warnings — исправить каждый по подсказке clippy (без изменения поведения; типичные: `needless_borrow`, `redundant_clone`, `uninlined_format_args`) и повторять команду до чистого прогона.
  Expected: PASS, ноль warnings.

- [ ] **Step 5: Полный прогон тестов**

  Run: `cargo test`
  Expected: PASS — все тесты workspace зелёные (клиент, CLI, MCP unit, MCP stdio).

- [ ] **Step 6: Коммиты**

  Run:
  ```
  git add README.md
  git commit -m "docs: README"
  ```
  Затем, если Steps 3–4 меняли код:
  ```
  git add -A
  git commit -m "chore: fmt + clippy clean"
  ```
  Финальная проверка: `git status` → рабочее дерево чистое (кроме игнорируемых `.env.test`, `target/`).
