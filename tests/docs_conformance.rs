//! The protocol reference must describe the protocol that exists.
//!
//! It has drifted twice in one session: the handshake example showed
//! `min_version` where the field is `min_protocol_version`, and listed five
//! server capabilities where there were ten. An agent written from that
//! example reads a field that is not there — a failure one layer out from the
//! ones this project has been chasing, and one nothing was checking.
//!
//! Every JSON request in the reference is deserialised as an `AgentRequest`,
//! and every documented response is compared against what the server actually
//! answers.

use dewey::agent::driver::HeadlessDriver;
use dewey::agent::protocol::{AgentEvent, AgentRequest};
use dewey::prelude::*;
use std::path::Path;

// ── an application with the widgets the reference names ─────────────────

struct DocApp {
    count: i32,
}

impl Model for DocApp {
    type Msg = ();

    fn update(&mut self, _m: ()) -> Command<()> {
        Command::None
    }

    fn view(&self, frame: &mut Frame<'_>) {
        use dewey::widget::{StatefulWidget, TextInput};
        let rows = frame.area.rows_of(&[40.0, 40.0, 40.0]);
        Button::new("+")
            .on("inc_btn", |a: &mut DocApp| a.count += 1)
            .render(rows[0], frame);
        Label::new(format!("Count: {}", self.count))
            .agent_id("counter_label")
            .render(rows[1], frame);
        let mut state = dewey::widget::input::TextInputState::new();
        TextInput::new()
            .on_input("new_todo", |_: &mut DocApp, _t: &str| {})
            .render(rows[2], frame, &mut state);
    }
}

fn driver() -> HeadlessDriver<DocApp> {
    let mut d = HeadlessDriver::new(DocApp { count: 0 }, 480.0, 200.0);
    d.init();
    d
}

// ── extracting the examples ─────────────────────────────────────────────

/// Every fenced JSON block in the reference, in order.
///
/// `jsonc` blocks carry explanatory comments and are skipped: they exist to be
/// read, not parsed.
fn json_blocks() -> Vec<String> {
    // The README carries the headline example — the one showing that any
    // language able to write lines of JSON can drive a Dewey application — and
    // it was wrong in three ways at once: a numeric `id` where the envelope
    // takes a string, externally-tagged requests where the protocol is
    // internally tagged, and three request names that never existed. It is the
    // first thing a reader sees and the last thing anything checked.
    let mut text = String::new();
    for file in ["docs/agent-protocol.md", "README.md"] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(file);
        text.push_str(&std::fs::read_to_string(&path).expect("documentation"));
        text.push('\n');
    }

    let mut blocks = Vec::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        match (line.trim_start(), current.as_mut()) {
            ("```json", None) => current = Some(String::new()),
            ("```", Some(_)) => blocks.push(current.take().unwrap_or_default()),
            (_, Some(buffer)) => {
                buffer.push_str(line);
                buffer.push('\n');
            }
            _ => {}
        }
    }
    // A JSON Lines block holds one document per line. Parsing it whole fails,
    // and the first version of this quietly skipped such a block — which is
    // the one the README's headline example lives in, so the check passed
    // while reading nothing.
    blocks
        .into_iter()
        .flat_map(|block| {
            if serde_json::from_str::<serde_json::Value>(block.trim()).is_ok() {
                return vec![block];
            }
            block
                .lines()
                .map(str::trim)
                .filter(|l| l.starts_with('{'))
                .map(str::to_string)
                .collect()
        })
        .collect()
}

/// The keys of a JSON object, sorted.
fn keys(value: &serde_json::Value) -> Vec<String> {
    value
        .as_object()
        .map(|o| {
            let mut k: Vec<String> = o.keys().cloned().collect();
            k.sort();
            k
        })
        .unwrap_or_default()
}

// ── the checks ──────────────────────────────────────────────────────────

/// Every documented request must be a request this crate accepts.
#[test]
fn every_documented_request_deserialises() {
    let mut checked = 0;
    for block in json_blocks() {
        let value: serde_json::Value = match serde_json::from_str(block.trim()) {
            Ok(v) => v,
            // Some blocks are illustrative fragments rather than whole
            // documents; a block that is not JSON at all is not this test's
            // business.
            Err(_) => continue,
        };

        // Either a bare request or one inside an envelope.
        let request = if value.get("type").is_some() {
            &value
        } else if let Some(inner) = value.get("request") {
            inner
        } else {
            continue;
        };

        // A `type` is either a request an agent sends or an event the
        // server pushes; the reference documents both, and both must be
        // things this crate actually defines.
        let as_request = serde_json::from_value::<AgentRequest>(request.clone());
        if as_request.is_err() {
            let as_event = serde_json::from_value::<AgentEvent>(request.clone());
            assert!(
                as_event.is_ok(),
                "the reference documents a message this crate does not define: {request}"
            );
            continue;
        }
        checked += 1;
    }
    assert!(checked >= 10, "only {checked} request examples found");
}

/// A documented response must have the fields the server actually sends.
///
/// Requests are run against a real driver and the documented keys compared
/// with the answer. Extra documented keys are the failure that matters: they
/// are the ones an agent will read and not find.
#[test]
fn documented_responses_match_what_the_server_sends() {
    let mut checked = 0;
    let mut last_request: Option<AgentRequest> = None;

    for block in json_blocks() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&block) else {
            continue;
        };

        // A request example: remember it for the response that follows.
        let request_value = if value.get("type").is_some() {
            Some(value.clone())
        } else {
            value.get("request").cloned()
        };
        if let Some(request_value) = request_value {
            last_request = serde_json::from_value(request_value).ok();
            continue;
        }

        // A response example: compare it against the real answer.
        if value.get("success").is_none() {
            continue;
        }
        let Some(request) = last_request.take() else {
            continue;
        };

        let mut d = driver();
        let actual = d.process_request(&request);

        let documented = keys(&value);
        let real = {
            let mut k = vec!["success".to_string()];
            if actual.data.is_some() {
                k.push("data".into());
            }
            if actual.error.is_some() {
                k.push("error".into());
            }
            // `id` only appears when the request was framed in an envelope,
            // which the reference's examples show.
            k.push("id".into());
            k.sort();
            k
        };
        for key in &documented {
            assert!(
                real.contains(key),
                "the reference shows `{key}` in a response to {request:?}, and \
                 the server sends {real:?}"
            );
        }

        // And the `data` payload, which is where the drift was.
        if let (Some(shown), Some(sent)) = (value.get("data"), actual.data.as_ref()) {
            let shown_keys = keys(shown);
            let sent_keys = keys(sent);
            if !shown_keys.is_empty() && !sent_keys.is_empty() {
                for key in &shown_keys {
                    assert!(
                        sent_keys.contains(key),
                        "the reference shows `data.{key}` for {request:?}; the \
                         server sends {sent_keys:?}. An agent written from the \
                         reference reads a field that is not there"
                    );
                }
            }
        }
        checked += 1;
    }

    assert!(
        checked >= 1,
        "no request/response pair was compared, so this test proves nothing"
    );
}

/// Every complete Rust sample in the README must compile.
///
/// The quick start — the first code a reader meets — did not. It returned
/// `Result<(), eframe::Error>`, and `Result` in this crate's prelude is
/// Dewey's own one-parameter alias, so the signature was rejected. Nothing
/// compiled the README, so it had been wrong for as long as the alias has
/// existed.
///
/// Fragments are marked ```rust,ignore and skipped: the agpu sample continues
/// the one above it and needs a feature, and the logging line is one
/// statement.
#[test]
fn every_complete_readme_sample_compiles() {
    let readme = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md"))
        .expect("README");

    let mut samples = Vec::new();
    let mut current: Option<String> = None;
    for line in readme.lines() {
        match (line.trim_end(), current.as_mut()) {
            ("```rust", None) => current = Some(String::new()),
            ("```", Some(_)) => samples.push(current.take().unwrap_or_default()),
            (_, Some(buffer)) => {
                buffer.push_str(line);
                buffer.push('\n');
            }
            _ => {}
        }
    }

    assert!(
        !samples.is_empty(),
        "no checkable sample found; if they were all marked `ignore` this test \
         would pass while proving nothing"
    );

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for (index, sample) in samples.iter().enumerate() {
        assert!(
            sample.contains("fn main"),
            "sample {index} has no `fn main`, so it is a fragment and should be \
             marked ```rust,ignore rather than presented as a program"
        );

        let name = format!("_readme_sample_{index}");
        let path = root.join("examples").join(format!("{name}.rs"));
        std::fs::write(&path, sample).expect("write sample");

        let output = std::process::Command::new("cargo")
            .args(["check", "--quiet", "--example", &name])
            .current_dir(root)
            .output();
        let _ = std::fs::remove_file(&path);

        let output = output.expect("run cargo");
        assert!(
            output.status.success(),
            "README sample {index} does not compile:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// A doctest may not be silenced without saying why.
///
/// All three ignored doctests in this crate were hiding broken code. The two
/// `Widget` trait examples called `Painter::draw_text`, which has never
/// existed, and passed a two-argument `fill_rect` that takes three. The web
/// backend's usage example imported and called `WebRunner`, a type nobody ever
/// wrote — the module is a painter with no runner in it at all.
///
/// `ignore` compiles nothing and checks nothing, so an example wearing it is
/// prose that looks like code. `no_run` compiles without running, and
/// `compile_fail` asserts the failure, and either is a claim about the code. A
/// bare `ignore` needs a reason on the line above it.
#[test]
fn no_doctest_is_silently_ignored() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders = Vec::new();
    let mut stack = vec![root];

    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("read dir").flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            let lines: Vec<&str> = text.lines().collect();
            for (number, line) in lines.iter().enumerate() {
                let trimmed = line.trim_start();
                let fence = trimmed
                    .strip_prefix("///")
                    .or_else(|| trimmed.strip_prefix("//!"))
                    .map(str::trim_start);
                let Some(fence) = fence else { continue };
                if !fence.starts_with("```") || !fence.contains("ignore") {
                    continue;
                }
                // A reason on the line above is the whole exception.
                let reason = number
                    .checked_sub(1)
                    .and_then(|i| lines.get(i))
                    .map(|l| l.to_lowercase())
                    .is_some_and(|l| l.contains("ignore:") || l.contains("cannot compile"));
                if !reason {
                    offenders.push(format!(
                        "{}:{}",
                        path.file_name().unwrap_or_default().to_string_lossy(),
                        number + 1
                    ));
                }
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "these doctests are ignored without a stated reason, so nothing checks \
         them and nothing says why: {offenders:?}. Write the example so it \
         compiles, or put `ignore: <reason>` on the line above the fence"
    );
}
