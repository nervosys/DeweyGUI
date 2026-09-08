//! The protocol loop every agent's traffic passes through.
//!
//! `serve_stdio` held `io::stdin()` and `io::stdout()` directly, so nothing
//! could drive it. The rate limit, the oversize guard, the reply to malformed
//! JSON, the events a subscriber is sent and the shutdown on `quit` were all
//! unverifiable — on the one surface an agent cannot avoid. `serve` is the
//! same loop over any reader and writer, and these drive it.
//!
//! The sink here is a real `HeadlessDriver`, so a reply is what an application
//! would actually send rather than a fixture's idea of one.

use std::io::Cursor;

use dewey::agent::driver::HeadlessDriver;
use dewey::agent::protocol::{AgentResponse, RequestEnvelope};
use dewey::agent::rpc::{RequestSink, serve};
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

/// A sink backed by a real driver, which is what the stdio transport wraps.
struct DriverSink {
    driver: HeadlessDriver<Counter>,
}

impl RequestSink for DriverSink {
    fn answer(&mut self, envelope: &RequestEnvelope) -> String {
        self.driver.process_envelope_json(envelope)
    }

    fn is_running(&self) -> bool {
        self.driver.is_running()
    }

    fn drain_events(&mut self) -> Vec<String> {
        self.driver.drain_events_json()
    }
}

fn sink() -> DriverSink {
    let mut driver = HeadlessDriver::new(Counter { count: 0 }, 200.0, 200.0);
    driver.init();
    DriverSink { driver }
}

/// Run the loop over `input` and give back the lines it wrote.
fn exchange(input: &str) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    let mut s = sink();
    serve(&mut s, Cursor::new(input.as_bytes().to_vec()), &mut out).expect("the loop");
    String::from_utf8(out)
        .expect("utf-8")
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("reply {l:?}: {e}")))
        .collect()
}

#[test]
fn a_request_is_answered_with_its_own_id() {
    let replies = exchange("{\"id\":\"a\",\"request\":{\"type\":\"ping\"}}\n");
    assert_eq!(replies.len(), 1, "{replies:?}");
    assert_eq!(replies[0]["success"], true);
    assert_eq!(replies[0]["id"], "a");
    assert_eq!(replies[0]["data"]["status"], "pong");
}

/// One reply per request, in order. An agent matches them up by `id`, and
/// would be reading somebody else's answer if the loop ever dropped one.
#[test]
fn every_request_gets_exactly_one_reply_in_order() {
    let replies = exchange(concat!(
        "{\"id\":\"1\",\"request\":{\"type\":\"ping\"}}\n",
        "{\"id\":\"2\",\"request\":{\"type\":\"get_state\",\"agent_id\":\"count\"}}\n",
        "{\"id\":\"3\",\"request\":{\"type\":\"ping\"}}\n",
    ));
    let ids: Vec<&str> = replies.iter().filter_map(|r| r["id"].as_str()).collect();
    assert_eq!(ids, ["1", "2", "3"], "{replies:?}");
}

/// Malformed JSON is answered and the connection stays open. A transport that
/// died on a bad line would take the whole session with it.
#[test]
fn malformed_json_is_reported_and_the_loop_continues() {
    let replies = exchange(concat!(
        "not json at all\n",
        "{\"id\":\"after\",\"request\":{\"type\":\"ping\"}}\n",
    ));
    assert_eq!(replies.len(), 2, "{replies:?}");
    assert_eq!(replies[0]["success"], false);
    assert!(
        replies[0]["error"]
            .as_str()
            .is_some_and(|e| e.contains("Invalid JSON")),
        "{replies:?}"
    );
    assert_eq!(replies[1]["id"], "after");
}

/// Blank lines are not requests and must not be answered.
#[test]
fn blank_lines_are_skipped() {
    let replies = exchange("\n\n{\"id\":\"x\",\"request\":{\"type\":\"ping\"}}\n\n");
    assert_eq!(replies.len(), 1, "{replies:?}");
    assert_eq!(replies[0]["id"], "x");
}

/// A line longer than the cap is refused by size rather than parsed. The
/// reader stops buffering at the cap, so this is what stands between an agent
/// and the process's memory.
#[test]
fn an_oversized_line_is_refused_and_the_loop_continues() {
    let huge = format!(
        "{{\"id\":\"big\",\"request\":{{\"type\":\"ping\"}},\"pad\":\"{}\"}}\n",
        "x".repeat(1_100_000)
    );
    let replies = exchange(&format!(
        "{huge}{}",
        "{\"id\":\"after\",\"request\":{\"type\":\"ping\"}}\n"
    ));
    assert_eq!(replies[0]["success"], false, "{replies:?}");
    assert!(
        replies[0]["error"]
            .as_str()
            .is_some_and(|e| e.contains("too large")),
        "{replies:?}"
    );
    assert_eq!(
        replies.last().expect("a reply")["id"],
        "after",
        "the loop stopped after an oversized line"
    );
}

/// `quit` ends the loop, and anything after it is never read.
#[test]
fn quit_closes_the_connection() {
    let replies = exchange(concat!(
        "{\"id\":\"1\",\"request\":{\"type\":\"quit\"}}\n",
        "{\"id\":\"2\",\"request\":{\"type\":\"ping\"}}\n",
    ));
    assert_eq!(
        replies.len(),
        1,
        "the loop kept reading after quit: {replies:?}"
    );
    assert_eq!(replies[0]["id"], "1");
}

/// A subscriber is sent what changed, on the same stream, after the reply that
/// caused it. The protocol accepted `subscribe` for a long time and nothing
/// ever sent anything back.
#[test]
fn a_subscriber_is_sent_events_after_the_reply() {
    let replies = exchange(concat!(
        "{\"id\":\"1\",\"request\":{\"type\":\"subscribe\",\"events\":[\"state_changed\"]}}\n",
        "{\"id\":\"2\",\"request\":{\"type\":\"execute_action\",\"agent_id\":\"inc\",\"action\":\"click\"}}\n",
    ));
    let events: Vec<&serde_json::Value> = replies
        .iter()
        .filter(|r| r.get("success").is_none())
        .collect();
    assert!(
        !events.is_empty(),
        "a subscribed agent was sent no events at all: {replies:?}"
    );
    assert!(
        events
            .iter()
            .any(|e| e["type"] == "state_changed" && e["agent_id"] == "count"),
        "the label changed and no `state_changed` named it: {events:?}"
    );
}

/// The reply to a request the server refuses is still a reply. An agent
/// waiting on an id must not be left waiting because the answer was `false`.
#[test]
fn a_refused_request_still_answers() {
    let replies = exchange(
        "{\"id\":\"nope\",\"request\":{\"type\":\"get_state\",\"agent_id\":\"missing\"}}\n",
    );
    assert_eq!(replies.len(), 1, "{replies:?}");
    assert_eq!(replies[0]["id"], "nope");
    assert_eq!(replies[0]["success"], false);
}

/// `AgentResponse` is what the loop writes, so a reply always parses back as
/// one. Anything else and an agent's client fails on a line it cannot decode.
#[test]
fn every_line_written_is_a_protocol_message() {
    let replies = exchange(concat!(
        "garbage\n",
        "{\"id\":\"1\",\"request\":{\"type\":\"ping\"}}\n",
    ));
    for reply in &replies {
        if reply.get("success").is_some() {
            serde_json::from_value::<AgentResponse>(reply.clone())
                .unwrap_or_else(|e| panic!("not an AgentResponse: {reply} ({e})"));
        }
    }
}

/// The rate limit, which is the one guard here with a magic number in it.
///
/// A thousand requests a second is the cap. The thousand-and-first inside the
/// same window is refused, and refused with an answer rather than by closing
/// the connection — an agent that is going too fast should be told so.
#[test]
fn past_the_rate_limit_a_request_is_refused_not_dropped() {
    let line = "{\"id\":\"n\",\"request\":{\"type\":\"ping\"}}\n";
    let input: String = line.repeat(1_010);
    let replies = exchange(&input);
    assert_eq!(
        replies.len(),
        1_010,
        "every line must be answered, even the ones over the limit"
    );
    let refused = replies
        .iter()
        .filter(|r| {
            r["error"]
                .as_str()
                .is_some_and(|e| e.contains("Rate limit"))
        })
        .count();
    assert!(
        refused >= 5,
        "1010 requests in one window and only {refused} were rate limited"
    );
}
