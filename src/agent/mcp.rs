//! MCP (Model Context Protocol) server for Dewey.
//!
//! Exposes the Dewey agent protocol as MCP tools over stdio using JSON-RPC 2.0.
//! An AI assistant can connect to this server, discover available tools via
//! `tools/list`, and invoke them via `tools/call` — each tool maps to an
//! [`AgentRequest`] variant processed by a [`HeadlessDriver`].
//!
//! # Wire format
//!
//! - Transport: **stdio** (line-delimited JSON-RPC 2.0)
//! - Client → Server: JSON-RPC request per line
//! - Server → Client: JSON-RPC response per line
//!
//! # Example
//!
//! ```text
//! → {"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}
//! ← {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"dewey","version":"1.0.0"}}}
//! → {"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}
//! ← {"jsonrpc":"2.0","id":2,"result":{"tools":[...]}}
//! → {"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"ping","arguments":{}}}
//! ← {"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"{\"success\":true}"}]}}
//! ```

use std::io::{self, Write};

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::driver::HeadlessDriver;
use super::protocol::{AgentRequest, BatchActionEntry, InjectedEvent};
use crate::runtime::Model;

// ── JSON-RPC 2.0 types ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<serde_json::Value>,
    method: String,
    #[serde(default)]
    params: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

impl JsonRpcResponse {
    fn ok(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }
    }

    fn err(id: serde_json::Value, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
            }),
        }
    }
}

// ── Error codes ──────────────────────────────────────────────────────────

const PARSE_ERROR: i32 = -32700;
const INVALID_REQUEST: i32 = -32600;
const METHOD_NOT_FOUND: i32 = -32601;
const INVALID_PARAMS: i32 = -32602;

/// Maximum allowed size for a single JSON-RPC request line (1 MB). Mirrors the
/// guard in [`super::rpc`]; rejecting oversized lines before parsing prevents a
/// runaway or malicious client from forcing a large `serde_json` allocation.
const MAX_LINE_BYTES: usize = 1_048_576;

// ── MCP tool definitions ─────────────────────────────────────────────────

fn tool_definitions() -> serde_json::Value {
    json!({ "tools": [
        {
            "name": "ping",
            "description": "Ping the Dewey application (keepalive / connectivity check).",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "query_ontology",
            "description": "Query the widget ontology catalog. Returns matching widget types.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Fuzzy search string" },
                    "role": { "type": "string", "description": "Filter by SemanticRole" }
                }
            }
        },
        {
            "name": "get_schema",
            "description": "Get the full schema for a specific widget type.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "widget_type": { "type": "string", "description": "Widget type name" }
                },
                "required": ["widget_type"]
            }
        },
        {
            "name": "get_tree",
            "description": "Get a snapshot of the current UI tree.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "get_state",
            "description": "Get the state of a specific widget by its agent ID.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "agent_id": { "type": "string", "description": "Widget agent ID" }
                },
                "required": ["agent_id"]
            }
        },
        {
            "name": "execute_action",
            "description": "Execute an action on a specific widget.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "agent_id": { "type": "string", "description": "Widget agent ID" },
                    "action": { "type": "string", "description": "Action name" },
                    "params": { "description": "Action parameters (any JSON)" }
                },
                "required": ["agent_id", "action"]
            }
        },
        {
            "name": "inject_event",
            "description": "Inject an input event (key, mouse, text, resize) into the application.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "event": {
                        "type": "object",
                        "description": "Event object with 'kind' field: key, mouse_click, mouse_move, mouse_scroll, text_input, resize"
                    }
                },
                "required": ["event"]
            }
        },
        {
            "name": "batch_actions",
            "description": "Execute multiple widget actions atomically in a single request.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "actions": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "agent_id": { "type": "string" },
                                "action": { "type": "string" },
                                "params": {}
                            },
                            "required": ["agent_id", "action"]
                        }
                    }
                },
                "required": ["actions"]
            }
        },
        {
            "name": "screenshot",
            "description": "Take a screenshot of the current frame.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "format": { "type": "string", "description": "Output format (e.g. 'png')", "default": "png" }
                }
            }
        },
        {
            "name": "negotiate",
            "description": "Negotiate protocol version and capabilities with the Dewey agent protocol.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "client_version": { "type": "integer", "description": "Client protocol version" },
                    "capabilities": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Requested capabilities"
                    }
                },
                "required": ["client_version"]
            }
        },
        {
            "name": "subscribe",
            "description": "Subscribe to application events.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "events": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["events"]
            }
        },
        {
            "name": "unsubscribe",
            "description": "Unsubscribe from application events.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "events": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["events"]
            }
        },
        {
            "name": "quit",
            "description": "Request the Dewey application to quit.",
            "inputSchema": { "type": "object", "properties": {} }
        }
    ]})
}

// ── Tool-name → AgentRequest conversion ──────────────────────────────────

fn parse_tool_call(name: &str, args: &serde_json::Value) -> Result<AgentRequest, String> {
    match name {
        "ping" => Ok(AgentRequest::Ping),
        "quit" => Ok(AgentRequest::Quit),
        "get_tree" => Ok(AgentRequest::GetTree { since: None }),
        "query_ontology" => Ok(AgentRequest::QueryOntology {
            query: args.get("query").and_then(|v| v.as_str()).map(String::from),
            role: args.get("role").and_then(|v| v.as_str()).map(String::from),
        }),
        "get_schema" => {
            let wt = args
                .get("widget_type")
                .and_then(|v| v.as_str())
                .ok_or("missing required field 'widget_type'")?;
            Ok(AgentRequest::GetSchema {
                widget_type: wt.to_string(),
            })
        }
        "get_state" => {
            let id = args
                .get("agent_id")
                .and_then(|v| v.as_str())
                .ok_or("missing required field 'agent_id'")?;
            Ok(AgentRequest::GetState {
                agent_id: id.to_string(),
            })
        }
        "execute_action" => {
            let id = args
                .get("agent_id")
                .and_then(|v| v.as_str())
                .ok_or("missing required field 'agent_id'")?;
            let action = args
                .get("action")
                .and_then(|v| v.as_str())
                .ok_or("missing required field 'action'")?;
            let params = args
                .get("params")
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            Ok(AgentRequest::ExecuteAction {
                agent_id: id.to_string(),
                action: action.to_string(),
                params,
            })
        }
        "inject_event" => {
            let event_val = args.get("event").ok_or("missing required field 'event'")?;
            let event: InjectedEvent = serde_json::from_value(event_val.clone())
                .map_err(|e| format!("invalid event: {e}"))?;
            Ok(AgentRequest::InjectEvent { event })
        }
        "batch_actions" => {
            let arr = args
                .get("actions")
                .and_then(|v| v.as_array())
                .ok_or("missing required field 'actions'")?;
            let actions: Vec<BatchActionEntry> = serde_json::from_value(json!(arr))
                .map_err(|e| format!("invalid actions array: {e}"))?;
            Ok(AgentRequest::BatchActions { actions })
        }
        "screenshot" => {
            let format = args
                .get("format")
                .and_then(|v| v.as_str())
                .unwrap_or("png")
                .to_string();
            Ok(AgentRequest::Screenshot { format })
        }
        "negotiate" => {
            let cv = args
                .get("client_version")
                .and_then(|v| v.as_u64())
                .ok_or("missing required field 'client_version'")? as u32;
            let caps = args
                .get("capabilities")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            Ok(AgentRequest::Negotiate {
                client_version: cv,
                capabilities: caps,
            })
        }
        "subscribe" => {
            let events = args
                .get("events")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .ok_or("missing required field 'events'")?;
            Ok(AgentRequest::Subscribe { events })
        }
        "unsubscribe" => {
            let events = args
                .get("events")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .ok_or("missing required field 'events'")?;
            Ok(AgentRequest::Unsubscribe { events })
        }
        _ => Err(format!("unknown tool '{name}'")),
    }
}

// ── MCP server ───────────────────────────────────────────────────────────

/// An MCP (Model Context Protocol) server that wraps a [`HeadlessDriver`].
///
/// Reads JSON-RPC 2.0 requests from stdin, dispatches them through the
/// Dewey agent protocol, and writes JSON-RPC responses to stdout.
pub struct McpServer<M: Model + 'static> {
    driver: HeadlessDriver<M>,
}

impl<M: Model + 'static> McpServer<M> {
    /// Create a new MCP server with the given model and viewport size.
    pub fn new(model: M, width: f32, height: f32) -> Self {
        let mut driver = HeadlessDriver::new(model, width, height);
        driver.init();
        Self { driver }
    }

    /// Run the MCP server loop, reading from stdin and writing to stdout.
    pub fn run(mut self) -> io::Result<()> {
        let stdin = io::stdin();
        let stdout = io::stdout();
        let mut stdout = stdout.lock();
        let mut reader = stdin.lock();

        while let Some((raw, oversized)) = super::read_capped_line(&mut reader, MAX_LINE_BYTES)? {
            // Reject oversized requests before attempting to parse them. The
            // reader caps buffering at MAX_LINE_BYTES, so an unbounded line can
            // never exhaust memory before this guard fires.
            if oversized {
                let resp = JsonRpcResponse::err(
                    serde_json::Value::Null,
                    INVALID_REQUEST,
                    format!("Request too large (max {MAX_LINE_BYTES} bytes)"),
                );
                write_response(&mut stdout, &resp)?;
                continue;
            }

            let line = String::from_utf8_lossy(&raw);
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let req: JsonRpcRequest = match serde_json::from_str(trimmed) {
                Ok(r) => r,
                Err(e) => {
                    let resp = JsonRpcResponse::err(
                        serde_json::Value::Null,
                        PARSE_ERROR,
                        format!("Parse error: {e}"),
                    );
                    write_response(&mut stdout, &resp)?;
                    continue;
                }
            };

            if req.jsonrpc != "2.0" {
                let resp = JsonRpcResponse::err(
                    req.id.unwrap_or(serde_json::Value::Null),
                    INVALID_REQUEST,
                    "Expected jsonrpc version \"2.0\"",
                );
                write_response(&mut stdout, &resp)?;
                continue;
            }

            let id = req.id.unwrap_or(serde_json::Value::Null);
            let resp = self.handle_method(&req.method, &req.params, id.clone());
            write_response(&mut stdout, &resp)?;

            if !self.driver.is_running() {
                break;
            }
        }

        Ok(())
    }

    fn handle_method(
        &mut self,
        method: &str,
        params: &serde_json::Value,
        id: serde_json::Value,
    ) -> JsonRpcResponse {
        match method {
            "initialize" => JsonRpcResponse::ok(
                id,
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": {
                        "name": "dewey",
                        "version": env!("CARGO_PKG_VERSION"),
                    }
                }),
            ),

            "notifications/initialized" | "initialized" => {
                // Client acknowledgment — no response needed for notifications,
                // but if it has an id we respond with empty result.
                JsonRpcResponse::ok(id, json!({}))
            }

            "tools/list" => JsonRpcResponse::ok(id, tool_definitions()),

            "tools/call" => {
                let name = match params.get("name").and_then(|v| v.as_str()) {
                    Some(n) => n,
                    None => {
                        return JsonRpcResponse::err(
                            id,
                            INVALID_PARAMS,
                            "missing 'name' in tools/call params",
                        );
                    }
                };
                let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

                match parse_tool_call(name, &arguments) {
                    Ok(agent_req) => {
                        let agent_resp = self.driver.process_request(&agent_req);
                        let text = serde_json::to_string(&agent_resp).unwrap_or_default();
                        JsonRpcResponse::ok(
                            id,
                            json!({
                                "content": [{ "type": "text", "text": text }]
                            }),
                        )
                    }
                    Err(e) => JsonRpcResponse::err(id, INVALID_PARAMS, e),
                }
            }

            _ => JsonRpcResponse::err(id, METHOD_NOT_FOUND, format!("Method not found: {method}")),
        }
    }
}

fn write_response(stdout: &mut impl Write, resp: &JsonRpcResponse) -> io::Result<()> {
    let json = serde_json::to_string(resp).unwrap_or_default();
    writeln!(stdout, "{json}")?;
    stdout.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_definitions_has_all_tools() {
        let defs = tool_definitions();
        let tools = defs["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 13);
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"ping"));
        assert!(names.contains(&"query_ontology"));
        assert!(names.contains(&"get_tree"));
        assert!(names.contains(&"execute_action"));
        assert!(names.contains(&"batch_actions"));
        assert!(names.contains(&"quit"));
    }

    #[test]
    fn parse_tool_call_ping() {
        let req = parse_tool_call("ping", &json!({})).unwrap();
        assert!(matches!(req, AgentRequest::Ping));
    }

    #[test]
    fn parse_tool_call_get_schema() {
        let req = parse_tool_call("get_schema", &json!({"widget_type": "Button"})).unwrap();
        match req {
            AgentRequest::GetSchema { widget_type } => assert_eq!(widget_type, "Button"),
            _ => panic!("expected GetSchema"),
        }
    }

    #[test]
    fn parse_tool_call_missing_required() {
        let err = parse_tool_call("get_schema", &json!({})).unwrap_err();
        assert!(err.contains("widget_type"));
    }

    #[test]
    fn parse_tool_call_execute_action() {
        let args = json!({"agent_id": "btn1", "action": "click", "params": {"x": 1}});
        let req = parse_tool_call("execute_action", &args).unwrap();
        match req {
            AgentRequest::ExecuteAction {
                agent_id,
                action,
                params,
            } => {
                assert_eq!(agent_id, "btn1");
                assert_eq!(action, "click");
                assert_eq!(params, json!({"x": 1}));
            }
            _ => panic!("expected ExecuteAction"),
        }
    }

    #[test]
    fn parse_tool_call_unknown() {
        let err = parse_tool_call("nonexistent", &json!({})).unwrap_err();
        assert!(err.contains("unknown tool"));
    }

    #[test]
    fn jsonrpc_response_serialization() {
        let resp = JsonRpcResponse::ok(json!(1), json!({"ok": true}));
        let s = serde_json::to_string(&resp).unwrap();
        assert!(s.contains("\"jsonrpc\":\"2.0\""));
        assert!(s.contains("\"id\":1"));
        assert!(!s.contains("error"));
    }

    #[test]
    fn jsonrpc_error_serialization() {
        let resp = JsonRpcResponse::err(json!(2), -32600, "bad request");
        let s = serde_json::to_string(&resp).unwrap();
        assert!(s.contains("\"code\":-32600"));
        assert!(s.contains("bad request"));
        assert!(!s.contains("result"));
    }
}
