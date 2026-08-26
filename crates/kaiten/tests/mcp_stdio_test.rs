//! End-to-end smoke tests: spawn `kaiten mcp serve` as a real process and
//! speak JSON-RPC over stdio, so the real rmcp router, serialization and
//! transport are exercised. The child is always driven synchronously; tokio
//! appears only where a wiremock Kaiten API is needed.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const USER_CURRENT: &str = include_str!("fixtures/mcp_user_current.json");

const READ_TIMEOUT: Duration = Duration::from_secs(20);

const EXPECTED_TOOLS: [&str; 35] = [
    "current_user",
    "list_spaces",
    "list_boards",
    "get_board",
    "list_cards",
    "get_card",
    "create_card",
    "update_card",
    "move_card",
    "archive_card",
    "add_card_member",
    "remove_card_member",
    "list_users",
    "list_comments",
    "add_comment",
    "list_checklists",
    "add_checklist",
    "add_checklist_item",
    "set_checklist_item_checked",
    "add_card_tag",
    "remove_card_tag",
    "list_card_types",
    "poll_updates",
    "list_custom_properties",
    "list_property_select_values",
    "link_cards",
    "unlink_cards",
    "release_blocks",
    "attach_file",
    "detach_file",
    "update_comment",
    "remove_comment",
    "set_card_responsible",
    "add_time_log",
    "list_time_logs",
];

struct McpProc {
    child: Child,
    stdin: ChildStdin,
    lines: mpsc::Receiver<String>,
}

impl McpProc {
    fn spawn() -> McpProc {
        // initialize/tools/list never call the Kaiten API,
        // so an unreachable base url is fine here.
        McpProc::spawn_with_base_url("http://127.0.0.1:9")
    }

    fn spawn_with_base_url(base_url: &str) -> McpProc {
        let config_dir =
            std::env::temp_dir().join(format!("kaiten-mcp-smoke-{}", std::process::id()));
        std::fs::create_dir_all(&config_dir).unwrap();

        let mut child = Command::new(assert_cmd::cargo::cargo_bin("kaiten"))
            .args(["mcp", "serve"])
            .env("KAITEN_BASE_URL", base_url)
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

        McpProc {
            child,
            stdin,
            lines: rx,
        }
    }

    fn send(&mut self, msg: &serde_json::Value) {
        let mut line = msg.to_string();
        line.push('\n');
        self.stdin.write_all(line.as_bytes()).unwrap();
        self.stdin.flush().unwrap();
    }

    /// `initialize` (id 1) followed by `notifications/initialized`;
    /// returns the initialize response.
    fn initialize(&mut self, protocol_version: &str) -> serde_json::Value {
        self.send(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": protocol_version,
                "capabilities": {},
                "clientInfo": { "name": "smoke-test", "version": "0.0.0" }
            }
        }));
        let init = self.read_response(1);
        self.send(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }));
        init
    }

    fn call_tool(
        &mut self,
        id: u64,
        name: &str,
        arguments: &serde_json::Value,
    ) -> serde_json::Value {
        self.send(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": { "name": name, "arguments": arguments }
        }));
        self.read_response(id)
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
fn mcp_stdio_initialize_and_list_all_tools() {
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
    assert_eq!(
        names.len(),
        EXPECTED_TOOLS.len(),
        "expected exactly {} tools, got: {names:?}",
        EXPECTED_TOOLS.len()
    );
    for expected in EXPECTED_TOOLS {
        assert!(
            names.contains(&expected),
            "missing tool `{expected}`, got: {names:?}"
        );
    }

    for tool in tools {
        assert!(
            tool["description"].as_str().is_some_and(|d| !d.is_empty()),
            "tool without description: {tool}"
        );
        assert!(
            tool["inputSchema"].is_object(),
            "tool without inputSchema: {tool}"
        );
    }
}

/// Result keys rmcp 3 emits only to peers that negotiated protocol 2026-07-28
/// (SEP-2322 `resultType`, SEP-2549 cache hints). Legacy clients — Claude Code and
/// Claude Desktop speak 2025-03-26 / 2025-06-18 — must keep the rmcp 2.x shape.
const NEW_PROTOCOL_ONLY_KEYS: [&str; 3] = ["resultType", "ttlMs", "cacheScope"];

fn assert_legacy_shape(result: &serde_json::Value, what: &str) {
    for key in NEW_PROTOCOL_ONLY_KEYS {
        assert!(
            result.get(key).is_none(),
            "{what}: `{key}` must not be sent to a legacy-protocol client, got: {result}"
        );
    }
}

fn tool_text(response: &serde_json::Value) -> &str {
    response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("tool result must have a text content block, got: {response}"))
}

#[test]
fn mcp_stdio_echoes_known_legacy_protocol_versions() {
    for version in ["2025-03-26", "2025-06-18"] {
        let mut mcp = McpProc::spawn();
        let init = mcp.initialize(version);
        let result = &init["result"];
        assert_eq!(
            result["protocolVersion"],
            serde_json::json!(version),
            "server must echo a known client protocol version, got: {init}"
        );
        assert!(
            result["serverInfo"]["name"]
                .as_str()
                .is_some_and(|n| !n.is_empty()),
            "serverInfo.name missing: {init}"
        );
        assert!(
            result["serverInfo"]["version"]
                .as_str()
                .is_some_and(|v| !v.is_empty()),
            "serverInfo.version missing: {init}"
        );
        assert!(
            result["instructions"]
                .as_str()
                .is_some_and(|i| i.contains("Kaiten")),
            "instructions missing: {init}"
        );
        assert_legacy_shape(result, "initialize");
    }
}

/// Pins the `tools/call` contract for legacy clients through the real router:
/// invalid params and API failures are tool-level errors (`isError`), unknown
/// tools are JSON-RPC errors, successes carry JSON text — and nothing new leaks.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_stdio_tools_call_legacy_wire_contract() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/users/current"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(USER_CURRENT, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/cards/1"))
        .respond_with(
            ResponseTemplate::new(404)
                .set_body_raw(r#"{"message":"Card not found"}"#, "application/json"),
        )
        .expect(1)
        .mount(&server)
        .await;

    let mut mcp = McpProc::spawn_with_base_url(&server.uri());
    let init = mcp.initialize("2025-03-26");
    assert!(
        init["result"]["capabilities"]["tools"].is_object(),
        "{init}"
    );

    mcp.send(&serde_json::json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }));
    let listed = mcp.read_response(2);
    assert!(listed["result"]["tools"].is_array(), "{listed}");
    assert_legacy_shape(&listed["result"], "tools/list");

    // Unknown parameter: rejected while deserializing, before any HTTP request —
    // a tool error that names the field (deny_unknown_fields), not a silent no-op.
    let rejected = mcp.call_tool(
        3,
        "update_card",
        &serde_json::json!({ "card_id": 1, "archived": true }),
    );
    assert!(
        rejected.get("error").is_none(),
        "invalid params must be a tool error, not a JSON-RPC error: {rejected}"
    );
    assert_eq!(
        rejected["result"]["isError"],
        serde_json::json!(true),
        "{rejected}"
    );
    assert!(tool_text(&rejected).contains("archived"), "{rejected}");
    assert_legacy_shape(&rejected["result"], "tools/call (invalid params)");
    assert!(
        server.received_requests().await.unwrap().is_empty(),
        "invalid params must be rejected before any API call"
    );

    // Unknown tool: JSON-RPC "invalid params" (-32602), as with rmcp 2.
    let unknown = mcp.call_tool(4, "no_such_tool", &serde_json::json!({}));
    assert_eq!(
        unknown["error"]["code"],
        serde_json::json!(-32602),
        "{unknown}"
    );
    assert!(unknown.get("result").is_none(), "{unknown}");

    // Success: bearer-authenticated GET, JSON text content, not an error.
    let user = mcp.call_tool(5, "current_user", &serde_json::json!({}));
    assert!(user.get("error").is_none(), "{user}");
    assert_ne!(user["result"]["isError"], serde_json::json!(true), "{user}");
    assert_eq!(
        user["result"]["content"][0]["type"],
        serde_json::json!("text"),
        "{user}"
    );
    let projected: serde_json::Value =
        serde_json::from_str(tool_text(&user)).expect("current_user text must be JSON");
    assert_eq!(projected["id"], serde_json::json!(1_068_514), "{projected}");
    assert_legacy_shape(&user["result"], "tools/call (success)");

    // API failure: a tool error carrying the HTTP status, not a protocol error.
    let missing = mcp.call_tool(6, "get_card", &serde_json::json!({ "card_id": 1 }));
    assert!(missing.get("error").is_none(), "{missing}");
    assert_eq!(
        missing["result"]["isError"],
        serde_json::json!(true),
        "{missing}"
    );
    assert!(tool_text(&missing).contains("404"), "{missing}");
    assert_eq!(server.received_requests().await.unwrap().len(), 2);
}
