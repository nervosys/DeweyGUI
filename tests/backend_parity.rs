//! Both backends must honour the same options and the same commands.
//!
//! Four defects in this project have had one shape: a capability the agpu
//! backend implements and the default egui backend silently does not, or the
//! reverse. `ProgramOptions::fullscreen` produced a fullscreen window under
//! agpu and was discarded under egui. `OntologyMode` was honoured by agpu and
//! ignored by egui. Drag-and-drop was converted by agpu and dropped by egui.
//! And the five window options added to fix the first of those were added to
//! egui alone, which is this file's own finding.
//!
//! A source-level audit, because neither backend can be started headlessly.
//! It compares the names each backend mentions, which is coarse — but every
//! one of the four defects above would have failed it.

use std::path::Path;

fn source(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {relative}: {e}"))
}

/// The field names declared on `ProgramOptions`.
fn option_fields(runtime: &str) -> Vec<String> {
    let start = runtime
        .find("pub struct ProgramOptions {")
        .expect("ProgramOptions");
    let end = runtime[start..].find("\n}").expect("end") + start;
    runtime[start..end]
        .lines()
        .filter_map(|l| l.trim().strip_prefix("pub "))
        // A field is `name: Type,`; the struct's own declaration line also
        // starts with `pub` and would otherwise be read as a field named
        // "struct ProgramOptions {".
        .filter_map(|l| l.split_once(':'))
        .map(|(name, _)| name.trim().to_string())
        .filter(|name| !name.contains(' '))
        .collect()
}

/// The variant names declared on `Command`.
fn command_variants(runtime: &str) -> Vec<String> {
    let start = runtime.find("pub enum Command<Msg> {").expect("Command");
    let end = runtime[start..].find("\n}").expect("end") + start;
    runtime[start..end]
        .lines()
        .map(str::trim)
        .filter(|l| l.chars().next().is_some_and(char::is_uppercase) && !l.starts_with("///"))
        .map(|l| {
            l.trim_end_matches(',')
                .split(['(', ' ', '{'])
                .next()
                .unwrap_or_default()
                .to_string()
        })
        .filter(|l| !l.is_empty())
        .collect()
}

#[test]
fn both_backends_apply_every_window_option() {
    let runtime = source("src/runtime/mod.rs");
    let agpu = source("src/backend/agpu_backend.rs");

    // Options about the window itself. `tick_rate` and `ontology` are about
    // the frame loop and are checked by their own tests.
    let window_options: Vec<String> = option_fields(&runtime)
        .into_iter()
        .filter(|f| !matches!(f.as_str(), "tick_rate" | "ontology"))
        .collect();
    assert!(window_options.len() >= 10, "{window_options:?}");

    let missing_from_agpu: Vec<&String> = window_options
        .iter()
        .filter(|f| !agpu.contains(&format!("options.{f}")))
        .collect();
    assert!(
        missing_from_agpu.is_empty(),
        "the agpu backend ignores: {missing_from_agpu:?} — an option honoured \
         by one backend and dropped by the other is the defect this file exists \
         to catch"
    );

    let missing_from_egui: Vec<&String> = window_options
        .iter()
        .filter(|f| !runtime.contains(&format!("self.options.{f}")))
        .collect();
    assert!(
        missing_from_egui.is_empty(),
        "the egui backend ignores: {missing_from_egui:?}"
    );
}

#[test]
fn both_backends_handle_every_command() {
    let runtime = source("src/runtime/mod.rs");
    let agpu = source("src/backend/agpu_backend.rs");
    let driver = source("src/agent/driver.rs");

    for variant in command_variants(&runtime) {
        let pattern = format!("Command::{variant}");
        for (name, text) in [
            ("egui backend", &runtime),
            ("agpu backend", &agpu),
            ("headless driver", &driver),
        ] {
            assert!(
                text.contains(&pattern),
                "{name} never mentions `{pattern}`; a command it does not match \
                 is one an application can return and watch do nothing"
            );
        }
    }
}
