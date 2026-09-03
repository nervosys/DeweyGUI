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

/// Both backends must be able to produce every event the application can see.
///
/// The egui backend emitted 6 of the 12 `Event` kinds and the agpu backend all
/// 12: no resize, no focus change, and none of the three file events. An
/// application written against one backend and run on the other simply never
/// heard about half of what happened to it.
#[test]
fn both_backends_can_emit_every_event_kind() {
    let events = source("src/event/mod.rs");
    let start = events.find("pub enum Event {").expect("Event");
    let end = events[start..].find("\n}").expect("end") + start;
    let variants: Vec<String> = events[start..end]
        .lines()
        .map(str::trim)
        .filter(|l| l.chars().next().is_some_and(char::is_uppercase))
        .map(|l| {
            l.trim_end_matches(',')
                .split(['(', ' ', '{'])
                .next()
                .unwrap_or_default()
                .to_string()
        })
        .filter(|l| !l.is_empty())
        .collect();
    assert!(variants.len() >= 12, "{variants:?}");

    let runtime = source("src/runtime/mod.rs");
    let agpu = source("src/backend/agpu_backend.rs");

    // `DragDrop` describes one widget being dragged onto another, and it needs
    // to know what is being dragged. The agpu event layer carries a payload
    // registered by the application; egui has no equivalent, so the egui
    // backend cannot synthesise one without the framework inventing a payload
    // it was never told about. A real asymmetry, named here rather than hidden
    // by a check that quietly skips it — and the reason a Dewey application
    // that needs widget-to-widget dragging drives it from `handle_event`.
    let application_driven = ["DragDrop"];

    for (name, text) in [("egui backend", &runtime), ("agpu backend", &agpu)] {
        let missing: Vec<&String> = variants
            .iter()
            .filter(|v| !application_driven.contains(&v.as_str()))
            .filter(|v| !text.contains(&format!("Event::{v}")))
            .collect();
        assert!(
            missing.is_empty(),
            "{name} can never produce: {missing:?} — an application running on \
             it would never hear about those at all"
        );
    }
}

/// Events that come from state must be emitted on change, not every frame.
///
/// egui reports the window size, the focus and the files hovering over the
/// window as state, republished on every frame. Converting each one straight
/// into an event puts sixty of them a second in front of the model while
/// nothing is happening — which is what the first version of the resize and
/// hover conversion did, and is the same fault as the state diff that reported
/// every unchanged widget as changed.
///
/// The signal that this is done right is that the backend remembers the last
/// frame. This asserts the comparison exists rather than trying to run a
/// window: the emission sites must sit next to a stored previous value.
#[test]
fn state_derived_events_are_compared_against_the_last_frame() {
    let runtime = source("src/runtime/mod.rs");

    for (field, event) in [
        ("last_size", "Event::Resize"),
        ("focused", "Event::FocusGained"),
        ("hovering_files", "Event::FileHover"),
    ] {
        assert!(
            runtime.contains(&format!("self.{field}")),
            "`{event}` derives from state egui republishes every frame, so the \
             backend must remember `{field}` from the last one"
        );
    }

    // The converter is stateless by construction — it takes only the context —
    // so nothing derived from state may be emitted there.
    let start = runtime
        .find("fn convert_egui_events")
        .expect("convert_egui_events");
    let end = runtime[start..]
        .find("\n}\n")
        .map_or(runtime.len(), |i| i + start);
    let converter = &runtime[start..end];

    for event in ["Event::Resize", "Event::FocusGained", "Event::FileHover"] {
        assert!(
            !converter.contains(event),
            "`{event}` is emitted from the stateless converter, which cannot \
             know whether anything changed since the last frame"
        );
    }

    // A drop, by contrast, happens once and belongs there.
    assert!(
        converter.contains("Event::FileDrop"),
        "a drop is a single moment and should be converted directly"
    );
}

/// A click must reach a widget's handler on every host that has clicks.
///
/// This is the fifth and worst of the shape this file exists for. The default
/// backend converted a mouse click into an `Event::Mouse`, handed it to
/// `Model::handle_event`, and stopped. It never called `hit_test` and held no
/// `Handlers` at all — so `Button::action`, `Button::on`, `Checkbox::on` and
/// the eleven other widget handlers did nothing under the backend that
/// `Program::run` uses, which is the backend the README's quick start runs on.
///
/// It worked headless, so every test passed, and it worked under `agpu`, which
/// is opt-in and off by default. The one configuration nobody could test
/// automatically was the one everybody ships.
#[test]
fn a_click_reaches_a_handler_on_every_host() {
    for (name, file) in [
        ("the default backend", "src/runtime/mod.rs"),
        ("the agpu backend", "src/backend/agpu_backend.rs"),
        ("the headless driver", "src/agent/driver.rs"),
    ] {
        let text = source(file);
        assert!(
            text.contains("hit_test"),
            "{name} never hit-tests a click, so it cannot know which widget \
             was pressed. Converting the click into an event and handing it to \
             `handle_event` leaves every `Button::action` inert"
        );
        // `handlers.apply_primary(`, not the bare name: `Handlers` is defined
        // in src/runtime/mod.rs, so the bare name is satisfied there by the
        // definition. The first version of this check passed with the default
        // backend's only call site renamed away.
        assert!(
            text.contains("handlers.apply_primary("),
            "{name} does not activate a widget through \
             `Handlers::apply_primary`, which is the one path from a physical \
             click to the action a widget advertises. Three hosts had three \
             copies of it; the copies are what diverge"
        );
    }
}

/// `Command::AgentAction` must reach a widget, not a log line.
///
/// The default backend's arm was a single `log::debug!` — the same line, in
/// the same shape, as the one that made the stdio and WebSocket transports
/// unable to act. A model returning this command to drive one of its own
/// widgets reached the widget under agpu and reached nothing under the backend
/// `Program::run` uses.
///
/// The existing command-parity test passed throughout, because it asks whether
/// each backend *mentions* every variant. Mentioning it is what the log line
/// did.
#[test]
fn agent_action_is_dispatched_and_not_merely_logged() {
    for (name, file) in [
        ("the default backend", "src/runtime/mod.rs"),
        ("the agpu backend", "src/backend/agpu_backend.rs"),
    ] {
        let text = source(file);
        let start = text
            .find("Command::AgentAction {")
            .unwrap_or_else(|| panic!("{name} does not handle Command::AgentAction"));
        // The arm runs until the next one.
        let rest = &text[start + 1..];
        let end = rest.find("\n            Command::").unwrap_or(rest.len());
        // rustfmt breaks a long call across lines, so `handlers.apply(`
        // is not contiguous in the source. Collapse the whitespace first:
        // the first version of this check failed against the fix itself.
        let arm: String = rest[..end].chars().filter(|c| !c.is_whitespace()).collect();

        assert!(
            arm.contains("handlers.apply(") || arm.contains("self.dispatch("),
            "{name} handles `Command::AgentAction` without dispatching it to a \
             handler. A log line is not an action: this arm is where a model \
             drives its own widget, and the same `log::debug!` in the same \
             position is what left both network transports unable to act"
        );
    }
}
