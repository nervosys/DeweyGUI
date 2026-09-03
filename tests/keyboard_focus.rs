//! Tab moves focus, and Enter presses the focused widget.
//!
//! `FocusManager` shipped as "Focus Management — Ring-buffer tab navigation"
//! and nothing drove it: no host registered a widget in the ring, nothing
//! routed a Tab key, and no widget drew an indicator, so pressing Tab in a
//! Dewey application did nothing at all. An agent never noticed, because it
//! addresses a widget by id; a keyboard user noticed immediately.
//!
//! These run against the headless driver, which is the host that can be
//! driven in a test. The behaviour is shared code, so the backends get the
//! same answers — `tests/backend_parity.rs` holds them to calling it.

use dewey::agent::protocol::{AgentRequest, InjectedEvent};
use dewey::backend::test::RenderOp;
use dewey::prelude::*;

struct Form {
    count: i32,
    saved: bool,
}

impl Model for Form {
    type Msg = ();

    fn update(&mut self, _m: ()) -> Command<()> {
        Command::None
    }

    fn view(&self, frame: &mut Frame<'_>) {
        let rows = frame.area.rows_of(&[40.0, 40.0, 40.0]);
        Label::new(format!("Count: {}", self.count))
            .agent_id("count")
            .render(rows[0], frame);
        Button::new("+")
            .on("inc", |f: &mut Form| f.count += 1)
            .render(rows[1], frame);
        Button::new("Save")
            .on("save", |f: &mut Form| f.saved = true)
            .render(rows[2], frame);
    }
}

fn driver() -> dewey::agent::driver::HeadlessDriver<Form> {
    let mut d = dewey::agent::driver::HeadlessDriver::new(
        Form {
            count: 0,
            saved: false,
        },
        200.0,
        200.0,
    );
    d.init();
    // The ring is built from what a render put in the hit map, so nothing is
    // focusable until something has been drawn.
    d.process_request(&AgentRequest::GetTree {
        since: None,
        viewport: None,
    });
    d
}

fn key(code: &str, modifiers: &[&str]) -> AgentRequest {
    AgentRequest::InjectEvent {
        event: InjectedEvent::Key {
            code: code.to_string(),
            modifiers: modifiers.iter().map(|m| (*m).to_string()).collect(),
        },
    }
}

/// Tab walks the widgets in the order they rendered, and wraps.
#[test]
fn tab_walks_the_ring_in_render_order() {
    let mut d = driver();
    assert_eq!(
        d.focused_id(),
        None,
        "nothing is focused before the first Tab"
    );

    // A `Label` is not interactive and registers no hitbox, so it is not a
    // stop: tabbing through static text is what makes keyboard navigation
    // unusable in the interfaces that get it wrong.
    d.process_request(&key("tab", &[]));
    assert_eq!(d.focused_id(), Some("inc"));

    d.process_request(&key("tab", &[]));
    assert_eq!(d.focused_id(), Some("save"));

    d.process_request(&key("tab", &[]));
    assert_eq!(d.focused_id(), Some("inc"), "the ring wraps");
}

/// Shift+Tab goes back, whichever way the host reports it.
#[test]
fn shift_tab_goes_backwards() {
    let mut d = driver();
    d.process_request(&key("tab", &[]));
    assert_eq!(d.focused_id(), Some("inc"));

    // An agent writes `{"code": "tab", "modifiers": ["shift"]}`; winit and
    // egui report a distinct `BackTab`. Both must go back.
    d.process_request(&key("tab", &["shift"]));
    assert_eq!(
        d.focused_id(),
        Some("save"),
        "wrapping backwards from the first"
    );
}

/// Enter presses the focused widget, by the same path a click takes.
#[test]
fn enter_activates_the_focused_widget() {
    let mut d = driver();
    d.process_request(&key("tab", &[]));
    assert_eq!(d.focused_id(), Some("inc"));

    d.process_request(&key("enter", &[]));
    assert_eq!(
        d.model().count,
        1,
        "Enter on the focused button did not press it"
    );

    d.process_request(&key("tab", &[]));
    d.process_request(&key("space", &[]));
    assert!(d.model().saved, "Space must activate as Enter does");
}

/// Nothing is activated when nothing is focused.
#[test]
fn enter_with_no_focus_does_nothing() {
    let mut d = driver();
    d.process_request(&key("enter", &[]));
    assert_eq!(d.model().count, 0);
    assert!(!d.model().saved);
}

/// Clicking a widget focuses it, so the keyboard carries on from the pointer.
#[test]
fn a_click_moves_focus_to_what_was_clicked() {
    let mut d = driver();
    d.process_request(&AgentRequest::InjectEvent {
        event: InjectedEvent::MouseClick {
            x: 20.0,
            y: 100.0,
            button: "left".into(),
        },
    });
    assert_eq!(d.focused_id(), Some("save"));
    assert!(d.model().saved, "the click also pressed it");

    d.process_request(&key("tab", &[]));
    assert_eq!(d.focused_id(), Some("inc"), "Tab continues from the click");
}

/// The focused widget is drawn with a ring around it.
///
/// Focus that cannot be seen is not focus. The runtime draws the indicator
/// rather than each widget rendering its own focused state: a widget that
/// forgot would be invisibly unreachable, and 29 widgets is 29 chances to
/// forget.
#[test]
fn the_focused_widget_is_drawn_with_a_ring() {
    let mut d = driver();
    d.process_request(&key("tab", &[]));
    // Re-render so the ring is painted with focus set.
    d.process_request(&AgentRequest::GetTree {
        since: None,
        viewport: None,
    });

    let bounds = 40.0..=80.0;
    let ring = d.painted().iter().any(|op| match op {
        RenderOp::StrokeRect { rect, color, .. } => {
            *color == dewey::focus::RING_COLOUR && bounds.contains(&rect.y)
        }
        _ => false,
    });
    assert!(
        ring,
        "nothing drew a focus ring around the focused widget, so a keyboard \
         user cannot see where they are: {:?}",
        d.painted()
    );
}

// ── a modal takes the input it covers ───────────────────────────────────

struct Dialog {
    pressed_behind: bool,
    confirmed: bool,
    open: bool,
}

impl Model for Dialog {
    type Msg = ();

    fn update(&mut self, _m: ()) -> Command<()> {
        Command::None
    }

    fn view(&self, frame: &mut Frame<'_>) {
        let rows = frame.area.rows_of(&[40.0, 40.0]);
        Button::new("Behind")
            .on("behind", |d: &mut Dialog| d.pressed_behind = true)
            .render(rows[0], frame);

        dewey::widget::Modal::new("Confirm", self.open)
            .agent_id("dialog")
            .render(frame.area, frame);

        if self.open {
            Button::new("OK")
                .on("ok", |d: &mut Dialog| d.confirmed = true)
                .render(rows[1], frame);
        }
    }
}

fn dialog(open: bool) -> dewey::agent::driver::HeadlessDriver<Dialog> {
    let mut d = dewey::agent::driver::HeadlessDriver::new(
        Dialog {
            pressed_behind: false,
            confirmed: false,
            open,
        },
        200.0,
        200.0,
    );
    d.init();
    d.process_request(&AgentRequest::GetTree {
        since: None,
        viewport: None,
    });
    d
}

/// A click on the backdrop does not reach the widget underneath.
///
/// The `Modal` dimmed what was behind it and registered nothing, so a click
/// went straight through and pressed the button it was covering. The roadmap
/// called that "input blocking".
#[test]
fn a_modal_takes_the_click_that_lands_on_it() {
    let mut d = dialog(true);
    d.process_request(&AgentRequest::InjectEvent {
        event: InjectedEvent::MouseClick {
            x: 20.0,
            y: 20.0,
            button: "left".into(),
        },
    });
    assert!(
        !d.model().pressed_behind,
        "the click went through the dialog and pressed the button behind it"
    );
}

/// With the dialog closed, the same click reaches the same button.
#[test]
fn a_closed_modal_blocks_nothing() {
    let mut d = dialog(false);
    d.process_request(&AgentRequest::InjectEvent {
        event: InjectedEvent::MouseClick {
            x: 20.0,
            y: 20.0,
            button: "left".into(),
        },
    });
    assert!(
        d.model().pressed_behind,
        "a closed dialog must not block anything, or the test above proves \
         nothing about the dialog"
    );
}

/// Tab does not walk into what the dialog covers.
#[test]
fn focus_does_not_walk_behind_a_modal() {
    let mut d = dialog(true);
    d.process_request(&key("tab", &[]));
    assert_eq!(
        d.focused_id(),
        Some("ok"),
        "Tab reached a widget the dialog is covering"
    );

    d.process_request(&key("tab", &[]));
    assert_eq!(d.focused_id(), Some("ok"), "the ring is the dialog alone");

    d.process_request(&key("enter", &[]));
    assert!(d.model().confirmed);
    assert!(!d.model().pressed_behind);
}
