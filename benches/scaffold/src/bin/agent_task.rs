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

#[derive(Clone, Copy, PartialEq)]
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

    fn add(&mut self) {
        let title = self.input.borrow().text.trim().to_string();
        if !title.is_empty() {
            self.todos.push(Todo { title, done: false });
            *self.input.borrow_mut() = TextInputState::new();
        }
    }
}

impl Model for App {
    type Msg = ();

    fn update(&mut self, _msg: ()) -> Command<()> {
        Command::None
    }

    fn view(&self, frame: &mut Frame<'_>) {
        let h = frame.area.height;
        let rows = frame.area.rows_of(&[36.0, 32.0, h - 96.0, 28.0]);

        let top = rows[0].cols_of(&[rows[0].width - 80.0, 80.0]);
        TextInput::new()
            .placeholder("What needs doing?")
            .on_input("new_todo", |a: &mut App, t: &str| {
                *a.input.borrow_mut() = TextInputState::new().with_text(t)
            })
            .render(top[0], frame, &mut self.input.borrow_mut());
        Button::new("Add").on("add", App::add).render(top[1], frame);

        let f = rows[1].split_columns(3);
        Button::new("All")
            .on("filter_all", |a: &mut App| a.filter = Filter::All)
            .render(f[0], frame);
        Button::new("Active")
            .on("filter_active", |a: &mut App| a.filter = Filter::Active)
            .render(f[1], frame);
        Button::new("Completed")
            .on("filter_completed", |a: &mut App| {
                a.filter = Filter::Completed
            })
            .render(f[2], frame);

        for (i, row) in self.visible().into_iter().zip(rows[2].rows(28.0)) {
            let c = row.cols_of(&[24.0, row.width - 52.0, 28.0]);
            Checkbox::new("", self.todos[i].done)
                .on(format!("toggle_{i}"), move |a: &mut App| {
                    a.todos[i].done = !a.todos[i].done
                })
                .render(c[0], frame);
            Label::new(self.todos[i].title.clone())
                .agent_id(format!("item_{i}"))
                .render(c[1], frame);
            Button::new("x")
                .on(format!("delete_{i}"), move |a: &mut App| {
                    a.todos.remove(i);
                })
                .render(c[2], frame);
        }

        let foot = rows[3].cols_of(&[rows[3].width - 140.0, 140.0]);
        Label::new(format!("{} items left", self.remaining()))
            .agent_id("remaining")
            .render(foot[0], frame);
        Button::new("Clear completed")
            .on("clear_completed", |a: &mut App| a.todos.retain(|t| !t.done))
            .render(foot[1], frame);
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
    act(id, "click")
}

/// A `Checkbox` advertises `toggle`, not `click`. Naming the action the
/// widget publishes is what an agent reading the ontology would do — and
/// `click` here reported success and changed nothing until the protocol
/// started refusing actions a widget never advertised.
fn act(id: &str, action: &'static str) -> AgentRequest {
    AgentRequest::ExecuteAction {
        agent_id: id.into(),
        action: action.into(),
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
        (
            "discover        (get_tree)",
            AgentRequest::GetTree {
                since: None,
                viewport: None,
            },
        ),
        (
            "type item 1     (set_text)",
            set_text("new_todo", "write tests"),
        ),
        ("add item 1      (click add)", click("add")),
        (
            "type item 2     (set_text)",
            set_text("new_todo", "ship it"),
        ),
        ("add item 2      (click add)", click("add")),
        (
            "complete item 1 (toggle_0.toggle)",
            act("toggle_0", "toggle"),
        ),
        (
            "filter active   (click filter_active)",
            click("filter_active"),
        ),
        (
            "re-read tree    (get_tree)",
            AgentRequest::GetTree {
                since: None,
                viewport: None,
            },
        ),
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

        let tree = d
            .process_request(&AgentRequest::GetTree {
                since: None,
                viewport: None,
            })
            .data
            .unwrap();
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
        assert!(
            state.contains("1 items left"),
            "agent reads the count: {state}"
        );
        println!(
            "task verified: 2 added, 1 completed, filter=active, footer reads \"1 items left\""
        );
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
    let first = d.process_request(&AgentRequest::GetTree {
        since: None,
        viewport: None,
    });
    let v = first.data.unwrap()["version"].as_u64().unwrap();

    let mut uncond = Duration::MAX;
    let mut cond = Duration::MAX;
    for _ in 0..ROUNDS {
        let t = Instant::now();
        black_box(d.process_request(&AgentRequest::GetTree {
            since: None,
            viewport: None,
        }));
        let e = t.elapsed();
        if e < uncond {
            uncond = e;
        }
        let t = Instant::now();
        black_box(d.process_request(&AgentRequest::GetTree {
            since: Some(v),
            viewport: None,
        }));
        let e = t.elapsed();
        if e < cond {
            cond = e;
        }
    }
    println!();
    println!("polling an unchanged screen, interleaved, min of {ROUNDS}:");
    println!("  get_tree                {:>10}", fmt(uncond));
    println!(
        "  get_tree since=version  {:>10}   ({:.0}x less)",
        fmt(cond),
        uncond.as_secs_f64() / cond.as_secs_f64()
    );
}
