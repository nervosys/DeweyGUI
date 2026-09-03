//! What it costs an agent to find out what is on screen.
//!
//! Every other benchmark here assumes the agent uses the protocol. This one
//! measures the assumption. A model that has not been told the application
//! describes itself reads the source instead — and for egui and iced, which
//! have no runtime introspection at all, reading the source is not a mistake,
//! it is the only thing available. Those are the frameworks a model has seen
//! most of in training, so it is also the habit it arrives with.
//!
//! Five questions an agent actually has to answer while driving a GUI, priced
//! three ways: ask the running application, read its source, or look at a
//! picture of it.
//!
//! # What this does not measure
//!
//! Not model behaviour. Nothing here calls a model or observes what one
//! chooses; it prices the strategies a model picks between. Whether it picks
//! well is what `INSTRUCTIONS` in `src/agent/mcp.rs` is for.
//!
//! Token counts are estimated, and bytes are exact. The estimator is
//! documented at [`tokens`] and is deliberately generous to the source-reading
//! side. Ratios between strategies are what matters and they survive a
//! different tokenizer; the absolute numbers will not match any particular
//! one.
//!
//! The source figures are a best case for source-reading, by a wide margin.
//! This TodoMVC is a single self-contained file of about 3 kB. A real
//! application is many files, and an agent that does not know which one to
//! read pays for finding out before it pays for reading.

use dewey::agent::driver::HeadlessDriver;
use dewey::agent::protocol::AgentRequest;
use dewey::prelude::*;
use dewey::widget::input::TextInputState;
use dewey::widget::{Checkbox, StatefulWidget, TextInput};
use std::cell::RefCell;
use std::path::Path;

// ── the canonical application ───────────────────────────────────────────
// The same TodoMVC as `todo_dewey.rs`, which is the file the source column
// prices. Kept in step by `the_priced_source_is_the_app_under_test` below.

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

// ── measuring ───────────────────────────────────────────────────────────

/// An estimate of how many tokens a string costs.
///
/// A real tokenizer is a dependency and a download; this is arithmetic. Runs
/// of letters and digits cost one token per four characters rounded up, which
/// is the usual approximation for prose and identifiers, and every other
/// non-space character costs one, because punctuation is where byte-count
/// approximations go wrong. JSON is punctuation-dense and source is not, so
/// this errs *against* the protocol and in favour of source-reading — which is
/// the direction an honest estimate should err here.
fn tokens(text: &str) -> usize {
    let mut total: usize = 0;
    let mut run: usize = 0;
    for ch in text.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            run += 1;
        } else {
            total += run.div_ceil(4);
            run = 0;
            if !ch.is_whitespace() {
                total += 1;
            }
        }
    }
    total + run.div_ceil(4)
}

/// What one observation costs, and whether it answers the question at all.
struct Cost {
    bytes: usize,
    tokens: usize,
    answers: bool,
}

impl Cost {
    fn of(text: &str, answers: bool) -> Self {
        Self {
            bytes: text.len(),
            tokens: tokens(text),
            answers,
        }
    }

    fn show(&self) -> String {
        if self.bytes == 0 {
            return "         —".to_string();
        }
        let mark = if self.answers { ' ' } else { '*' };
        format!("{:>6} tok{mark}", self.tokens)
    }
}

fn source(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {relative}: {e}"))
}

fn driver() -> HeadlessDriver<App> {
    let mut d = HeadlessDriver::new(
        App {
            todos: vec![
                Todo {
                    title: "write the benchmark".into(),
                    done: true,
                },
                Todo {
                    title: "read the numbers".into(),
                    done: false,
                },
                Todo {
                    title: "believe them".into(),
                    done: false,
                },
            ],
            filter: Filter::All,
            input: RefCell::new(TextInputState::new()),
        },
        480.0,
        400.0,
    );
    d.init();
    d
}

fn reply(d: &mut HeadlessDriver<App>, request: &AgentRequest) -> String {
    d.process_request_json(request)
}

// ── the questions ───────────────────────────────────────────────────────

fn main() {
    let mut d = driver();

    let tree = reply(
        &mut d,
        &AgentRequest::GetTree {
            since: None,
            viewport: None,
        },
    );
    let catalogue = reply(
        &mut d,
        &AgentRequest::QueryOntology {
            query: None,
            role: None,
        },
    );
    let one_widget = reply(
        &mut d,
        &AgentRequest::GetState {
            agent_id: "remaining".into(),
        },
    );
    let unchanged = {
        let version = serde_json::from_str::<serde_json::Value>(&tree)
            .ok()
            .and_then(|v| v["data"]["version"].as_u64())
            .expect("a tree reply carries its version");
        reply(
            &mut d,
            &AgentRequest::GetTree {
                since: Some(version),
                viewport: None,
            },
        )
    };

    let dewey_src = source("src/bin/todo_dewey.rs");
    let egui_src = source("src/bin/todo_egui.rs");
    let iced_src = source("src/bin/todo_iced.rs");

    // A screenshot of this application, as the framework actually produces one.
    let screenshot = reply(
        &mut d,
        &AgentRequest::Screenshot {
            format: "text".into(),
        },
    );

    println!("Observation cost — TodoMVC, three todos, one completed");
    println!("Tokens estimated (see `tokens`); * = the strategy cannot answer the question.\n");

    let questions: Vec<(&str, Cost, Cost, Cost, Cost)> = vec![
        (
            // Structure. Source can answer this one: the id is written in it.
            "which widget adds a todo?",
            Cost::of(&tree, true),
            Cost::of(&dewey_src, true),
            Cost::of(&egui_src, true),
            Cost::of(&iced_src, true),
        ),
        (
            // State. Source describes a program, not a run of it. It cannot
            // answer this at any price, and that is the finding — not the
            // token counts.
            "is the second todo completed?",
            Cost::of(&tree, true),
            Cost::of(&dewey_src, false),
            Cost::of(&egui_src, false),
            Cost::of(&iced_src, false),
        ),
        (
            "what does the footer say?",
            Cost::of(&one_widget, true),
            Cost::of(&dewey_src, false),
            Cost::of(&egui_src, false),
            Cost::of(&iced_src, false),
        ),
        (
            "what can I do to this interface?",
            Cost::of(&catalogue, true),
            Cost::of(&dewey_src, true),
            Cost::of(&egui_src, true),
            Cost::of(&iced_src, true),
        ),
        (
            // The loop an agent spends most of its time in: has anything
            // changed since I last looked?
            "has anything changed since I looked?",
            Cost::of(&unchanged, true),
            Cost::of(&dewey_src, false),
            Cost::of(&egui_src, false),
            Cost::of(&iced_src, false),
        ),
    ];

    println!(
        "{:<38} {:>11} {:>11} {:>11} {:>11}",
        "question", "ask dewey", "read dewey", "read egui", "read iced"
    );
    for (q, ask, dewey, egui, iced) in &questions {
        println!(
            "{q:<38} {} {} {} {}",
            ask.show(),
            dewey.show(),
            egui.show(),
            iced.show()
        );
    }

    let answerable = questions.iter().filter(|q| q.2.answers).count();
    println!(
        "\nReading the source answers {answerable} of the five. The other {} are questions\n\
         about a run of the program, and source describes the program.",
        questions.len() - answerable
    );

    // ── the result that surprised me ────────────────────────────────────
    println!(
        "\nA full `get_tree` costs more than this application's entire source.\n\
         {} tokens against {} for todo_egui.rs. The tree describes every widget\n\
         with its bounds and state; the source is ninety-five lines of dense\n\
         Rust. On an application this small, an agent that reads the source once\n\
         and never looks again has paid less than one observation.\n\
         \n\
         That is the honest shape of it, and it is not where the argument is.",
        tokens(&tree),
        tokens(&egui_src)
    );

    // ── a session, which is where the argument is ───────────────────────
    // An agent does not observe once. It observes, acts, and observes again to
    // see whether the action worked, for as many steps as the task takes. The
    // nine-step TodoMVC workflow in `agent_task.rs` is the shape.
    const STEPS: usize = 9;

    // With no runtime introspection, the only way to see the result of an
    // action is a picture. Claude prices an image at about (width * height) /
    // 750 tokens, so this 480x400 window is ~256 — the cheapest an image gets,
    // and it still has to be read by a vision model that may misread it.
    let screenshot_tokens = (480 * 400) / 750;

    let strategies: [(&str, usize, &str); 4] = [
        (
            "egui: source + a screenshot per step",
            tokens(&egui_src) + STEPS * screenshot_tokens,
            "the only option: no runtime introspection exists",
        ),
        (
            "iced: source + a screenshot per step",
            tokens(&iced_src) + STEPS * screenshot_tokens,
            "likewise",
        ),
        (
            "dewey, unprompted: source + screenshots",
            tokens(&dewey_src) + STEPS * screenshot_tokens,
            "the failure mode this benchmark exists for",
        ),
        (
            "dewey, prompted: tree once, then targeted reads",
            tokens(&tree) + STEPS * tokens(&one_widget),
            "read what changed, not the whole screen",
        ),
    ];

    println!("\nA nine-step task, priced end to end:\n");
    for (name, cost, why) in &strategies {
        println!("  {name:<48} {cost:>6} tok   {why}");
    }

    let unprompted = strategies[2].1;
    let prompted = strategies[3].1;
    println!(
        "\nSame framework, same task, same model: {:.0}% more tokens for not knowing\n\
         the application describes itself ({unprompted} against {prompted}).",
        (unprompted as f64 / prompted as f64 - 1.0) * 100.0
    );

    // Where the crossover is, and what moves it.
    //
    // Asking pays a fixed premium — the first tree costs more than this toy's
    // whole source — and then saves on every observation after it. So the
    // question is how many steps it takes to earn the premium back, and the
    // answer depends almost entirely on how large the application is.
    let per_step_source = screenshot_tokens;
    let per_step_asking = tokens(&one_widget);
    let saving = per_step_source - per_step_asking;
    let crossover =
        |source: usize| -> f64 { (tokens(&tree) as f64 - source as f64) / saving as f64 };

    println!(
        "\nAsking pays {} tokens up front for the first tree, then saves {saving} on every\n\
         observation after it ({per_step_asking} against {per_step_source}). How many steps that \
         takes to earn back\ndepends on the size of the application, which is the term that \
         actually moves:\n",
        tokens(&tree)
    );
    for (what, size) in [
        ("todo_egui.rs, 95 lines", tokens(&egui_src)),
        ("todo_dewey.rs, 121 lines", tokens(&dewey_src)),
        ("three times the size", tokens(&egui_src) * 3),
        ("sixteen times the size", tokens(&egui_src) * 16),
    ] {
        let steps = crossover(size);
        if steps <= 0.0 {
            println!("  {what:<26} {size:>6} tok of source   ahead immediately");
        } else {
            println!("  {what:<26} {size:>6} tok of source   ahead after {steps:.0} steps");
        }
    }
    println!(
        "\nThis TodoMVC is about the smallest application anyone would write, in a\n\
         single file, and it is the only size at which reading is competitive. An\n\
         agent that does not already know which file to read pays for finding out\n\
         before it pays for reading."
    );

    println!("\nThe pieces, in bytes and estimated tokens:");
    for (what, text) in [
        ("get_tree (full)", &tree),
        ("get_tree since=version", &unchanged),
        ("get_state (one widget)", &one_widget),
        ("query_ontology (catalogue)", &catalogue),
        ("screenshot format=text", &screenshot),
        ("todo_dewey.rs", &dewey_src),
        ("todo_egui.rs", &egui_src),
        ("todo_iced.rs", &iced_src),
    ] {
        println!(
            "  {what:<28} {:>7} bytes  {:>6} tok",
            text.len(),
            tokens(text)
        );
    }
    println!(
        "  {:<28} {:>7} {:>13} tok",
        "a 480x400 screenshot", "-", screenshot_tokens
    );

    // ── what the numbers have to keep being true ────────────────────────
    // Compiling a benchmark is not running one, and printing a table is not
    // checking it. These are the claims the table is used to make. The first
    // version of this file asserted that asking always costs less than
    // reading; it does not, and finding that out is the reason to run a
    // benchmark rather than write a paragraph.

    assert!(
        tokens(&one_widget) < tokens(&egui_src) / 4,
        "a targeted read is the cheap path and must stay far under a source \
         file: {} vs {}",
        tokens(&one_widget),
        tokens(&egui_src)
    );
    assert!(
        tokens(&unchanged) * 20 < tokens(&tree),
        "an unchanged poll must be far cheaper than a full read, or the \
         conditional path is not worth advertising: {} vs {}",
        tokens(&unchanged),
        tokens(&tree)
    );
    assert!(
        prompted < unprompted,
        "over a realistic task, asking must beat reading, or the ontology is \
         not paying for itself: {prompted} vs {unprompted}"
    );
    assert!(
        saving > 0,
        "a targeted read must cost less than a screenshot, or there is nothing \
         to earn the first tree back with"
    );
    assert!(
        crossover(tokens(&egui_src) * 3) <= 0.0,
        "on an application three times the size of this toy — still tiny — \
         asking must be ahead from the first observation, or the case for an \
         ontology rests on applications nobody ships"
    );
    assert!(
        tokens(&catalogue) > tokens(&tree) * 2,
        "`query_ontology` is the expensive call, which is why the MCP \
         instructions tell a model to start with `get_tree`. If that stops \
         being true the advice needs revisiting: catalogue={} tree={}",
        tokens(&catalogue),
        tokens(&tree)
    );
    assert!(
        tree.contains("item_1"),
        "the tree must name the widgets an agent addresses, or the ask column \
         is not answering the question"
    );
    assert!(
        tree.contains("write the benchmark"),
        "the tree must carry state the source cannot know"
    );
    assert!(
        !egui_src.contains("write the benchmark") && !dewey_src.contains("write the benchmark"),
        "the source files must not contain the running state, or the claim \
         that they cannot answer a state question is false"
    );

    // The source column prices the file this benchmark's app was copied from.
    // If they drift, the comparison is against something that is not the
    // application under test.
    for id in [
        "new_todo",
        "add",
        "filter_all",
        "clear_completed",
        "remaining",
    ] {
        assert!(
            dewey_src.contains(id),
            "todo_dewey.rs no longer contains `{id}`, so it is not the \
             application this benchmark measures"
        );
        assert!(
            tree.contains(id),
            "the rendered tree no longer contains `{id}`"
        );
    }

    println!("\nall assertions hold");
}
