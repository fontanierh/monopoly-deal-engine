//! Minimal MCP (Model Context Protocol) server over stdio.
//!
//! Transport: newline-delimited JSON-RPC 2.0 messages on stdin/stdout, as in the MCP
//! stdio transport. Logs go to stderr only. The server is bound to one seat, so no tool
//! takes a seat argument and a client can only ever see its own seat's private view.

use std::io::{self, BufRead, Write};

use serde_json::{json, Value};

pub const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

/// A game rule or input error reported back to the model as a tool result with `isError: true`.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolError {
    pub code: String,
    pub message: String,
}

impl ToolError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

/// The game operations one seat may perform. All validation happens behind this trait in core.
pub trait Seat {
    fn observation(&mut self) -> Result<Value, ToolError>;
    fn legal_actions(&mut self) -> Result<Value, ToolError>;
    fn take_action(&mut self, action: Value) -> Result<Value, ToolError>;
    fn status(&mut self) -> Result<Value, ToolError>;
}

pub struct Server<S> {
    seat: S,
    /// The protocol version agreed in `initialize`.
    protocol: Option<&'static str>,
}

impl<S: Seat> Server<S> {
    pub fn new(seat: S) -> Self {
        Self {
            seat,
            protocol: None,
        }
    }

    /// Reads messages until EOF, writing one response line per request.
    pub fn serve(&mut self, input: impl BufRead, mut output: impl Write) -> io::Result<()> {
        for line in input.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Some(response) = self.handle_line(&line) {
                serde_json::to_writer(&mut output, &response)?;
                output.write_all(b"\n")?;
                output.flush()?;
            }
        }
        Ok(())
    }

    pub fn handle_line(&mut self, line: &str) -> Option<Value> {
        match serde_json::from_str::<Value>(line) {
            Ok(Value::Array(batch)) => self.handle_batch(batch),
            Ok(message) => self.handle_message(message),
            Err(e) => Some(error_response(
                Value::Null,
                PARSE_ERROR,
                &format!("parse error: {e}"),
            )),
        }
    }

    /// JSON-RPC batches: protocol 2025-03-26 requires servers to accept them, 2024-11-05
    /// inherits them from JSON-RPC 2.0, and 2025-06-18 removed them.
    fn handle_batch(&mut self, batch: Vec<Value>) -> Option<Value> {
        if !matches!(self.protocol, Some("2025-03-26" | "2024-11-05")) {
            return Some(error_response(
                Value::Null,
                INVALID_REQUEST,
                "JSON-RPC batches are not supported by this protocol version",
            ));
        }
        if batch.is_empty() {
            return Some(error_response(Value::Null, INVALID_REQUEST, "empty batch"));
        }
        let responses: Vec<Value> = batch
            .into_iter()
            .filter_map(|message| {
                if message.get("method") == Some(&json!("initialize")) {
                    let id = message.get("id").cloned().unwrap_or(Value::Null);
                    return Some(error_response(
                        id,
                        INVALID_REQUEST,
                        "initialize must not be part of a batch",
                    ));
                }
                self.handle_message(message)
            })
            .collect();
        (!responses.is_empty()).then_some(Value::Array(responses))
    }

    pub fn handle_message(&mut self, message: Value) -> Option<Value> {
        let Some(obj) = message.as_object() else {
            return Some(error_response(
                Value::Null,
                INVALID_REQUEST,
                "expected a single JSON-RPC object",
            ));
        };
        let id = obj.get("id").cloned();
        let method = obj.get("method").and_then(Value::as_str);
        let (Some(method), true) = (method, obj.get("jsonrpc") == Some(&json!("2.0"))) else {
            // Responses from the client (we never send requests) and malformed messages.
            return id
                .map(|id| error_response(id, INVALID_REQUEST, "invalid JSON-RPC 2.0 request"));
        };
        let params = obj.get("params").cloned().unwrap_or(Value::Null);
        let Some(id) = id else {
            // Notifications never get a response; `notifications/initialized` needs no action.
            return None;
        };
        let result = self.dispatch(method, params);
        Some(match result {
            Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
            Err((code, message)) => error_response(id, code, &message),
        })
    }

    fn dispatch(&mut self, method: &str, params: Value) -> Result<Value, (i64, String)> {
        match method {
            "initialize" => {
                let version = negotiate(&params);
                self.protocol = Some(version);
                Ok(initialize_result(version))
            }
            "ping" => Ok(json!({})),
            _ if self.protocol.is_none() => Err((INVALID_REQUEST, "server not initialized".into())),
            "tools/list" => Ok(json!({"tools": tool_definitions()})),
            "tools/call" => self.call_tool(params),
            _ => Err((METHOD_NOT_FOUND, format!("method not found: {method}"))),
        }
    }

    fn call_tool(&mut self, params: Value) -> Result<Value, (i64, String)> {
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or((INVALID_PARAMS, "missing tool name".to_string()))?;
        let args = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let outcome = match name {
            "get_observation" => self.seat.observation(),
            "get_legal_actions" => self.seat.legal_actions(),
            "get_game_status" => self.seat.status(),
            "take_action" => match args.get("action") {
                Some(action) => self.seat.take_action(action.clone()),
                None => Err(ToolError::new(
                    "invalid_arguments",
                    "take_action requires an `action` argument",
                )),
            },
            _ => return Err((INVALID_PARAMS, format!("unknown tool: {name}"))),
        };
        Ok(tool_result(outcome))
    }
}

/// The client's version if supported, otherwise the latest one we support.
fn negotiate(params: &Value) -> &'static str {
    let requested = params.get("protocolVersion").and_then(Value::as_str);
    SUPPORTED_PROTOCOL_VERSIONS
        .iter()
        .find(|v| Some(**v) == requested)
        .unwrap_or(&SUPPORTED_PROTOCOL_VERSIONS[0])
}

fn initialize_result(version: &str) -> Value {
    json!({
        "protocolVersion": version,
        "capabilities": {"tools": {"listChanged": false}},
        "serverInfo": {"name": "monopoly-deal-engine", "version": env!("CARGO_PKG_VERSION")},
        "instructions": "You control one seat in a game of Monopoly Deal. Call get_game_status to see whose decision it is, get_observation for your private view, get_legal_actions for every currently legal action, and take_action with one of those actions copied verbatim."
    })
}

fn tool_result(outcome: Result<Value, ToolError>) -> Value {
    let (structured, is_error) = match outcome {
        Ok(value) => (wrap_object(value), false),
        Err(e) => (
            json!({"error": {"code": e.code, "message": e.message}}),
            true,
        ),
    };
    json!({
        "content": [{"type": "text", "text": structured.to_string()}],
        "structuredContent": structured,
        "isError": is_error,
    })
}

/// MCP requires `structuredContent` to be a JSON object.
fn wrap_object(value: Value) -> Value {
    if value.is_object() {
        value
    } else {
        json!({"result": value})
    }
}

fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

pub fn tool_definitions() -> Value {
    let no_args = json!({"type": "object", "properties": {}, "additionalProperties": false});
    let read_only = json!({"readOnlyHint": true, "openWorldHint": false});
    json!([
        {
            "name": "get_game_status",
            "title": "Game status",
            "description": "Public game status: phase, whose decision it is, whether you must act now, and the winner if the game is over.",
            "inputSchema": no_args,
            "annotations": read_only,
        },
        {
            "name": "get_observation",
            "title": "Your observation",
            "description": "Your private view of the game: your hand, all public table state, your legal actions, recent public events, and a `cards` map from card id to name, value and kind. Other players' hands are only shown as counts.",
            "inputSchema": no_args,
            "annotations": read_only,
        },
        {
            "name": "get_legal_actions",
            "title": "Legal actions",
            "description": "Every action you may take right now. Empty when it is not your decision.",
            "inputSchema": no_args,
            "annotations": read_only,
        },
        {
            "name": "take_action",
            "title": "Take action",
            "description": "Submit one action, copied exactly from get_legal_actions. The engine validates it and returns the new game status and your updated observation.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": {
                        "type": "object",
                        "description": "One action object from get_legal_actions, for example {\"type\": \"end_turn\"}.",
                        "properties": {"type": {"type": "string"}},
                        "required": ["type"],
                    }
                },
                "required": ["action"],
                "additionalProperties": false,
            },
            "annotations": {
                "readOnlyHint": false,
                "destructiveHint": false,
                "idempotentHint": false,
                "openWorldHint": false,
            },
        },
    ])
}
