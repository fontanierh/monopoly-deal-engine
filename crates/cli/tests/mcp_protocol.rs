use monopoly_deal_cli::mcp::{Seat, Server, ToolError};
use serde_json::{json, Value};

#[derive(Default)]
struct FakeSeat {
    taken: Vec<Value>,
}

impl Seat for FakeSeat {
    fn observation(&mut self) -> Result<Value, ToolError> {
        Ok(json!({"hand": ["mine"]}))
    }
    fn legal_actions(&mut self) -> Result<Value, ToolError> {
        Ok(json!([{"type": "end_turn"}]))
    }
    fn take_action(&mut self, action: Value) -> Result<Value, ToolError> {
        if action == json!({"type": "end_turn"}) {
            self.taken.push(action);
            Ok(json!({"events": []}))
        } else {
            Err(ToolError::new("illegal_action", "not legal"))
        }
    }
    fn status(&mut self) -> Result<Value, ToolError> {
        Ok(json!({"to_act": [0]}))
    }
}

fn run(lines: &[Value]) -> Vec<Value> {
    let input: String = lines.iter().map(|l| format!("{l}\n")).collect();
    let mut out = Vec::new();
    Server::new(FakeSeat::default())
        .serve(input.as_bytes(), &mut out)
        .unwrap();
    String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn init() -> Value {
    json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "test", "version": "0"}}})
}

fn call(id: i64, name: &str, arguments: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": name, "arguments": arguments}})
}

#[test]
fn lifecycle_and_tool_listing() {
    let out = run(&[
        init(),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
    ]);
    assert_eq!(out.len(), 2, "notifications get no response");
    assert_eq!(out[0]["result"]["protocolVersion"], "2025-06-18");
    assert!(out[0]["result"]["capabilities"]["tools"].is_object());
    let names: Vec<&str> = out[1]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "get_game_status",
            "get_observation",
            "get_legal_actions",
            "take_action"
        ]
    );
    for tool in out[1]["result"]["tools"].as_array().unwrap() {
        assert_eq!(tool["inputSchema"]["type"], "object");
        assert!(
            tool["inputSchema"]["properties"].get("seat").is_none(),
            "seat is fixed by the server"
        );
    }
}

#[test]
fn unsupported_protocol_version_gets_latest() {
    let mut req = init();
    req["params"]["protocolVersion"] = json!("1999-01-01");
    assert_eq!(run(&[req])[0]["result"]["protocolVersion"], "2025-06-18");
}

#[test]
fn tools_require_initialize() {
    let out = run(&[json!({"jsonrpc": "2.0", "id": 5, "method": "tools/list"})]);
    assert_eq!(out[0]["id"], 5);
    assert_eq!(out[0]["error"]["code"], -32600);
}

#[test]
fn tool_calls_return_structured_content() {
    let out = run(&[
        init(),
        call(2, "get_observation", json!({})),
        call(3, "get_legal_actions", json!({})),
        call(4, "take_action", json!({"action": {"type": "end_turn"}})),
    ]);
    assert_eq!(
        out[1]["result"]["structuredContent"],
        json!({"hand": ["mine"]})
    );
    assert_eq!(out[1]["result"]["isError"], false);
    let text: Value =
        serde_json::from_str(out[1]["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(text, out[1]["result"]["structuredContent"]);
    // Arrays are wrapped because structuredContent must be an object.
    assert_eq!(
        out[2]["result"]["structuredContent"],
        json!({"result": [{"type": "end_turn"}]})
    );
    assert_eq!(out[3]["result"]["isError"], false);
}

#[test]
fn rule_errors_are_tool_errors_not_protocol_errors() {
    let out = run(&[
        init(),
        call(2, "take_action", json!({"action": {"type": "cheat"}})),
        call(3, "take_action", json!({})),
    ]);
    assert_eq!(out[1]["result"]["isError"], true);
    assert_eq!(
        out[1]["result"]["structuredContent"]["error"]["code"],
        "illegal_action"
    );
    assert_eq!(out[2]["result"]["isError"], true);
    assert_eq!(
        out[2]["result"]["structuredContent"]["error"]["code"],
        "invalid_arguments"
    );
}

#[test]
fn protocol_errors() {
    let out = run(&[
        init(),
        call(2, "peek_at_opponent", json!({})),
        json!({"jsonrpc": "2.0", "id": 3, "method": "resources/list"}),
        json!([{"jsonrpc": "2.0", "id": 4, "method": "ping"}]),
        json!({"jsonrpc": "2.0", "id": 6, "method": "ping"}),
    ]);
    assert_eq!(out[1]["error"]["code"], -32602);
    assert_eq!(out[2]["error"]["code"], -32601);
    assert_eq!(out[3]["error"]["code"], -32600);
    assert_eq!(out[4]["result"], json!({}));
}

#[test]
fn malformed_json_is_parse_error() {
    let mut out = Vec::new();
    Server::new(FakeSeat::default())
        .serve("{not json\n".as_bytes(), &mut out)
        .unwrap();
    let resp: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(resp["error"]["code"], -32700);
    assert_eq!(resp["id"], Value::Null);
}

#[test]
fn tool_annotations() {
    let out = run(&[
        init(),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
    ]);
    for tool in out[1]["result"]["tools"].as_array().unwrap() {
        let expected = if tool["name"] == "take_action" {
            json!({"readOnlyHint": false, "destructiveHint": false, "idempotentHint": false, "openWorldHint": false})
        } else {
            json!({"readOnlyHint": true, "openWorldHint": false})
        };
        assert_eq!(tool["annotations"], expected, "{}", tool["name"]);
    }
}

fn init_version(version: &str) -> Value {
    let mut req = init();
    req["params"]["protocolVersion"] = json!(version);
    req
}

#[test]
fn batches_are_accepted_for_2025_03_26() {
    let out = run(&[
        init_version("2025-03-26"),
        json!([
            {"jsonrpc": "2.0", "id": 2, "method": "ping"},
            {"jsonrpc": "2.0", "method": "notifications/initialized"},
            call(3, "get_observation", json!({})),
            {"jsonrpc": "2.0", "id": 4, "method": "initialize", "params": {}},
        ]),
        json!([{"jsonrpc": "2.0", "method": "notifications/initialized"}]),
        json!([]),
    ]);
    assert_eq!(out[0]["result"]["protocolVersion"], "2025-03-26");
    let batch = out[1].as_array().unwrap();
    assert_eq!(batch.len(), 3, "notifications get no response");
    assert_eq!(batch[0]["result"], json!({}));
    assert_eq!(
        batch[1]["result"]["structuredContent"],
        json!({"hand": ["mine"]})
    );
    assert_eq!(batch[2]["error"]["code"], -32600);
    // A batch of only notifications gets no response, so the empty batch error is next.
    assert_eq!(out.len(), 3);
    assert_eq!(out[2]["error"]["code"], -32600);
}

#[test]
fn batches_are_rejected_for_2025_06_18_and_before_initialize() {
    let ping = json!([{"jsonrpc": "2.0", "id": 2, "method": "ping"}]);
    let out = run(&[ping.clone(), init(), ping]);
    assert_eq!(out[0]["error"]["code"], -32600);
    assert_eq!(out[2]["error"]["code"], -32600);
}
