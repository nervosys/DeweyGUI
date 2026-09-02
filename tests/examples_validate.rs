//! The examples must pass the checks the framework advertises.
//!
//! An example is the code people copy. Shipping one that a `validate` call
//! would reject teaches the pattern the check exists to prevent — and the
//! framework's own TodoMVC sample already turned out to be written with
//! positional ids.
//!
//! This is a source-level audit rather than a run of each example: they build
//! windows, and most cannot be constructed headlessly from outside the crate.
//! It catches the two mistakes that are visible in source.

use std::path::Path;

fn examples() -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("examples dir").flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        out.push((name, std::fs::read_to_string(&path).unwrap_or_default()));
    }
    out.sort();
    out
}

/// Strip line comments so prose about a pattern is not mistaken for the pattern.
fn code_only(source: &str) -> String {
    source
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// No example may name a widget by its index.
///
/// `toggle_{i}` follows position: remove the row above it and the same name
/// means a different thing, so an agent acting on what it read a moment ago
/// acts on the wrong widget and is told it succeeded. `validate` reports this
/// as `positional_id`; an example that does it teaches it.
#[test]
fn no_example_names_a_widget_by_its_index() {
    let mut offenders = Vec::new();
    for (name, source) in examples() {
        let code = code_only(&source);
        for line in code.lines() {
            let trimmed = line.trim();
            // `format!("thing_{i}")` as an id, in any of the id-taking calls.
            let takes_id = trimmed.contains(".on(")
                || trimmed.contains(".action(")
                || trimmed.contains(".agent_id(")
                || trimmed.contains("on_select(")
                || trimmed.contains("on_input(")
                || trimmed.contains("on_change(");
            if takes_id && (trimmed.contains("_{i}") || trimmed.contains("_{index}")) {
                offenders.push(format!("{name}: {trimmed}"));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "examples are copied, and these name widgets by position:\n  {}",
        offenders.join("\n  ")
    );
}

/// Every interactive widget in an example must be given an id.
///
/// A `Button` with no id renders correctly and is dead: no hitbox, no tree
/// node, nothing an agent can name. `validate` reports it as
/// `unaddressable_widget`, and the mistake was originally made while writing
/// this project's own benchmarks.
#[test]
fn no_example_leaves_an_interactive_widget_unnamed() {
    let mut offenders = Vec::new();
    for (name, source) in examples() {
        let code = code_only(&source);
        // A builder chain ends at `.render(`; check each one that starts with
        // an interactive widget.
        for chunk in code.split(".render(") {
            // The chain is what follows the last statement break; without
            // this, `registry.register(&Button::new("_"))` on an earlier line
            // is read as part of a later widget's chain.
            let chain = chunk.rsplit_once(';').map_or(chunk, |(_, after)| after);
            let Some(start) = chain.rfind("Button::new") else {
                continue;
            };
            let tail = &chain[start..];
            let named =
                tail.contains(".on(") || tail.contains(".action(") || tail.contains(".agent_id(");
            if !named {
                let snippet: String = tail
                    .split_whitespace()
                    .take(6)
                    .collect::<Vec<_>>()
                    .join(" ");
                offenders.push(format!("{name}: {snippet}"));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "an unnamed Button renders and is dead:\n  {}",
        offenders.join("\n  ")
    );
}
