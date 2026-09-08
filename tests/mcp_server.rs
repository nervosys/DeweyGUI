//! The MCP loop a coding agent connects to.
//!
//! `McpServer::run` held `io::stdin()` and `io::stdout()`, so nothing could
//! drive it. The tool list and the argument parsing had unit tests; the loop
//! that frames them had none — not the JSON-RPC envelope, not the refusal of a
//! wrong version, not the code an unknown method comes back with, and not
//! `initialize`.
//!
//! `initialize` is where `INSTRUCTIONS` reaches the model. That text is the
//! highest-leverage string in this project — it is the whole of the argument
//! that an agent should ask the application rather than read its source — and
//! nothing checked it was ever sent.

use std::io::Cursor;

use dewey::agent::mcp::McpServer;
use dewey::prelude::*;

struct Counter {
    count: i32,
}

impl Model for Counter {
    type Msg = ();

    fn update(&mut self, _m: ()) -> Command<()> {
        Command::None
    }

    fn view(&self, frame: &mut Frame<'_>) {
        let rows = frame.area.rows_of(&[40.0, 40.0]);
        Label::new(format!("Count: {}", self.count))
            .agent_id("count")
            .render(rows[0], frame);
        Button::new("+")
            .on("inc", |c: &mut Counter| c.count += 1)
            .render(rows[1], frame);
    }
}

/// Feed the loop some lines and collect the JSON-RPC messages it wrote.
fn exchange(input: &str) -> Vec<serde_json::Value> {
    let server = McpServer::new(Counter { count: 0 }, 200.0, 200.0);
    let mut out = Vec::new();
    server
        .serve(Cursor::new(input.as_bytes().to_vec()), &mut out)
        .expect("the loop");
    String::from_utf8(out)
        .expect("utf-8")
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("reply {l:?}: {e}")))
        .collect()
}

fn call(method: &str, params: &str, id: u32) -> String {
    format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"method\":\"{method}\",\"params\":{params}}}\n")
}

/// The instructions reach the client, and say the thing they exist to say.
#[test]
fn initialize_returns_the_instructions_that_steer_the_model() {
    let replies = exchange(&call("initialize", "{}", 1));
    assert_eq!(replies.len(), 1, "{replies:?}");
    let result = &replies[0]["result"];
    assert_eq!(result["protocolVersion"], "2024-11-05");
    assert_eq!(result["serverInfo"]["name"], "dewey");

    let instructions = result["instructions"]
        .as_str()
        .unwrap_or_else(|| panic!("`initialize` sent no instructions: {result}"));
    assert!(
        instructions.contains("read its source code"),
        "the instructions no longer steer a model away from the source: \
         {instructions}"
    );
    assert!(
        instructions.contains("get_tree"),
        "the instructions must name the cheapest first call: {instructions}"
    );
}

/// Every tool the server advertises comes back from `tools/list`, with a
/// description — the only text a model reads before choosing one.
#[test]
fn tools_list_returns_described_tools() {
    let replies = exchange(&call("tools/list", "{}", 2));
    let tools = replies[0]["result"]["tools"]
        .as_array()
        .unwrap_or_else(|| panic!("{:?}", replies[0]));
    assert!(tools.len() >= 15, "{} tools", tools.len());
    for tool in tools {
        let name = tool["name"].as_str().expect("a name");
        let described = tool["description"].as_str().unwrap_or_default();
        assert!(
            described.len() > 20,
            "`{name}` is offered with no useful description"
        );
        assert!(
            tool["inputSchema"].is_object(),
            "`{name}` is offered without an input schema"
        );
    }
}

/// A tool call reaches the application and changes it.
#[test]
fn a_tool_call_reaches_the_model() {
    let replies = exchange(&format!(
        "{}{}",
        call(
            "tools/call",
            "{\"name\":\"execute_action\",\"arguments\":{\"agent_id\":\"inc\",\"action\":\"click\"}}",
            1
        ),
        call(
            "tools/call",
            "{\"name\":\"get_state\",\"arguments\":{\"agent_id\":\"count\"}}",
            2
        ),
    ));
    assert_eq!(replies.len(), 2, "{replies:?}");
    let text = serde_json::to_string(&replies[1]).expect("json");
    assert!(
        text.contains("Count: 1"),
        "the click did not reach the model: {text}"
    );
}

/// An unknown method is refused with the JSON-RPC code for it, not with a
/// success carrying an error inside.
#[test]
fn an_unknown_method_is_refused_with_its_own_code() {
    let replies = exchange(&call("tools/frobnicate", "{}", 7));
    assert_eq!(replies[0]["id"], 7);
    assert_eq!(
        replies[0]["error"]["code"], -32601,
        "method-not-found is -32601: {:?}",
        replies[0]
    );
    assert!(replies[0].get("result").is_none());
}

/// Malformed JSON is a parse error, and the connection stays open.
#[test]
fn malformed_json_is_a_parse_error_and_the_loop_continues() {
    let replies = exchange(&format!(
        "this is not json\n{}",
        call("initialize", "{}", 1)
    ));
    assert_eq!(replies.len(), 2, "{replies:?}");
    assert_eq!(replies[0]["error"]["code"], -32700, "parse error is -32700");
    assert_eq!(replies[1]["id"], 1, "the loop stopped after a bad line");
}

/// A request naming the wrong protocol version is refused rather than guessed
/// at. JSON-RPC 2.0 is the only version this speaks.
#[test]
fn a_wrong_jsonrpc_version_is_refused() {
    let replies =
        exchange("{\"jsonrpc\":\"1.0\",\"id\":3,\"method\":\"initialize\",\"params\":{}}\n");
    assert_eq!(replies[0]["error"]["code"], -32600, "{:?}", replies[0]);
    assert_eq!(replies[0]["id"], 3, "the refusal must carry the id back");
}

/// Blank lines are not requests.
#[test]
fn blank_lines_are_skipped() {
    let replies = exchange(&format!("\n\n{}\n", call("initialize", "{}", 1)));
    assert_eq!(replies.len(), 1, "{replies:?}");
}

/// Every line written is a JSON-RPC message carrying the id it answers, so a
/// client waiting on one is never left waiting.
#[test]
fn every_reply_is_addressed() {
    let replies = exchange(&format!(
        "{}{}{}",
        call("initialize", "{}", 1),
        call("tools/list", "{}", 2),
        call("tools/nowhere", "{}", 3),
    ));
    let ids: Vec<u64> = replies.iter().filter_map(|r| r["id"].as_u64()).collect();
    assert_eq!(ids, [1, 2, 3], "{replies:?}");
    for reply in &replies {
        assert_eq!(reply["jsonrpc"], "2.0");
        assert!(
            reply.get("result").is_some() || reply.get("error").is_some(),
            "a reply that is neither a result nor an error: {reply}"
        );
    }
}
