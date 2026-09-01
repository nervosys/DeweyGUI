//! A realistic multi-step agent task against the complex canonical app.
//!
//! The counter benchmark times one click. This drives TodoMVC the way an agent
//! actually would after scaffolding it: add two items, complete one, switch
//! filter, and read back enough state to prove the app behaves. Every step is
//! asserted before anything is timed.

use dewey::agent::driver::HeadlessDriver;
use dewey::agent::protocol::AgentRequest;
use dewey::prelude::*;
use dewey::widget::input::TextInputState;
use dewey::widget::{Checkbox, StatefulWidget, TextInput};
use std::cell::RefCell;
use std::hint::black_box;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Debug)]
enum Filter {
    All,
    Active,
    Completed,
}

struct Todo {
    title: String,
    done: bool,
}

struct App {
    todos: Vec<Todo>,
    filter: Filter,
    input: RefCell<TextInputState>,
}

#[derive(Debug)]
enum Msg {
    Add,
    Toggle(usize),
    Delete(usize),
    SetFilter(Filter),
    ClearCompleted,
}

impl App {
    fn visible(&self) -> Vec<usize> {
        (0..self.todos.len())
            .filter(|&i| match self.filter {
                Filter::All => true,
                Filter::Active => !self.todos[i].done,
                Filter::Completed => self.todos[i].done,
            })
            .collect()
    }

    fn remaining(&self) -> usize {
        self.todos.iter().filter(|t| !t.done).count()
    }
}

impl Model for App {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) -> Command<Msg> {
        match msg {
            Msg::Add => {
                let title = self.input.borrow().text.trim().to_string();
                if !title.is_empty() {
                    self.todos.push(Todo { title, done: false });
                    *self.input.borrow_mut() = TextInputState::new();
                }
            }
            Msg::Toggle(i) => {
                if let Some(t) = self.todos.get_mut(i) {
                    t.done = !t.done;
                }
            }
            Msg::Delete(i) => {
                if i < self.todos.len() {
                    self.todos.remove(i);
                }
            }
            Msg::SetFilter(f) => self.filter = f,
            Msg::ClearCompleted => self.todos.retain(|t| !t.done),
        }
        Command::None
    }

    fn view(&self, frame: &mut Frame<'_>) {
        let rows = Layout::vertical([
            Constraint::Length(36.0),
            Constraint::Length(32.0),
            Constraint::Fill(1.0),
            Constraint::Length(28.0),
        ])
        .split(frame.area);

        let top =
            Layout::horizontal([Constraint::Fill(1.0), Constraint::Length(80.0)]).split(rows[0]);
        TextInput::new()
            .placeholder("What needs doing?")
            .agent_id("new_todo")
            .render(top[0], frame, &mut self.input.borrow_mut());
        Button::new("Add").action("add", Msg::Add).render(top[1], frame);

        let f = Layout::horizontal([Constraint::Ratio(1, 3); 3]).split(rows[1]);
        Button::new("All").action("filter_all", Msg::SetFilter(Filter::All)).render(f[0], frame);
        Button::new("Active").action("filter_active", Msg::SetFilter(Filter::Active)).render(f[1], frame);
        Button::new("Completed").action("filter_completed", Msg::SetFilter(Filter::Completed)).render(f[2], frame);

        for (idx, row) in self.visible().into_iter().zip(rows[2].rows(28.0)) {
            let cols = Layout::horizontal([
                Constraint::Length(24.0),
                Constraint::Fill(1.0),
                Constraint::Length(28.0),
            ])
            .split(row);
            Checkbox::new("", self.todos[idx].done)
                .action(format!("toggle_{idx}"), Msg::Toggle(idx))
                .render(cols[0], frame);
            Label::new(self.todos[idx].title.clone())
                .agent_id(format!("item_{idx}"))
                .render(cols[1], frame);
            Button::new("x")
                .action(format!("delete_{idx}"), Msg::Delete(idx))
                .render(cols[2], frame);
        }

        let foot =
            Layout::horizontal([Constraint::Fill(1.0), Constraint::Length(140.0)]).split(rows[3]);
        Label::new(format!("{} items left", self.remaining()))
            .agent_id("remaining")
            .render(foot[0], frame);
        Button::new("Clear completed")
            .action("clear_completed", Msg::ClearCompleted)
            .render(foot[1], frame);
    }

    /// Only the text field still needs a handler: a `TextInput` carries state,
    /// not a message.
    fn execute_action(&mut self, id: &str, action: &str, p: &serde_json::Value) -> serde_json::Value {
        if (id, action) == ("new_todo", "set_text") {
            let text = p.get("text").and_then(|v| v.as_str()).unwrap_or("");
            *self.input.borrow_mut() = TextInputState::new().with_text(text);
        }
        serde_json::json!({ "todos": self.todos.len(), "remaining": self.remaining() })
    }
}

fn set_text(id: &str, text: &str) -> AgentRequest {
    AgentRequest::ExecuteAction {
        agent_id: id.into(),
        action: "set_text".into(),
        params: serde_json::json!({ "text": text }),
    }
}

fn click(id: &str) -> AgentRequest {
    AgentRequest::ExecuteAction {
        agent_id: id.into(),
        action: "click".into(),
        params: serde_json::Value::Null,
    }
}

fn driver() -> HeadlessDriver<App> {
    let mut d = HeadlessDriver::new(
        App {
            todos: Vec::new(),
            filter: Filter::All,
            input: RefCell::new(TextInputState::new()),
        },
        480.0,
        400.0,
    );
    d.init();
    d
}

fn task() -> Vec<(&'static str, AgentRequest)> {
    vec![
        ("discover        (get_tree)", AgentRequest::GetTree { since: None }),
        ("type item 1     (set_text)", set_text("new_todo", "write tests")),
        ("add item 1      (click add)", click("add")),
        ("type item 2     (set_text)", set_text("new_todo", "ship it")),
        ("add item 2      (click add)", click("add")),
        ("complete item 1 (click toggle_0)", click("toggle_0")),
        ("filter active   (click filter_active)", click("filter_active")),
        ("re-read tree    (get_tree)", AgentRequest::GetTree { since: None }),
        (
            "verify counter  (get_state remaining)",
            AgentRequest::GetState {
                agent_id: "remaining".into(),
            },
        ),
    ]
}

fn fmt(d: Duration) -> String {
    let us = d.as_secs_f64() * 1e6;
    if us < 1.0 {
        return format!("{:.0} ns", d.as_secs_f64() * 1e9);
    }
    if us >= 1000.0 {
        format!("{:.2} ms", us / 1000.0)
    } else {
        format!("{us:.1} us")
    }
}

fn main() {
    const ROUNDS: usize = 2_000;

    // Prove the task actually works before timing it.
    {
        let mut d = driver();
        for (label, req) in task() {
            let r = d.process_request(&req);
            assert!(r.success, "step failed: {label}");
        }
        assert_eq!(d.model().todos.len(), 2, "two todos were added");
        assert!(d.model().todos[0].done, "first todo was completed");
        assert_eq!(d.model().remaining(), 1, "one item left");

        let tree = d.process_request(&AgentRequest::GetTree { since: None }).data.unwrap();
        let shown = serde_json::to_string(&tree).unwrap();
        assert!(
            shown.contains("ship it"),
            "the active filter must still show the incomplete item"
        );
        assert!(
            !shown.contains("write tests"),
            "the completed item must be filtered out of the tree"
        );
        let state = d
            .process_request(&AgentRequest::GetState {
                agent_id: "remaining".into(),
            })
            .data
            .unwrap();
        let state = serde_json::to_string(&state).unwrap();
        assert!(state.contains("1 items left"), "agent reads the count: {state}");
        println!("task verified: 2 added, 1 completed, filter=active, footer reads \"1 items left\"");
        println!("the agent proved all of that with no window, no GPU, no screenshot.\n");
    }

    let steps = task();
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

    println!("TodoMVC agent task - {ROUNDS} rounds, min per step");
    println!("{:<40} {:>11}", "", "min");
    for (i, (label, _)) in steps.iter().enumerate() {
        println!("{label:<40} {:>11}", fmt(mins[i]));
    }
    println!("{:-<52}", "");
    println!("{:<40} {:>11}", "full 9-step task", fmt(whole));
    println!(
        "\n{:.0} complete task runs per second, single-threaded, no GPU.",
        1.0 / whole.as_secs_f64()
    );

    // What an agent pays to poll a screen that has not moved.
    let mut d = driver();
    let first = d.process_request(&AgentRequest::GetTree { since: None });
    let v = first.data.unwrap()["version"].as_u64().unwrap();

    let mut uncond = Duration::MAX;
    let mut cond = Duration::MAX;
    for _ in 0..ROUNDS {
        let t = Instant::now();
        black_box(d.process_request(&AgentRequest::GetTree { since: None }));
        let e = t.elapsed();
        if e < uncond {
            uncond = e;
        }
        let t = Instant::now();
        black_box(d.process_request(&AgentRequest::GetTree { since: Some(v) }));
        let e = t.elapsed();
        if e < cond {
            cond = e;
        }
    }
    println!("");
    println!("polling an unchanged screen, interleaved, min of {ROUNDS}:");
    println!("  get_tree                {:>10}", fmt(uncond));
    println!(
        "  get_tree since=version  {:>10}   ({:.0}x less)",
        fmt(cond),
        uncond.as_secs_f64() / cond.as_secs_f64()
    );
}
