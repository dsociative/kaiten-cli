//! End-to-end smoke test: spawns `kaiten mcp serve` as a real process and
//! speaks JSON-RPC over stdio. Deliberately no tokio here.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const READ_TIMEOUT: Duration = Duration::from_secs(20);

const EXPECTED_TOOLS: [&str; 30] = [
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
