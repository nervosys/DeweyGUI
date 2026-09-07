//! A click lands somewhere, and the widget under it has to be told where.
//!
//! Every widget whose action takes a parameter registers a handler that reads
//! that parameter out of a JSON object. A physical click has no JSON object,
//! so the host passes `null` — and each of those handlers has an
//! `unwrap_or(...)` behind it. The result is not "nothing happened": it is a
//! slider jumping to zero, a tab strip selecting the first tab, and a list
//! selecting row 0, whichever one was clicked.
//!
//! These run against the headless driver because it is the host that can be
//! driven in a test. The click path is shared, so the two that open a window
//! answer the same way — `tests/backend_parity.rs` holds them to it.

use dewey::agent::protocol::{AgentRequest, InjectedEvent};
use dewey::prelude::*;
use dewey::widget::{Slider, SliderState, StatefulWidget, TabState, Tabs};

struct Panel {
    volume: f64,
    tab: usize,
    slider: std::cell::RefCell<SliderState>,
    tabs: std::cell::RefCell<TabState>,
}

impl Model for Panel {
    type Msg = ();

    fn update(&mut self, _m: ()) -> Command<()> {
        Command::None
    }

    fn view(&self, frame: &mut Frame<'_>) {
        // A 100-wide slider over 0..100, so a click at x + 75 means 75 and
        // nothing else. Laid out at a known origin so the test can aim.
        Slider::new(0.0, 100.0)
            .on_change("volume", |p: &mut Panel, v| p.volume = v)
            .render(
                Rect::new(0.0, 0.0, 100.0, 20.0),
                frame,
                &mut self.slider.borrow_mut(),
            );
        Tabs::new(vec!["one".into(), "two".into(), "three".into()])
            .on_select("tabs", |p: &mut Panel, i| p.tab = i)
            .render(
                Rect::new(0.0, 40.0, 300.0, 30.0),
                frame,
                &mut self.tabs.borrow_mut(),
            );
    }
}

fn driver() -> dewey::agent::driver::HeadlessDriver<Panel> {
    let mut d = dewey::agent::driver::HeadlessDriver::new(
        Panel {
            volume: 60.0,
            tab: 2,
            slider: std::cell::RefCell::new(SliderState { value: 60.0 }),
            tabs: std::cell::RefCell::new(TabState { selected: 2 }),
        },
        400.0,
        200.0,
    );
    d.init();
    d.process_request(&AgentRequest::GetTree {
        since: None,
        viewport: None,
    });
    d
}

fn click(d: &mut dewey::agent::driver::HeadlessDriver<Panel>, x: f32, y: f32) {
    let response = d.process_request(&AgentRequest::InjectEvent {
        event: InjectedEvent::MouseClick {
            x,
            y,
            button: "left".into(),
        },
    });
    assert!(response.success, "{:?}", response.error);
}

/// Clicking three quarters of the way along a 0..100 slider means 75.
#[test]
fn a_click_on_a_slider_sets_the_value_it_landed_on() {
    let mut d = driver();
    click(&mut d, 75.0, 10.0);
    let volume = d.model().volume;
    assert!(
        (volume - 75.0).abs() < 1.0,
        "clicking at 75% of a 0..100 slider set it to {volume}"
    );
}

/// Clicking the third tab selects the third tab.
#[test]
fn a_click_on_a_tab_selects_the_tab_it_landed_on() {
    let mut d = driver();
    // A tab is as wide as its label plus 16. The test backend measures text at
    // 0.6 x the font size per character, so at 14 pt "one" and "two" are 41.2
    // wide and "three" is 58: the second tab spans x 41.2..82.4, and the strip
    // is 28 tall from y 40. Nothing here is a round number, which is the point
    // — the widths are a `measure_text` result and are not known until the
    // frame runs.
    click(&mut d, 60.0, 50.0);
    assert_eq!(
        d.model().tab,
        1,
        "clicking the second of three tabs selected another one"
    );
}

/// A click inside the widget and past the last tab selects nothing.
///
/// This is the case that makes the rest safe. The obvious implementation
/// clamps to a valid index, and clamping is how a click on the empty space
/// after the tabs comes to select a tab nobody aimed at.
#[test]
fn a_click_past_the_last_tab_changes_nothing() {
    let mut d = driver();
    // Well past "three", which ends around x 140, and still inside the 300
    // wide strip the widget registered as its hitbox.
    click(&mut d, 250.0, 50.0);
    assert_eq!(
        d.model().tab,
        2,
        "clicking the empty space after the last tab moved the selection"
    );
}

/// A click that lands on a widget must never quietly move it somewhere the
/// click did not indicate. This is the failure the two tests above are
/// specific cases of, and it is the one that matters: a wrong answer is worse
/// than no answer, because nothing reports it.
#[test]
fn a_click_never_invents_a_value() {
    let mut d = driver();
    // The far right of the slider. Whatever the framework does with this, 0.0
    // is the one answer that cannot be right.
    click(&mut d, 99.0, 10.0);
    let volume = d.model().volume;
    assert!(
        volume != 0.0,
        "a click at the right-hand end of a 0..100 slider set it to 0"
    );
}

/// Every clickable widget with a valued handler says what a click on it means.
///
/// This is the check, and the three tests above are examples of what it
/// catches. A widget that registers a hitbox can be clicked; a widget that
/// registers a `ValueMutation` has an action taking parameters; and a click
/// carries no parameters. Fourteen widgets were in that position and every one
/// of them silently used its handler's `unwrap_or` default — a slider set to
/// 0, a tab strip on the first tab, a toolbar firing an item id of "".
///
/// So the pairing is required rather than encouraged. A source-level check,
/// because a widget that forgets is wrong at a keystroke a test would have to
/// know to aim.
#[test]
fn a_clickable_widget_with_a_valued_handler_declares_what_a_click_means() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/widget");
    let mut checked = 0;
    for entry in std::fs::read_dir(&dir).expect("src/widget") {
        let path = entry.expect("entry").path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("read");
        // A widget that cannot be clicked cannot be clicked wrongly, and one
        // whose handler takes no value has nothing to get wrong.
        if !text.contains("register_hitbox") || !text.contains("ValueMutation") {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        assert!(
            text.contains("register_click("),
            "{name} can be clicked and its action takes parameters, and it \
             does not say what a click supplies. Pick `FromPosition`, \
             `Ignored` or `Unavailable`; the default is to pass null, which \
             every one of these handlers reads as its `unwrap_or` value"
        );
        checked += 1;
    }
    assert!(
        checked >= 12,
        "only {checked} widgets matched; the check has stopped finding them"
    );
}

// -- a widget only an agent could operate ------------------------------

/// `Tree` advertised `expand` and `collapse` and registered no hitbox, so no
/// click reached it and — because the focus ring is built from the hit map —
/// neither did Tab. An agent could expand a node and a person could not,
/// which is the house defect wearing its opposite face: not a control that
/// does nothing, but a control only one kind of user can reach.
mod tree {
    use super::*;
    use dewey::widget::tree::{Tree, TreeChange, TreeNode};

    struct Explorer {
        root: std::cell::RefCell<TreeNode>,
    }

    impl Model for Explorer {
        type Msg = ();

        fn update(&mut self, _m: ()) -> Command<()> {
            Command::None
        }

        fn view(&self, frame: &mut Frame<'_>) {
            let root = self.root.borrow().clone();
            Tree::new(root)
                .on_change("files", |e: &mut Explorer, change| {
                    let mut root = e.root.borrow_mut();
                    match change {
                        TreeChange::Expand(path) => {
                            if let Some(n) = root.find_by_path_mut(path) {
                                n.expanded = true;
                            }
                        }
                        TreeChange::Collapse(path) => {
                            if let Some(n) = root.find_by_path_mut(path) {
                                n.expanded = false;
                            }
                        }
                        TreeChange::ExpandAll | TreeChange::CollapseAll => {}
                    };
                })
                .render(Rect::new(0.0, 0.0, 200.0, 200.0), frame);
        }
    }

    fn explorer() -> dewey::agent::driver::HeadlessDriver<Explorer> {
        let root = TreeNode::branch(
            "src",
            vec![
                TreeNode::branch("widget", vec![TreeNode::leaf("button.rs")]),
                TreeNode::leaf("lib.rs"),
            ],
        );
        let mut d = dewey::agent::driver::HeadlessDriver::new(
            Explorer {
                root: std::cell::RefCell::new(root),
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

    fn click_row(d: &mut dewey::agent::driver::HeadlessDriver<Explorer>, row: usize) {
        // Rows are 20 tall from 4 below the top; the middle of one is
        // 4 + 20 * row + 10.
        let y = 4.0 + 20.0 * row as f32 + 10.0;
        let response = d.process_request(&AgentRequest::InjectEvent {
            event: InjectedEvent::MouseClick {
                x: 20.0,
                y,
                button: "left".into(),
            },
        });
        assert!(response.success, "{:?}", response.error);
    }

    fn expanded(d: &dewey::agent::driver::HeadlessDriver<Explorer>, path: &str) -> bool {
        d.model()
            .root
            .borrow()
            .find_by_path(path)
            .expect("a node")
            .expanded
    }

    #[test]
    fn a_person_can_collapse_a_branch_by_clicking_it() {
        let mut d = explorer();
        // `TreeNode::branch` opens a branch, so this one starts expanded and
        // the first thing a click can do to it is close it.
        assert!(expanded(&d, "src"));
        click_row(&mut d, 0);
        assert!(
            !expanded(&d, "src"),
            "clicking an expanded branch did not collapse it"
        );
    }

    /// The half a fixed action cannot do. A click fires the first handler a
    /// widget registered, and `Tree` registers `expand` first — so a tree a
    /// click could collapse could never be reopened by one.
    #[test]
    fn clicking_a_collapsed_branch_expands_it_again() {
        let mut d = explorer();
        click_row(&mut d, 0);
        assert!(!expanded(&d, "src"));
        click_row(&mut d, 0);
        assert!(
            expanded(&d, "src"),
            "a click closed a branch and could not open it again"
        );
    }

    /// A leaf has nothing to expand, and the space below the last row is not
    /// a row. Both must do nothing rather than act on the nearest branch.
    #[test]
    fn clicking_a_leaf_or_empty_space_does_nothing() {
        let mut d = explorer();
        // Rows as drawn, everything open: src, widget, button.rs, lib.rs.
        click_row(&mut d, 3); // lib.rs, a leaf
        click_row(&mut d, 9); // below everything
        assert!(
            expanded(&d, "src") && expanded(&d, "src/widget"),
            "clicking a leaf and then empty space closed a branch neither was"
        );
    }
}

/// A widget an agent can operate and a person cannot is a widget half-built.
///
/// `Tree` advertised `expand` and `collapse`, registered handlers for both,
/// and registered no hitbox — so no click could reach it, and because the
/// focus ring is built from the hit map, neither could Tab. It was operable
/// through `execute_action` and by nothing else.
///
/// Four widgets are still in that position and are named here with the reason,
/// which is the bargain the rest of this repository strikes: wire it, or say
/// in the open that you have not. A widget that appears in neither list fails.
#[test]
fn a_widget_with_a_handler_is_reachable_by_pointer_or_says_why_not() {
    // Actions that are not pointer gestures. A click is a point; none of these
    // is a question a point answers.
    const NOT_A_CLICK: [(&str, &str); 4] = [
        (
            "chart.rs",
            "add_series/remove_series/clear are data operations",
        ),
        ("rich_text.rs", "set_markdown/clear replace content"),
        ("scroll.rs", "scroll_to is a wheel or a drag, not a click"),
        (
            "menu.rs",
            "select_item names an item, and this widget paints only a title              bar — there are no items on screen to click. The gap is the              painting, not the wiring, and ROADMAP.md says so",
        ),
    ];

    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/widget");
    let mut checked = 0;
    for entry in std::fs::read_dir(&dir).expect("src/widget") {
        let path = entry.expect("entry").path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("read");
        if !text.contains("register_message") {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        // A barrier is a hit-map registration too: it is what a click on a
        // modal backdrop lands on.
        let reachable = text.contains("register_hitbox") || text.contains("register_barrier");
        if let Some((_, why)) = NOT_A_CLICK.iter().find(|(f, _)| *f == name) {
            assert!(
                !reachable,
                "{name} is listed as unreachable by pointer because {why}, and                  it registers a hitbox. Take it off the list"
            );
            continue;
        }
        assert!(
            reachable,
            "{name} registers a handler and no hitbox, so an agent can operate              it and a person cannot — no click reaches it, and the focus ring              is built from the hit map, so Tab does not either. Register one,              or add it to NOT_A_CLICK with the reason"
        );
        checked += 1;
    }
    assert!(
        checked >= 14,
        "only {checked} widgets matched; the check has stopped finding them"
    );
}

// -- a dialog you cannot press the button in ---------------------------

/// A widget inside a modal must be clickable.
///
/// The backdrop registers a barrier so a click cannot fall through to what the
/// dialog covers, which is right. It registered it at `u32::MAX`, and
/// `hit_test` takes the highest z-order among everything from the barrier
/// onward — so the backdrop outranked every widget the dialog itself drew, and
/// nothing can register above `u32::MAX`. The dimming worked, the blocking
/// worked, and the OK button could not be pressed.
mod modal {
    use super::*;
    use dewey::widget::Modal;

    struct Dialog {
        open: bool,
        confirmed: bool,
    }

    impl Model for Dialog {
        type Msg = ();

        fn update(&mut self, _m: ()) -> Command<()> {
            Command::None
        }

        fn view(&self, frame: &mut Frame<'_>) {
            Button::new("behind")
                .on("behind", |d: &mut Dialog| d.confirmed = true)
                .render(Rect::new(0.0, 0.0, 200.0, 40.0), frame);
            Modal::new("Confirm", self.open)
                .on_change("dialog", |d: &mut Dialog, open| d.open = open)
                .render(frame.area, frame);
            if self.open {
                // The dialog's own control, drawn after the backdrop the way
                // an application puts content into a modal.
                Button::new("OK")
                    .on("ok", |d: &mut Dialog| d.confirmed = true)
                    .render(Rect::new(120.0, 120.0, 80.0, 30.0), frame);
            }
        }
    }

    fn dialog(open: bool) -> dewey::agent::driver::HeadlessDriver<Dialog> {
        let mut d = dewey::agent::driver::HeadlessDriver::new(
            Dialog {
                open,
                confirmed: false,
            },
            400.0,
            300.0,
        );
        d.init();
        d.process_request(&AgentRequest::GetTree {
            since: None,
            viewport: None,
        });
        d
    }

    fn click_at(d: &mut dewey::agent::driver::HeadlessDriver<Dialog>, x: f32, y: f32) {
        d.process_request(&AgentRequest::InjectEvent {
            event: InjectedEvent::MouseClick {
                x,
                y,
                button: "left".into(),
            },
        });
    }

    #[test]
    fn a_button_inside_an_open_modal_can_be_pressed() {
        let mut d = dialog(true);
        click_at(&mut d, 160.0, 135.0);
        assert!(
            d.model().confirmed,
            "the dialog's own button did not answer a click that landed on it"
        );
    }

    /// The blocking the barrier is for must survive the fix.
    #[test]
    fn a_click_on_the_backdrop_does_not_reach_what_it_covers() {
        let mut d = dialog(true);
        // Over the button behind the dialog, on dimmed backdrop.
        click_at(&mut d, 20.0, 20.0);
        assert!(
            !d.model().confirmed,
            "a click went through the backdrop and pressed the widget beneath"
        );
    }
}
