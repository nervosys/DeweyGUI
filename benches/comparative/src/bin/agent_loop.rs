//! How fast can an agent close the loop on a running GUI?
//!
//! Scaffolding a GUI is only half of what an agent does; the other half is
//! confirming the thing it just wrote actually works. That loop is
//! discover → understand → act → verify, and Dewey answers all four over the
//! agent protocol with no window, no GPU and no screenshot.
//!
//! There is no equivalent to time in egui or iced: neither exposes a widget
//! tree, a typed action, or a readable state snapshot to an external process.
//! An agent driving either must open a real window and fall back on pixels —
//! see README.md for what that costs and, more importantly, why it cannot
//! assert the way this can.

use std::hint::black_box;
use std::time::{Duration, Instant};

use dewey::agent::driver::HeadlessDriver;
use dewey::agent::protocol::AgentRequest;
use dewey::prelude::*;

// ── The canonical counter, agent-driveable ─────────────────────────

struct App {
    count: i32,
}

/// A realistic application message type: the benchmark drives the app
/// through widget handlers, so nothing here is constructed by name.
#[derive(Debug)]
#[allow(dead_code)]
enum Msg {
    Increment,
    Decrement,
    Reset,
}

impl Model for App {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) -> Command<Msg> {
        match msg {
            Msg::Increment => self.count += 1,
            Msg::Decrement => self.count -= 1,
            Msg::Reset => self.count = 0,
        }
        Command::None
    }

    fn view(&self, frame: &mut Frame<'_>) {
        let chunks = Layout::new(
            Direction::Vertical,
            [Constraint::Length(40.0), Constraint::Length(40.0)],
        )
        .split(frame.area);

        Label::new(format!("Count: {}", self.count))
            .agent_id("count")
            .render(chunks[0], frame);

        let cols = Layout::new(
            Direction::Horizontal,
            [
                Constraint::Percentage(33.3),
                Constraint::Percentage(33.3),
                Constraint::Percentage(33.3),
            ],
        )
        .split(chunks[1]);

        Button::new("- Decrement")
            .agent_id("dec")
            .render(cols[0], frame);
        Button::new("Reset").agent_id("reset").render(cols[1], frame);
        Button::new("+ Increment")
            .agent_id("inc")
            .render(cols[2], frame);
    }

    fn handle_event(&self, _event: Event) -> Option<Msg> {
        None
    }

    /// What lets an agent press a button without a mouse.
    fn execute_action(
        &mut self,
        agent_id: &str,
        action: &str,
        _params: &serde_json::Value,
    ) -> serde_json::Value {
        if action != "click" {
            return serde_json::Value::Null;
        }
        match agent_id {
            "inc" => self.count += 1,
            "dec" => self.count -= 1,
            "reset" => self.count = 0,
            _ => return serde_json::Value::Null,
        }
        serde_json::json!({ "count": self.count })
    }
}

fn driver() -> HeadlessDriver<App> {
    let mut d = HeadlessDriver::new(App { count: 0 }, 400.0, 200.0);
    d.init();
    d
}

fn fmt(d: Duration) -> String {
    let us = d.as_secs_f64() * 1e6;
    if us < 1.0 {
        return format!("{:.0} ns", d.as_secs_f64() * 1e9);
    }
    if us >= 1000.0 {
        format!("{:.2} ms", us / 1000.0)
    } else {
        format!("{us:.1} µs")
    }
}

fn main() {
    const ROUNDS: usize = 2_000;

    let steps: Vec<(&str, AgentRequest)> = vec![
        ("1. discover    (get_tree)", AgentRequest::GetTree { since: None, viewport: None }),
        (
            "2. read schema (get_schema Button)",
            AgentRequest::GetSchema {
                widget_type: "Button".into(),
            },
        ),
        (
            "3. act         (execute_action inc.click)",
            AgentRequest::ExecuteAction {
                agent_id: "inc".into(),
                action: "click".into(),
                params: serde_json::Value::Null,
            },
        ),
        (
            "4. verify      (get_state count)",
            AgentRequest::GetState {
                agent_id: "count".into(),
            },
        ),
        ("5. check       (validate)", AgentRequest::Validate),
    ];

    // Correctness first: a benchmark of a loop that does not work is noise.
    {
        let mut d = driver();
        let before = d.model().count;
        let act = d.process_request(&steps[2].1);
        assert!(act.success, "execute_action must succeed");
        let after = d.model().count;
        assert_eq!(after, before + 1, "the click must actually change state");
        let verify = d.process_request(&steps[3].1);
        let state = verify.data.expect("get_state returns data");
        let shown = serde_json::to_string(&state).unwrap();
        assert!(
            shown.contains(&format!("Count: {after}")),
            "agent must read the new value back: {shown}"
        );
        println!("closed-loop check: clicked inc, state now {shown}\n");
    }

    // Reading the whole widget catalogue is session setup, not part of the
    // loop: an agent learns what a Button is once and then works. Timed on
    // its own rather than folded into a per-round total it would dominate.
    {
        let mut d = driver();
        let req = AgentRequest::QueryOntology {
            query: None,
            role: None,
        };
        let types = d
            .process_request(&req)
            .data
            .and_then(|v| v.as_array().map(Vec::len))
            .unwrap_or(0);
        let mut once = Duration::MAX;
        for _ in 0..ROUNDS {
            let t = Instant::now();
            black_box(d.process_request(&req));
            once = once.min(t.elapsed());
        }
        println!("session setup: query_ontology returned {types} widget types in {}
", fmt(once));
    }

    println!("Agent loop on a running GUI — {ROUNDS} rounds, min per step");
    println!("{:<44} {:>11}", "", "min");

    let mut mins = vec![Duration::MAX; steps.len()];
    let mut whole = Duration::MAX;
    for _ in 0..ROUNDS {
        let mut d = driver();
        let t_all = Instant::now();
        for (i, (_, req)) in steps.iter().enumerate() {
            let t = Instant::now();
            black_box(d.process_request(req));
            let e = t.elapsed();
            if e < mins[i] {
                mins[i] = e;
            }
        }
        let e = t_all.elapsed();
        if e < whole {
            whole = e;
        }
    }

    for (i, (label, _)) in steps.iter().enumerate() {
        println!("{label:<44} {:>11}", fmt(mins[i]));
    }
    println!("{:-<56}", "");
    println!("{:<44} {:>11}", "full discover→act→verify loop", fmt(whole));
    println!(
        "\n{:.0} complete agent loops per second, single-threaded, no GPU.",
        1.0 / whole.as_secs_f64()
    );

    // Re-polling a tree that has not changed is the commonest thing an agent
    // does and the most wasteful: a full render and a full serialisation to
    // learn nothing. Passing the version it last saw turns that into a
    // comparison.
    {
        let mut d = driver();
        let first = d.process_request(&AgentRequest::GetTree { since: None, viewport: None });
        let version = first
            .data
            .as_ref()
            .and_then(|v| v.get("version"))
            .and_then(serde_json::Value::as_u64)
            .expect("a tree reply carries the version it was taken at");

        let unchanged = d.process_request(&AgentRequest::GetTree {
                since: Some(version),
                viewport: None,
            });
        assert_eq!(
            unchanged.data.as_ref().and_then(|v| v.get("unchanged")),
            Some(&serde_json::json!(true)),
            "an unchanged tree must be reported as unchanged, not resent"
        );

        let mut full = Duration::MAX;
        let mut poll = Duration::MAX;
        for _ in 0..ROUNDS {
            let t = Instant::now();
            black_box(d.process_request(&AgentRequest::GetTree { since: None, viewport: None }));
            full = full.min(t.elapsed());

            let t = Instant::now();
            black_box(d.process_request(&AgentRequest::GetTree {
                since: Some(version),
                viewport: None,
            }));
            poll = poll.min(t.elapsed());
        }
        println!("
Polling a tree that has not changed");
        println!("{:<44} {:>11}", "get_tree (full)", fmt(full));
        println!(
            "{:<44} {:>11}",
            "get_tree since=version (unchanged)",
            fmt(poll)
        );
        println!(
            "{:<44} {:>10.0}x",
            "cost of learning nothing, avoided",
            full.as_secs_f64() / poll.as_secs_f64()
        );
    }
}
