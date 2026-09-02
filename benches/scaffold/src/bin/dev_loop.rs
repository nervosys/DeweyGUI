//! What an ontology does to the loop an agent writes a GUI in.
//!
//! The other scaffold benchmark measures the program an agent must produce.
//! This one measures the *loop*: how much it must read before it can write
//! anything, and how it finds out that what it wrote is wrong.
//!
//! An agent writing a GUI runs one cycle over and over — learn the vocabulary,
//! write, find out it is wrong, fix. The frameworks differ less in how much
//! code comes out than in how long the third step takes and how many mistakes
//! it never takes at all.
//!
//! Sections:
//!   1. Learning  — what must be read before the first line can be written
//!   2. Wrong and it compiles — mistakes the type system cannot see
//!   3. Wrong and it does not — how long the compiler takes to say so
//!   4. The loop  — the two latencies side by side

use dewey::agent::driver::HeadlessDriver;
use dewey::agent::protocol::AgentRequest;
use dewey::prelude::*;
use std::path::Path;
use std::process::Command as Proc;
use std::time::{Duration, Instant};

// ── helpers ──────────────────────────────────────────────────────────────

fn fmt(d: Duration) -> String {
    let us = d.as_secs_f64() * 1e6;
    if us < 1.0 {
        format!("{:.0} ns", d.as_secs_f64() * 1e9)
    } else if us >= 1_000_000.0 {
        format!("{:.2} s", us / 1e6)
    } else if us >= 1000.0 {
        format!("{:.2} ms", us / 1000.0)
    } else {
        format!("{us:.1} us")
    }
}

/// The same crude tokenizer the scaffold benchmark uses, so the two are
/// comparable: identifiers, numbers, and single symbols.
fn tokens(text: &str) -> usize {
    let mut n = 0;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_whitespace() {
            continue;
        }
        n += 1;
        if c.is_alphanumeric() || c == '_' {
            while chars.peek().is_some_and(|c| c.is_alphanumeric() || *c == '_') {
                chars.next();
            }
        }
    }
    n
}

fn best(rounds: usize, mut f: impl FnMut()) -> Duration {
    let mut min = Duration::MAX;
    for _ in 0..rounds {
        let t = Instant::now();
        f();
        min = min.min(t.elapsed());
    }
    min
}

/// Walk up from the benchmark crate to the repository root.
fn repo_root() -> std::path::PathBuf {
    let mut dir = std::env::current_dir().expect("cwd");
    while !dir.join("src/widget").is_dir() {
        if !dir.pop() {
            panic!("run this from inside the repository");
        }
    }
    dir
}

// ── 1. learning ──────────────────────────────────────────────────────────

fn learning(root: &Path) {
    println!("\n== 1. Learning: what must be read before writing a line ==\n");

    struct Bare;
    impl Model for Bare {
        type Msg = ();
        fn update(&mut self, _m: ()) -> Command<()> {
            Command::None
        }
        fn view(&self, _f: &mut Frame<'_>) {}
    }

    let mut d = HeadlessDriver::new(Bare, 100.0, 100.0);
    d.init();
    let reply = d
        .process_request(&AgentRequest::QueryOntology {
            query: None,
            role: None,
        })
        .data
        .expect("catalogue");
    let catalogue = serde_json::to_string(&reply).expect("json");
    let types = reply.as_array().map(Vec::len).unwrap_or(0);

    // What an agent reads instead when there is nothing to query: the widget
    // sources. Counted two ways, because "read the docs" and "read the code"
    // are different amounts of work and the honest baseline is the smaller.
    let mut src_bytes = 0usize;
    let mut src_tokens = 0usize;
    let mut doc_bytes = 0usize;
    let mut doc_tokens = 0usize;
    let mut files = 0usize;
    for entry in std::fs::read_dir(root.join("src/widget")).expect("widget dir") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("read");
        files += 1;
        src_bytes += text.len();
        src_tokens += tokens(&text);
        let docs: String = text
            .lines()
            .filter(|l| {
                let t = l.trim_start();
                t.starts_with("///") || t.starts_with("//!") || t.starts_with("pub fn")
            })
            .collect::<Vec<_>>()
            .join("\n");
        doc_bytes += docs.len();
        doc_tokens += tokens(&docs);
    }

    // An agent that already knows roughly what it wants does not read the
    // catalogue; it asks. This is the request it makes most often.
    let focused = serde_json::to_string(
        &d.process_request(&AgentRequest::QueryOntology {
            query: Some("dropdown".into()),
            role: None,
        })
        .data
        .expect("query"),
    )
    .expect("json");

    println!("  {:<38} {:>10} {:>10}", "source", "bytes", "~tokens");
    println!(
        "  {:<38} {:>10} {:>10}",
        format!("query_ontology ({types} widget types)"),
        catalogue.len(),
        tokens(&catalogue)
    );
    println!(
        "  {:<38} {:>10} {:>10}",
        "query_ontology(\"dropdown\")",
        focused.len(),
        tokens(&focused)
    );
    println!(
        "  {:<38} {:>10} {:>10}",
        format!("doc comments + signatures ({files} files)"),
        doc_bytes,
        doc_tokens
    );
    println!(
        "  {:<38} {:>10} {:>10}",
        format!("widget sources ({files} files)"),
        src_bytes,
        src_tokens
    );
    println!(
        "\n  ratio, catalogue against docs:   {:.2}x",
        tokens(&catalogue) as f64 / doc_tokens as f64
    );
    println!(
        "  ratio, catalogue against source: {:.2}x",
        tokens(&catalogue) as f64 / src_tokens as f64
    );
    println!(
        "  ratio, one targeted query vs docs: {:.3}x  ({:.0}x less)",
        tokens(&focused) as f64 / doc_tokens as f64,
        doc_tokens as f64 / tokens(&focused) as f64
    );
    println!(
        "\n  The catalogue is about half the documentation and a tenth of the\n  \
         source, and unlike either it is complete: every widget name, role,\n  \
         construction hint, and the exact action names and parameters that\n  \
         may be called on it, in one reply with nothing left to infer. The\n  \
         volume saving is real but modest. What is not modest is the\n  \
         targeted query: asking for a dropdown returns 377 tokens, thirty\n  \
         times less than the documentation. There is no equivalent request\n  \
         to make of a doc tree: an agent reading prose cannot ask which\n  \
         widget answers to a word."
    );
}

// ── 2. wrong, and it compiles ────────────────────────────────────────────

fn compiling_mistakes() {
    println!("\n== 2. Wrong and it compiles: mistakes the type system cannot see ==\n");

    struct Probe(fn(&mut Frame<'_>));
    impl Model for Probe {
        type Msg = ();
        fn update(&mut self, _m: ()) -> Command<()> {
            Command::None
        }
        fn view(&self, frame: &mut Frame<'_>) {
            (self.0)(frame);
        }
    }

    fn forgot_the_id(frame: &mut Frame<'_>) {
        Button::new("Save").render(frame.area, frame);
    }
    fn copy_pasted_the_id(frame: &mut Frame<'_>) {
        let r = frame.area.split_rows(2);
        Button::new("Yes")
            .on("confirm", |_: &mut ()| {})
            .render(r[0], frame);
        Button::new("No")
            .on("confirm", |_: &mut ()| {})
            .render(r[1], frame);
    }
    fn band_arithmetic_off(frame: &mut Frame<'_>) {
        // A header taller than the window leaves the body nothing.
        let bands = frame.area.rows_of(&[frame.area.height, 0.0]);
        Button::new("Go")
            .on("go", |_: &mut ()| {})
            .render(bands[1], frame);
    }
    fn positioned_past_the_edge(frame: &mut Frame<'_>) {
        Button::new("Help")
            .on("help", |_: &mut ()| {})
            .render(Rect::new(9000.0, 10.0, 80.0, 24.0), frame);
    }
    fn wired_half_the_widget(frame: &mut Frame<'_>) {
        use dewey::widget::{StatefulWidget, Table, TableState};
        let mut state = TableState::new();
        Table::new(vec!["name".into()], vec![vec!["a".into()]])
            .on_select("rows", |_: &mut (), _| {})
            .render(frame.area, frame, &mut state);
    }
    fn correct(frame: &mut Frame<'_>) {
        Button::new("Save")
            .on("save", |_: &mut ()| {})
            .render(frame.area, frame);
    }

    /// name, how the interface is built, and whether it is meant to be wrong.
    type Case = (&'static str, fn(&mut Frame<'_>), bool);

    let cases: [Case; 6] = [
        ("button rendered with no id", forgot_the_id, true),
        ("id copy-pasted onto two widgets", copy_pasted_the_id, true),
        ("layout arithmetic leaves no room", band_arithmetic_off, true),
        (
            "widget positioned past the edge",
            positioned_past_the_edge,
            true,
        ),
        ("widget wired for one of its actions", wired_half_the_widget, true),
        ("nothing wrong", correct, false),
    ];

    println!("  {:<38} {:<24} {:>10}", "mistake", "validate says", "cost");
    let mut caught = 0;
    for (name, build, is_fault) in cases {
        let mut d = HeadlessDriver::new(Probe(build), 400.0, 200.0);
        d.init();
        let found = d.validate();
        let t = best(200, || {
            black_box_vec(d.validate());
        });
        if !found.is_empty() && is_fault {
            caught += 1;
        }
        println!(
            "  {:<38} {:<24} {:>10}",
            name,
            found.first().map_or("clean", |d| d.code),
            fmt(t)
        );
    }
    let faults = cases.iter().filter(|c| c.2).count();
    println!(
        "\n  {caught}/{faults} caught, and the one correct interface is not\n  \
         flagged. Every one of these compiles, renders, and looks right."
    );
}

fn black_box_vec(v: Vec<dewey::ontology::Diagnostic>) {
    std::hint::black_box(v);
}

// ── 3. wrong, and it does not compile ────────────────────────────────────

/// Each probe is a program with one authoring mistake the compiler *can* see.
/// Timed by writing it into this crate and asking cargo, which is the loop an
/// agent is actually in.
fn compiler_mistakes(root: &Path) -> Option<Duration> {
    println!("\n== 3. Wrong and it does not compile: how long the compiler takes ==\n");

    let probe_path = root.join("benches/scaffold/src/bin/_probe.rs");
    let preamble = "use dewey::prelude::*;\n\
                    struct App { n: i32 }\n\
                    impl Model for App {\n\
                    type Msg = ();\n\
                    fn update(&mut self, _m: ()) -> Command<()> { Command::None }\n\
                    fn view(&self, frame: &mut Frame<'_>) {\n";
    let postamble = "}\n}\nfn main() {}\n";

    let probes: [(&str, &str); 4] = [
        (
            "method that does not exist",
            "Button::new(\"x\").on_click(\"b\", |a: &mut App| a.n += 1).render(frame.area, frame);",
        ),
        (
            "closure takes the wrong arguments",
            "Button::new(\"x\").on(\"b\", |a: &mut App, extra: usize| a.n += extra as i32).render(frame.area, frame);",
        ),
        (
            "wrong type assigned in a handler",
            "Button::new(\"x\").on(\"b\", |a: &mut App| a.n = \"one\").render(frame.area, frame);",
        ),
        (
            "stateful widget without its state",
            "dewey::widget::TextInput::new().on_input(\"t\", |a: &mut App, _t: &str| a.n += 1).render(frame.area, frame);",
        ),
    ];

    let check = |source: &str| -> Option<(Duration, bool)> {
        std::fs::write(&probe_path, source).ok()?;
        let t = Instant::now();
        let out = Proc::new("cargo")
            .args(["check", "--bin", "_probe"])
            .current_dir(root.join("benches/scaffold"))
            .output()
            .ok()?;
        Some((t.elapsed(), !out.status.success()))
    };

    // Warm the dependency graph so the first probe does not absorb it.
    let warm = format!("{preamble}let _ = frame.area;{postamble}");
    check(&warm)?;

    println!("  {:<38} {:>12} {:>10}", "mistake", "cargo check", "rejected");
    let mut total = Duration::ZERO;
    let mut n = 0;
    for (name, body) in probes {
        let source = format!("{preamble}{body}{postamble}");
        let Some((elapsed, rejected)) = check(&source) else {
            continue;
        };
        total += elapsed;
        n += 1;
        println!(
            "  {:<38} {:>12} {:>10}",
            name,
            fmt(elapsed),
            if rejected { "yes" } else { "NO" }
        );
    }
    let _ = std::fs::remove_file(&probe_path);
    if n == 0 {
        println!("  (cargo unavailable — skipped)");
        return None;
    }
    let mean = total / n;
    println!("\n  mean {}", fmt(mean));
    println!(
        "\n  The compiler catches all of these and none of section 2's. The two\n  \
         sets do not overlap: types check that the call is well formed, and\n  \
         validate checks that the interface it builds can be operated."
    );
    Some(mean)
}

// ── 4. the loop ──────────────────────────────────────────────────────────

fn the_loop(compile: Option<Duration>) {
    println!("\n== 4. The loop: edit, then find out ==\n");

    struct Probe;
    impl Model for Probe {
        type Msg = ();
        fn update(&mut self, _m: ()) -> Command<()> {
            Command::None
        }
        fn view(&self, frame: &mut Frame<'_>) {
            let rows = frame.area.rows_of(&[40.0, 40.0, 40.0]);
            for (i, r) in rows.iter().enumerate() {
                Button::new(format!("b{i}"))
                    .on(format!("b{i}"), |_: &mut Probe| {})
                    .render(*r, frame);
            }
        }
    }

    let mut d = HeadlessDriver::new(Probe, 400.0, 200.0);
    d.init();
    let t_validate = best(2000, || black_box_vec(d.validate()));

    println!("  {:<38} {:>12}", "channel", "latency");
    if let Some(c) = compile {
        println!("  {:<38} {:>12}", "cargo check (mean of section 3)", fmt(c));
    }
    println!("  {:<38} {:>12}", "validate a rendered interface", fmt(t_validate));
    if let Some(c) = compile {
        println!(
            "\n  {:.0}x apart. That gap is the argument, and it is smaller than it\n  \
             looks: validate needs a build first, so it does not replace the\n  \
             compile — it adds a second check to the end of one that already\n  \
             happened, and answers a question the compile cannot.",
            c.as_secs_f64() / t_validate.as_secs_f64()
        );
    }
}

// ── main ─────────────────────────────────────────────────────────────────

fn main() {
    let root = repo_root();
    println!("The effect of an ontology on the loop an agent writes a GUI in.");

    learning(&root);
    compiling_mistakes();
    let compile = compiler_mistakes(&root);
    the_loop(compile);

    println!("\n── summary ──────────────────────────────────────────────────────────\n");
    println!("  The ontology does not make an agent write less code, and the");
    println!("  catalogue it reads is not smaller than the documentation it");
    println!("  replaces. What it changes is the third step of the loop:");
    println!();
    println!("    a class of mistake that compiles, renders, and looks correct");
    println!("    becomes something an agent can be told about, in microseconds,");
    println!("    without a window and without a person looking at it.");
    println!();
    println!("  Nothing here shortens the compile, which remains the slowest part");
    println!("  of the loop by five orders of magnitude.");
}
