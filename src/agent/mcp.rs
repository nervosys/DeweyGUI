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

/// What the client tells the model before it decides anything.
///
/// MCP surfaces this from `initialize`, and it is the highest-leverage text in
/// this project: a model that has not been told the application describes
/// itself will read the source instead, which is slower, larger, and answers a
/// question about the code rather than about the running program. The ontology
/// costs nothing if nobody asks it.
const INSTRUCTIONS: &str = "\
This is a running GUI application that describes itself. You do not need to \
read its source code to drive it, and reading the source answers a different \
question — what the program could do, rather than what is on screen now.

Start with `get_tree`: it returns every widget currently displayed, with its \
id, its state, its bounds, and the actions it accepts. Act with \
`execute_action` using an id from that tree. `query_ontology` describes the \
kinds of widget available and what each kind can do, which is what you want \
before writing an interface rather than driving one.

Three things worth knowing. `get_tree` takes `since`, the `version` from your \
last reply, and answers `unchanged` without rendering anything — polling that \
way costs about a hundredth of a full read. It also takes a `viewport`, and a \
long list is a large reply without one. And `validate` reports faults you \
cannot see in a screenshot: widgets that rendered with no id and so cannot be \
clicked at all, duplicate ids, zero-size bounds, and text painted at a \
contrast nobody can read.\
";

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
            "description": "What kinds of widget this application is built \
                from, and what each one can be asked to do. Ask this \
                rather than reading the source: the answer describes what \
                the running program actually does, costs a few hundred \
                tokens against a whole codebase, and cannot be out of \
                date.",
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
            "description": "What is on screen right now: every widget, its \
                id, its state and where it is. This is how you find the id \
                to act on — you need neither the source nor a screenshot. \
                Pass the \
    version from a previous reply as 'since' to be told 'unchanged' instead of \
    being sent an identical tree.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "since": {
                        "type": "integer",
                        "description": "Version last seen; omit for a full tree"
                    },
                    "viewport": {
                        "type": "object",
                        "description": "Describe only widgets intersecting this rectangle. Without it the tree covers every widget including those scrolled out of sight.",
                        "properties": {
                            "x": { "type": "number" },
                            "y": { "type": "number" },
                            "width": { "type": "number" },
                            "height": { "type": "number" }
                        },
                        "required": ["x", "y", "width", "height"]
                    }
                }
            }
        },
        {
            "name": "validate",
            "description": "Check the rendered interface for structural faults: \
    widgets that cannot be clicked or addressed, duplicated ids, empty or offscreen \
    bounds, and handlers bound to actions a widget does not advertise. Answers \
    whether what was built can be operated, which a screenshot cannot.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "strict": {
                        "type": "boolean",
                        "description": "Promote warnings to errors and also \
    report widgets that publish actions with nothing wired to any of them. For an \
    application meant to be driven unattended.",
                        "default": false
                    }
                }
            }
        },
        {
            "name": "get_state",
            "description": "The state of one widget, by id. Cheaper than \
                `get_tree` when you already know which widget you care \
                about.",
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
            "description": "Do something to a widget: press it, set its \
                value, select an item. Action names come from \
                `query_ontology` or `get_tree`; an action a widget does \
                not advertise is refused rather than silently ignored.",
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
            "description": "Run several actions in one request, in order. \
                Not atomic: nothing is rolled back. Stops at the first \
                failure and reports `applied` and `failed_at`, so you can \
                tell how far it got.",
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
                    "format": {
                        "type": "string",
                        "description": "Output format. 'text' returns a stable \
    tree rendering suitable for golden comparison between runs.",
                        "default": "json"
                    }
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
        "get_tree" => Ok(AgentRequest::GetTree {
            since: args.get("since").and_then(serde_json::Value::as_u64),
            viewport: args
                .get("viewport")
                .and_then(|v| serde_json::from_value(v.clone()).ok()),
        }),
        "validate" => Ok(AgentRequest::Validate {
            strict: args
                .get("strict")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
        }),
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
                .unwrap_or("json")
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
                    },
                    "instructions": INSTRUCTIONS,
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
        assert_eq!(tools.len(), 14);
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"ping"));
        assert!(names.contains(&"query_ontology"));
        assert!(names.contains(&"get_tree"));
        assert!(names.contains(&"execute_action"));
        assert!(names.contains(&"batch_actions"));
        assert!(names.contains(&"quit"));
        assert!(names.contains(&"validate"));
    }

    /// The tool descriptions are what a model reads before deciding anything.
    ///
    /// The performance case for an ontology assumes the agent asks it. A model
    /// that has not been told the application describes itself reads the
    /// source instead — slower, far larger, and an answer about what the code
    /// could do rather than about what is on screen. So the descriptions and
    /// the `initialize` instructions have to say it, and this asserts they do.
    #[test]
    fn the_tools_tell_a_model_not_to_read_the_source() {
        assert!(
            INSTRUCTIONS.contains("You do not need to")
                && INSTRUCTIONS.contains("read its source code"),
            "the MCP instructions no longer steer a model away from the source"
        );
        assert!(
            INSTRUCTIONS.contains("get_tree"),
            "the instructions must name the cheapest first call"
        );

        let defs = tool_definitions();
        let described = |name: &str| -> String {
            defs["tools"]
                .as_array()
                .unwrap()
                .iter()
                .find(|t| t["name"] == name)
                .unwrap_or_else(|| panic!("{name} is not a tool"))["description"]
                .as_str()
                .unwrap()
                .to_string()
        };
        for name in ["query_ontology", "get_tree"] {
            let text = described(name);
            assert!(
                text.contains("source") || text.contains("screenshot"),
                "`{name}` does not say why to use it instead of reading the \
                 source, and the description is the only thing a model reads \
                 before choosing: {text}"
            );
        }
    }

    /// No tool may promise atomicity, because nothing rolls back.
    ///
    /// The protocol reference stopped calling batches atomic when it turned
    /// out that a failing entry did not even stop the ones after it. The MCP
    /// description went on saying "atomically" — the same claim, in the one
    /// place a coding agent actually reads.
    #[test]
    fn no_tool_claims_to_be_atomic() {
        let defs = tool_definitions();
        for tool in defs["tools"].as_array().unwrap() {
            let text = tool["description"].as_str().unwrap_or_default();
            let lower = text.to_lowercase();
            // Saying it is *not* atomic is the point, so only the promise
            // counts against a description.
            let promises = lower.contains("atomic") && !lower.contains("not atomic");
            assert!(
                !promises,
                "`{}` promises atomicity and nothing in this crate rolls \
                 anything back: {text}",
                tool["name"]
            );
        }
    }

    /// Every request the protocol understands and an agent would want should
    /// be reachable from MCP, which is the interface a coding agent connects
    /// through. `validate` and the conditional `get_tree` were added to the
    /// protocol and not to this list, so neither could be called here.
    #[test]
    fn mcp_exposes_validation_and_conditional_tree_reads() {
        assert!(matches!(
            parse_tool_call("validate", &json!({})).unwrap(),
            AgentRequest::Validate { strict: false }
        ));
        assert!(matches!(
            parse_tool_call("validate", &json!({"strict": true})).unwrap(),
            AgentRequest::Validate { strict: true }
        ));
        assert!(matches!(
            parse_tool_call("get_tree", &json!({})).unwrap(),
            AgentRequest::GetTree {
                since: None,
                viewport: None
            }
        ));
        assert!(matches!(
            parse_tool_call("get_tree", &json!({"since": 7})).unwrap(),
            AgentRequest::GetTree {
                since: Some(7),
                viewport: None
            }
        ));

        let defs = tool_definitions();
        let tools = defs["tools"].as_array().unwrap();
        let get_tree = tools.iter().find(|t| t["name"] == "get_tree").unwrap();
        assert!(
            get_tree["inputSchema"]["properties"]["since"].is_object(),
            "an agent reading the schema must be told `since` exists"
        );
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
