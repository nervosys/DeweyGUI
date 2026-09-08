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
        // `apply_primary_at(`, not the bare name: `Handlers` is defined in
        // src/runtime/mod.rs, so the bare name is satisfied there by the
        // definition. The first version of this check passed with the default
        // backend's only call site renamed away.
        assert!(
            text.contains("apply_primary_at("),
            "{name} does not activate a widget through \
             `Handlers::apply_primary_at`, which is the one path from a \
             physical click to the action a widget advertises. Three hosts \
             had three copies of it; the copies are what diverge"
        );
        // The positional form, with a position. `apply_primary_at(id, None)`
        // is the keyboard case and compiles just as well, so a host that
        // forgets the click coordinates is back to a slider that zeroes
        // itself when clicked, with the call site still looking right.
        assert!(
            text.contains("Some(m.position)"),
            "{name} activates a widget without telling it where the click \
             landed, so every widget whose action takes a parameter falls \
             back to its handler's `unwrap_or` default"
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

/// Keyboard focus must work on every host, not just the testable one.
///
/// `FocusManager` shipped as a complete feature and no host drove it, so
/// pressing Tab did nothing anywhere. The behaviour now lives in `focus.rs`
/// and every host calls it — which is the only reason the headless tests in
/// `tests/keyboard_focus.rs` say anything about the two that open a window.
#[test]
fn every_host_drives_the_focus_ring() {
    for (name, file) in [
        ("the default backend", "src/runtime/mod.rs"),
        ("the agpu backend", "src/backend/agpu_backend.rs"),
        ("the headless driver", "src/agent/driver.rs"),
    ] {
        let text: String = source(file)
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        for (what, call) in [
            ("route keys to the ring", "focus::handle_key("),
            ("rebuild the ring after a frame", ".rebuild("),
            ("draw the focus indicator", "focus::draw_ring("),
            ("focus what was clicked", ".focus_on(&id)"),
        ] {
            assert!(
                text.contains(call),
                "{name} does not {what}. Focus that works on one host is the \
                 shape that left `Button::action` inert under the backend \
                 everybody ships"
            );
        }
    }
}

/// Every host must measure the frame it drew, and route updates through the
/// one place that times them.
///
/// `Profiler` shipped complete, was driven by the opt-in agpu backend alone,
/// and nothing read it — and even there, `FrameProfile::update` came from a
/// timer no host ever started, so it read zero for the life of the crate. Both
/// halves of that are the pattern this file exists to catch.
#[test]
fn every_host_measures_a_frame() {
    for (name, file, calls) in [
        (
            "the default backend",
            "src/runtime/mod.rs",
            [
                "driver.begin_frame(",
                "driver.end_frame(",
                "driver.update_model(",
            ],
        ),
        (
            "the headless driver",
            "src/agent/driver.rs",
            ["self.begin_frame(", "self.end_frame(", "self.update_model("],
        ),
        (
            "the agpu backend",
            "src/backend/agpu_backend.rs",
            [
                "profiler.begin_frame(",
                "profiler.end_frame(",
                "profiler.start(",
            ],
        ),
    ] {
        let text: String = source(file)
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        for call in calls {
            let wanted: String = call.chars().filter(|c| !c.is_whitespace()).collect();
            assert!(
                text.contains(&wanted),
                "{name} never calls `{call}`, so a frame it drew is not measured; instrumentation one host drives is how the profiler came to report nothing"
            );
        }
    }

    // The update timer is the half that read zero everywhere. It is filled by
    // exactly one function, and every host reaches it by calling that.
    let driver = source("src/agent/driver.rs");
    assert!(
        driver.contains("self.pending_update += started.elapsed()"),
        "`update_model` no longer times `Model::update`, so `update_ms` is a constant zero again"
    );
    assert!(
        !source("src/runtime/mod.rs").contains("model_mut().update("),
        "the default backend calls `Model::update` directly again, going around the only place that times it"
    );
}

/// Every host runs a view through `runtime::render`, not `Model::view`.
///
/// `view` alone leaves whatever the widgets queued with `Frame::overlay`
/// undrawn — an open `Select` simply would not appear, and the field it
/// covers would take its clicks. Three hosts each remembering to run the
/// deferred pass is the arrangement that left the click path, Tab, modal
/// input blocking and the plugin system working on one host and not another.
#[test]
fn every_host_renders_through_one_function() {
    for (name, file) in [
        ("the default backend", "src/runtime/mod.rs"),
        ("the agpu backend", "src/backend/agpu_backend.rs"),
        ("the headless driver", "src/agent/driver.rs"),
    ] {
        let text = source(file);
        assert!(
            text.contains("render(") && !text.contains(".view(&mut frame)"),
            "{name} calls `Model::view` directly instead of going through \
             `runtime::render`, so anything a widget deferred to an overlay \
             is never drawn on it"
        );
    }

    // And the one function does the deferred pass. Without this the check
    // above passes on a `render` that forgot it.
    let runtime = source("src/runtime/mod.rs");
    assert!(
        runtime.contains("frame.run_overlays()"),
        "`runtime::render` no longer runs the overlays it exists to run"
    );
}

/// Every host tells the frame where the pointer is.
///
/// A widget that wants to know whether it is hovered — a `Tooltip` deciding
/// whether to show its tip — asks the frame. A host that does not pass the
/// pointer answers "nowhere", and the tip silently never appears on that host
/// while working on the others. That is the shape of four defects in this
/// file already.
#[test]
fn every_host_says_where_the_pointer_is() {
    // The expression each host passes, not just the call. The first version
    // of this check asked whether `.with_pointer(` appeared anywhere, and
    // passed with the headless driver changed to `.with_pointer(None)` —
    // which is the whole defect, spelled out.
    for (name, file, expected) in [
        (
            "the default backend",
            "src/runtime/mod.rs",
            ".with_pointer(pointer)",
        ),
        (
            "the agpu backend",
            "src/backend/agpu_backend.rs",
            ".with_pointer(self.pointer)",
        ),
        (
            "the headless driver",
            "src/agent/driver.rs",
            ".with_pointer(self.pointer)",
        ),
    ] {
        let text: String = source(file)
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        let wanted: String = expected.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(
            text.contains(&wanted),
            "{name} does not pass `{expected}` to its frame, so every widget \
             on it believes nothing is hovered"
        );
    }

    // And each of the two that receive mouse events keeps track of it. The
    // default backend reads it from egui as state, which is what a hover is.
    for (name, file, marker) in [
        (
            "the agpu backend",
            "src/backend/agpu_backend.rs",
            "app.pointer=Some(m.position)",
        ),
        (
            "the headless driver",
            "src/agent/driver.rs",
            "self.pointer=Some(m.position)",
        ),
    ] {
        let text: String = source(file)
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        assert!(
            text.contains(marker),
            "{name} never records where a mouse event happened, so the \
             pointer it passes is whatever it started as"
        );
    }
    let runtime: String = source("src/runtime/mod.rs")
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    assert!(
        runtime.contains("i.pointer.latest_pos()"),
        "the default backend stopped reading the pointer from egui"
    );
}

/// Every host routes a wheel turn to the widget under it.
///
/// A turn used to become an `Event::Mouse` and stop there: `ScrollArea` and
/// `VirtualList` advertise `scroll_to`, neither registered a hitbox, and no
/// host hit-tested a scroll — so every application caught the event and did
/// the coordinate arithmetic hit-testing exists to do. Exactly where the
/// click path was before this session.
#[test]
fn every_host_routes_a_wheel_turn() {
    for (name, file) in [
        ("the default backend", "src/runtime/mod.rs"),
        ("the agpu backend", "src/backend/agpu_backend.rs"),
        ("the headless driver", "src/agent/driver.rs"),
    ] {
        let text: String = source(file)
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        assert!(
            text.contains("MouseEventKind::Scroll{delta_x,delta_y}"),
            "{name} does not look at a wheel turn at all"
        );
        assert!(
            text.contains("apply_scroll_at(") || text.contains("dispatch_scroll("),
            "{name} sees a wheel turn and never routes it to a widget, so a \
             scrollable region does not scroll under the pointer on it"
        );
    }

    // Routed to whatever is under the pointer, not to a fixed widget.
    for (name, file) in [
        ("the default backend", "src/runtime/mod.rs"),
        ("the agpu backend", "src/backend/agpu_backend.rs"),
        ("the headless driver", "src/agent/driver.rs"),
    ] {
        let text: String = source(file)
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        assert!(
            text.matches("hit_test(m.position)").count() >= 2,
            "{name} hit-tests a click and not a wheel turn, so the turn goes \
             wherever the last click did"
        );
    }
}

/// Every host reads a drag and delivers it.
///
/// `Event::DragDrop` shipped in v1.1 with a complete vocabulary and no host
/// that could produce one: agpu converted an `agpu::Event::DragDrop` the agpu
/// crate never constructs, the default backend had no drag path, and the
/// protocol could not inject a release. The reading is one implementation now,
/// and this is what keeps it that way.
#[test]
fn every_host_reads_a_drag() {
    for (name, file) in [
        ("the default backend", "src/runtime/mod.rs"),
        ("the agpu backend", "src/backend/agpu_backend.rs"),
        ("the headless driver", "src/agent/driver.rs"),
    ] {
        let text: String = source(file)
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        assert!(
            text.contains("drag.handle("),
            "{name} does not read its mouse events as a drag, so \
             `Event::DragDrop` reaches no application on it"
        );
        assert!(
            text.contains("Event::DragDrop(drag)"),
            "{name} reads a drag and delivers nothing, which is the shape the \
             agpu backend was already in: it converted a drag event and no \
             application ever saw one"
        );
    }

    // One reading, not three. The tracker is the only thing that decides
    // whether letting go is a drop or a cancel.
    let drag = source("src/drag.rs");
    assert!(
        drag.contains("DragDropKind::Drop") && drag.contains("DragDropKind::DragCancel"),
        "the tracker no longer decides between a drop and a cancel"
    );
}
