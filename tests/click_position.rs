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
    const NOT_A_CLICK: [(&str, &str); 2] = [
        (
            "chart.rs",
            "add_series/remove_series/clear are data operations",
        ),
        ("rich_text.rs", "set_markdown/clear replace content"),
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

// -- a dropdown that drops down ----------------------------------------

/// `Select` called itself a dropdown and never drew one.
///
/// It painted the current value and an arrow. The option list was held,
/// described to an agent, and never rendered — so a person could not see the
/// options, could not aim at one, and clicking the arrow had nothing to fire.
/// Drawing it inline would not have worked either: the next field down is
/// rendered after, and would have covered it. It goes through
/// `Frame::overlay`, which draws after every widget has had its turn.
mod select {
    use super::*;
    use dewey::widget::Select;
    use dewey::widget::select::SelectState;

    struct Form {
        colour: usize,
        open: bool,
        state: std::cell::RefCell<SelectState>,
    }

    impl Model for Form {
        type Msg = ();

        fn update(&mut self, _m: ()) -> Command<()> {
            Command::None
        }

        fn view(&self, frame: &mut Frame<'_>) {
            Select::new("Colour", vec!["red".into(), "green".into(), "blue".into()])
                .open(self.open)
                .on_select("colour", |f: &mut Form, i| {
                    f.colour = i;
                    f.state.borrow_mut().selected = i;
                    f.open = false;
                })
                .on_open("colour", |f: &mut Form, open| f.open = open)
                .render(
                    Rect::new(0.0, 0.0, 120.0, 30.0),
                    frame,
                    &mut self.state.borrow_mut(),
                );
            // Rendered after the select, and directly under it: the field the
            // open list covers. Before overlays this button was painted over
            // the dropdown and took its clicks.
            Button::new("underneath")
                .on("underneath", |f: &mut Form| f.colour = 99)
                .render(Rect::new(0.0, 30.0, 120.0, 40.0), frame);
        }
    }

    fn form(open: bool) -> dewey::agent::driver::HeadlessDriver<Form> {
        let mut d = dewey::agent::driver::HeadlessDriver::new(
            Form {
                colour: 0,
                open,
                state: std::cell::RefCell::new(SelectState { selected: 0 }),
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

    fn click_at(d: &mut dewey::agent::driver::HeadlessDriver<Form>, x: f32, y: f32) {
        d.process_request(&AgentRequest::InjectEvent {
            event: InjectedEvent::MouseClick {
                x,
                y,
                button: "left".into(),
            },
        });
    }

    #[test]
    fn clicking_a_closed_select_opens_it() {
        let mut d = form(false);
        click_at(&mut d, 60.0, 15.0);
        assert!(
            d.model().open,
            "clicking the field did not open the option list"
        );
    }

    /// The options are painted. Nothing about a list an agent is told exists
    /// and a person cannot see is a dropdown.
    #[test]
    fn an_open_select_paints_its_options() {
        let d = form(true);
        let drawn: Vec<&str> = d
            .painted()
            .iter()
            .filter_map(|op| match op {
                dewey::backend::test::RenderOp::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        for option in ["red", "green", "blue"] {
            assert!(
                drawn.iter().any(|t| t.contains(option)),
                "`{option}` is an option of an open select and was not drawn: {drawn:?}"
            );
        }
    }

    /// The click lands on the option under the pointer, and on the option
    /// rather than on the control the list is covering.
    #[test]
    fn clicking_an_option_selects_that_option() {
        let mut d = form(true);
        // The list starts below the 30-tall field; rows are 24. The third
        // option spans y 78..102, which is also over the button underneath.
        click_at(&mut d, 60.0, 90.0);
        assert_eq!(
            d.model().colour,
            2,
            "clicking the third option did not select it — 99 means the button              the list covers took the click"
        );
    }

    /// A click below the last option is on the backdrop of nothing.
    #[test]
    fn clicking_past_the_last_option_selects_nothing() {
        let mut d = form(true);
        click_at(&mut d, 60.0, 150.0);
        assert_eq!(
            d.model().colour,
            0,
            "a click past the list changed the selection"
        );
    }
}

// -- a menu bar with no menu -------------------------------------------

/// `Menu` held its items, advertised `select_item` for them, and painted a bar
/// with a title and nothing else. There was nothing on screen to pick, the
/// node it published carried no items either, and its handler was registered
/// inside `if frame.describes(area)` — so on an ordinary frame, which the
/// default backend renders without building a tree, there was no handler at
/// all.
mod menu {
    use super::*;
    use dewey::widget::menu::{Menu, MenuItem};

    struct Editor {
        open: bool,
        chosen: String,
    }

    impl Model for Editor {
        type Msg = ();

        fn update(&mut self, _m: ()) -> Command<()> {
            Command::None
        }

        fn view(&self, frame: &mut Frame<'_>) {
            Menu::new(
                "File",
                vec![
                    MenuItem::new("Open").shortcut("Ctrl+O"),
                    MenuItem::new("Save").shortcut("Ctrl+S"),
                    MenuItem::new("Revert").enabled(false),
                ],
            )
            .open(self.open)
            .on_item("file", |e: &mut Editor, label| e.chosen = label.to_string())
            .on_open("file", |e: &mut Editor, open| e.open = open)
            .render(Rect::new(0.0, 0.0, 300.0, 200.0), frame);
        }
    }

    fn editor(open: bool) -> dewey::agent::driver::HeadlessDriver<Editor> {
        let mut d = dewey::agent::driver::HeadlessDriver::new(
            Editor {
                open,
                chosen: String::new(),
            },
            300.0,
            200.0,
        );
        d.init();
        d.process_request(&AgentRequest::GetTree {
            since: None,
            viewport: None,
        });
        d
    }

    fn click_at(d: &mut dewey::agent::driver::HeadlessDriver<Editor>, x: f32, y: f32) {
        d.process_request(&AgentRequest::InjectEvent {
            event: InjectedEvent::MouseClick {
                x,
                y,
                button: "left".into(),
            },
        });
    }

    #[test]
    fn clicking_the_bar_opens_the_menu() {
        let mut d = editor(false);
        click_at(&mut d, 20.0, 14.0);
        assert!(d.model().open, "clicking the menu bar did not open it");
    }

    #[test]
    fn an_open_menu_paints_its_items() {
        let d = editor(true);
        let drawn: Vec<&str> = d
            .painted()
            .iter()
            .filter_map(|op| match op {
                dewey::backend::test::RenderOp::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        for item in ["Open", "Save", "Revert", "Ctrl+O"] {
            assert!(
                drawn.iter().any(|t| t.contains(item)),
                "`{item}` is in this menu and was not drawn: {drawn:?}"
            );
        }
    }

    #[test]
    fn clicking_an_item_chooses_it() {
        let mut d = editor(true);
        // The bar is 28 tall and items are 24 each, so "Save" spans y 52..76.
        click_at(&mut d, 40.0, 60.0);
        assert_eq!(d.model().chosen, "Save");
    }

    /// A greyed item is drawn as unavailable and must behave that way.
    #[test]
    fn clicking_a_disabled_item_chooses_nothing() {
        let mut d = editor(true);
        click_at(&mut d, 40.0, 84.0);
        assert_eq!(d.model().chosen, "", "a disabled item answered a click");
    }

    /// The items an agent is told about must be the items on screen.
    #[test]
    fn the_tree_describes_the_items() {
        let mut d = editor(true);
        let reply = d.process_request(&AgentRequest::GetState {
            agent_id: "file".into(),
        });
        let text = serde_json::to_string(&reply.data).expect("state");
        for item in ["Open", "Save", "Revert"] {
            assert!(
                text.contains(item),
                "the menu published no `{item}`, so an agent cannot know what                  there is to pick: {text}"
            );
        }
    }
}

/// Wiring must not be conditional on the ontology being built.
///
/// `Menu` registered its `select_item` handler inside
/// `if frame.describes(area)`. That block is for publishing a `UiNode`, and
/// it is skipped on every frame that is not building a tree — which, under
/// `OntologyMode::OnDemand`, is every ordinary frame the default backend
/// draws. So the menu had a handler when an agent was looking and none when a
/// person clicked it.
///
/// The two things look alike in the source and are not: describing a widget
/// is optional, wiring it is not.
#[test]
fn no_widget_registers_its_wiring_only_when_describing_itself() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/widget");
    let mut checked = 0;
    for entry in std::fs::read_dir(&dir).expect("src/widget") {
        let path = entry.expect("entry").path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(&path).expect("read");
        let lines: Vec<&str> = text.lines().collect();
        for (n, line) in lines.iter().enumerate() {
            if !(line.trim_start().starts_with("if") && line.contains("frame.describes(")) {
                continue;
            }
            checked += 1;
            let indent = line.len() - line.trim_start().len();
            for inner in &lines[n + 1..] {
                let trimmed = inner.trim_start();
                let closed = !trimmed.is_empty()
                    && inner.len() - trimmed.len() <= indent
                    && trimmed.starts_with('}');
                if closed {
                    break;
                }
                for call in [
                    "register_message(",
                    "register_hitbox(",
                    "register_click(",
                    "register_barrier(",
                ] {
                    assert!(
                        !inner.contains(call),
                        "{name} calls `{call}` inside `if frame.describes(..)`, \
                         so it is wired only on frames that build the ontology \
                         tree. The default backend builds one when an agent \
                         asks and not otherwise, so this widget answers an \
                         agent and ignores a person"
                    );
                }
            }
        }
    }
    assert!(
        checked >= 15,
        "only {checked} `describes` blocks found; the check has stopped \
         finding them"
    );
}

// -- a tooltip only an agent could read --------------------------------

/// `Tooltip` held its tip text, published it to the ontology, and drew the
/// label alone. Its own doc comment said "visual tooltip popups are handled by
/// the backend"; none was, on any host. So an agent could read a tooltip that
/// no person could ever see — the same asymmetry as `Tree`, in the other
/// direction from the usual one.
///
/// Showing it needed the frame to know where the pointer is. Nothing did: a
/// widget asking whether it was hovered had to be told by the application,
/// which meant the application redoing the arithmetic the hit map already
/// does.
mod tooltip {
    use super::*;
    use dewey::widget::Tooltip;

    struct Help;

    impl Model for Help {
        type Msg = ();

        fn update(&mut self, _m: ()) -> Command<()> {
            Command::None
        }

        fn view(&self, frame: &mut Frame<'_>) {
            Tooltip::new("(?)", "Saves without asking")
                .agent_id("help")
                .render(Rect::new(10.0, 10.0, 40.0, 20.0), frame);
        }
    }

    fn help() -> dewey::agent::driver::HeadlessDriver<Help> {
        let mut d = dewey::agent::driver::HeadlessDriver::new(Help, 200.0, 200.0);
        d.init();
        d
    }

    fn drawn(d: &dewey::agent::driver::HeadlessDriver<Help>) -> Vec<String> {
        d.painted()
            .iter()
            .filter_map(|op| match op {
                dewey::backend::test::RenderOp::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    fn move_pointer(d: &mut dewey::agent::driver::HeadlessDriver<Help>, x: f32, y: f32) {
        d.process_request(&AgentRequest::InjectEvent {
            event: InjectedEvent::MouseMove { x, y },
        });
    }

    fn render(d: &mut dewey::agent::driver::HeadlessDriver<Help>) {
        d.process_request(&AgentRequest::GetTree {
            since: None,
            viewport: None,
        });
    }

    #[test]
    fn the_tip_is_not_drawn_until_the_pointer_is_on_it() {
        let mut d = help();
        render(&mut d);
        let text = drawn(&d);
        assert!(text.iter().any(|t| t == "(?)"), "{text:?}");
        assert!(
            !text.iter().any(|t| t.contains("Saves")),
            "the tip was drawn with the pointer nowhere near it: {text:?}"
        );
    }

    #[test]
    fn hovering_the_label_draws_the_tip() {
        let mut d = help();
        move_pointer(&mut d, 20.0, 15.0);
        render(&mut d);
        let text = drawn(&d);
        assert!(
            text.iter().any(|t| t.contains("Saves without asking")),
            "the pointer is on the label and the tip was not drawn: {text:?}"
        );
    }

    #[test]
    fn moving_away_takes_the_tip_with_it() {
        let mut d = help();
        move_pointer(&mut d, 20.0, 15.0);
        render(&mut d);
        move_pointer(&mut d, 150.0, 150.0);
        render(&mut d);
        assert!(
            !drawn(&d).iter().any(|t| t.contains("Saves")),
            "the tip stayed after the pointer left"
        );
    }

    /// An agent is told whether the tip is on screen, which is a different
    /// fact from whether the widget has one.
    #[test]
    fn the_tree_says_whether_the_tip_is_showing() {
        let mut d = help();
        render(&mut d);
        let closed = d.process_request(&AgentRequest::GetState {
            agent_id: "help".into(),
        });
        assert_eq!(
            closed
                .data
                .as_ref()
                .and_then(|v| v["state"]["showing"].as_bool()),
            Some(false),
            "{:?}",
            closed.data
        );

        move_pointer(&mut d, 20.0, 15.0);
        render(&mut d);
        let open = d.process_request(&AgentRequest::GetState {
            agent_id: "help".into(),
        });
        assert_eq!(
            open.data
                .as_ref()
                .and_then(|v| v["state"]["showing"].as_bool()),
            Some(true),
            "{:?}",
            open.data
        );
    }
}

// -- two more that only an agent could work -----------------------------

/// `DatePicker` painted a calendar and answered no click on it.
///
/// `set_date` takes a year, a month and a day; a click passed none, so the
/// handler read its `unwrap_or`s — 1 January 1970, wherever you clicked. The
/// month arrows were worse: they were drawn inside one string with the month
/// name between them, so their positions depended on the width of the word
/// "September" and could not be hit-tested at all. They are painted as their
/// own runs now, in the cells the click map names.
mod date_picker {
    use super::*;
    use dewey::widget::date_picker::{DateChange, DatePicker, DatePickerState, DateValue};

    struct Booking {
        state: std::cell::RefCell<DatePickerState>,
        picked: Option<(i32, u32, u32)>,
    }

    impl Model for Booking {
        type Msg = ();

        fn update(&mut self, _m: ()) -> Command<()> {
            Command::None
        }

        fn view(&self, frame: &mut Frame<'_>) {
            DatePicker::new()
                .label("When")
                .on_change("when", |b: &mut Booking, change| match change {
                    DateChange::Set { year, month, day } => {
                        b.picked = Some((year, month, day));
                        b.state.borrow_mut().selected = DateValue::new(year, month, day);
                    }
                    DateChange::PrevMonth => {
                        let mut s = b.state.borrow_mut();
                        s.view_month = if s.view_month == 1 {
                            12
                        } else {
                            s.view_month - 1
                        };
                    }
                    DateChange::NextMonth => {
                        let mut s = b.state.borrow_mut();
                        s.view_month = if s.view_month == 12 {
                            1
                        } else {
                            s.view_month + 1
                        };
                    }
                    DateChange::Toggle => {
                        let open = b.state.borrow().open;
                        b.state.borrow_mut().open = !open;
                    }
                })
                .render(
                    Rect::new(0.0, 0.0, 210.0, 220.0),
                    frame,
                    &mut self.state.borrow_mut(),
                );
        }
    }

    /// March 2024 starts on a Friday, so the first row is blank until column 4.
    fn booking(open: bool) -> dewey::agent::driver::HeadlessDriver<Booking> {
        let state = DatePickerState {
            open,
            view_year: 2024,
            view_month: 3,
            selected: DateValue::new(2024, 3, 1),
        };
        let mut d = dewey::agent::driver::HeadlessDriver::new(
            Booking {
                state: std::cell::RefCell::new(state),
                picked: None,
            },
            300.0,
            300.0,
        );
        d.init();
        d.process_request(&AgentRequest::GetTree {
            since: None,
            viewport: None,
        });
        d
    }

    fn click_at(d: &mut dewey::agent::driver::HeadlessDriver<Booking>, x: f32, y: f32) {
        d.process_request(&AgentRequest::InjectEvent {
            event: InjectedEvent::MouseClick {
                x,
                y,
                button: "left".into(),
            },
        });
    }

    #[test]
    fn clicking_the_display_row_opens_the_calendar() {
        let mut d = booking(false);
        click_at(&mut d, 50.0, 14.0);
        assert!(d.model().state.borrow().open);
    }

    /// Cells are 30 wide (210 / 7) and 24 tall; the grid starts 80 below the
    /// top. 1 March 2024 is a Friday, so it is column 4 of the first row.
    #[test]
    fn clicking_a_day_picks_that_day() {
        let mut d = booking(true);
        click_at(&mut d, 4.0 * 30.0 + 15.0, 80.0 + 12.0);
        assert_eq!(
            d.model().picked,
            Some((2024, 3, 1)),
            "clicking the first of the month picked something else"
        );
    }

    /// The blanks before the first of the month are not days.
    #[test]
    fn clicking_a_blank_cell_picks_nothing() {
        let mut d = booking(true);
        click_at(&mut d, 15.0, 80.0 + 12.0);
        assert_eq!(
            d.model().picked,
            None,
            "a blank cell before the first of the month answered a click"
        );
    }

    #[test]
    fn the_arrows_change_the_month() {
        let mut d = booking(true);
        click_at(&mut d, 15.0, 32.0 + 12.0);
        assert_eq!(d.model().state.borrow().view_month, 2, "the left arrow");
        click_at(&mut d, 210.0 - 15.0, 32.0 + 12.0);
        click_at(&mut d, 210.0 - 15.0, 32.0 + 12.0);
        assert_eq!(d.model().state.borrow().view_month, 4, "the right arrow");
    }
}

/// `CommandPalette` covered the window with a hitbox and answered no click.
///
/// `execute` takes a `command_id`; a click supplied none, so the handler read
/// the empty string and ran nothing. The palette is a fuzzy launcher whose
/// entire point is to be clicked.
mod palette {
    use super::*;
    use dewey::widget::command_palette::{
        CommandPalette, CommandPaletteState, PaletteChange, PaletteCommand,
    };

    struct App {
        state: std::cell::RefCell<CommandPaletteState>,
        ran: String,
    }

    impl Model for App {
        type Msg = ();

        fn update(&mut self, _m: ()) -> Command<()> {
            Command::None
        }

        fn view(&self, frame: &mut Frame<'_>) {
            CommandPalette::new(vec![
                PaletteCommand::new("save", "Save file"),
                PaletteCommand::new("quit", "Quit"),
            ])
            .on_change("palette", |a: &mut App, change| match change {
                PaletteChange::Execute(id) => a.ran = id.to_string(),
                PaletteChange::Close => a.state.borrow_mut().open = false,
                PaletteChange::Open => a.state.borrow_mut().open = true,
                PaletteChange::Search(_) => {}
            })
            .render(
                Rect::new(0.0, 0.0, 400.0, 300.0),
                frame,
                &mut self.state.borrow_mut(),
            );
        }
    }

    fn app() -> dewey::agent::driver::HeadlessDriver<App> {
        let state = CommandPaletteState {
            open: true,
            ..Default::default()
        };
        let mut d = dewey::agent::driver::HeadlessDriver::new(
            App {
                state: std::cell::RefCell::new(state),
                ran: String::new(),
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

    fn click_at(d: &mut dewey::agent::driver::HeadlessDriver<App>, x: f32, y: f32) {
        d.process_request(&AgentRequest::InjectEvent {
            event: InjectedEvent::MouseClick {
                x,
                y,
                button: "left".into(),
            },
        });
    }

    /// The window is 400x300, so the palette is 280 wide at x 60, y 40. Rows
    /// start 56 below the top of the palette — y 96 — and are 24 tall, so the
    /// second one is y 120..144.
    #[test]
    fn clicking_a_result_runs_it() {
        let mut d = app();
        click_at(&mut d, 200.0, 132.0);
        assert_eq!(d.model().ran, "quit");
    }

    /// The title and the query line are inside the palette and are not results.
    #[test]
    fn clicking_the_query_line_runs_nothing() {
        let mut d = app();
        click_at(&mut d, 200.0, 75.0);
        assert_eq!(d.model().ran, "");
        assert!(
            d.model().state.borrow().open,
            "clicking inside the palette closed it"
        );
    }

    #[test]
    fn clicking_outside_the_palette_closes_it() {
        let mut d = app();
        click_at(&mut d, 10.0, 290.0);
        assert!(
            !d.model().state.borrow().open,
            "a click on the dimmed area did not close the palette"
        );
        assert_eq!(d.model().ran, "", "and it must not have run anything");
    }
}

// -- the wheel reached nothing -----------------------------------------

/// A wheel turn over a scrollable region reached no widget at all.
///
/// `Scroll` and `VirtualList` both advertise `scroll_to`, neither registered a
/// hitbox, and no host hit-tested a scroll — so the wheel became an
/// `Event::Mouse` the application had to catch and turn into coordinates
/// itself. That is the arithmetic hit-testing exists to do, and it is the
/// state the click path was in before this session started on it.
mod wheel {
    use super::*;
    use dewey::widget::scroll::{ScrollArea, ScrollState};
    use dewey::widget::virtual_list::{VirtualList, VirtualListState};

    struct Page {
        scroll: std::cell::RefCell<ScrollState>,
        list: std::cell::RefCell<VirtualListState>,
        scrolled_to: Option<usize>,
    }

    impl Model for Page {
        type Msg = ();

        fn update(&mut self, _m: ()) -> Command<()> {
            Command::None
        }

        fn view(&self, frame: &mut Frame<'_>) {
            ScrollArea::vertical()
                .on_scroll("region", |p: &mut Page, x, y| {
                    let mut s = p.scroll.borrow_mut();
                    if let Some(x) = x {
                        s.offset_x = x;
                    }
                    if let Some(y) = y {
                        s.offset_y = y;
                    }
                })
                .render(
                    Rect::new(0.0, 0.0, 200.0, 100.0),
                    frame,
                    &mut self.scroll.borrow_mut(),
                );
            VirtualList::new(20.0, |_i, _r, _f| {})
                .on_scroll("rows", |p: &mut Page, index| p.scrolled_to = Some(index))
                .render(
                    Rect::new(0.0, 120.0, 200.0, 80.0),
                    frame,
                    &mut self.list.borrow_mut(),
                );
        }
    }

    fn page() -> dewey::agent::driver::HeadlessDriver<Page> {
        let mut d = dewey::agent::driver::HeadlessDriver::new(
            Page {
                scroll: std::cell::RefCell::new(ScrollState::new()),
                list: std::cell::RefCell::new(VirtualListState {
                    scroll_offset: 200.0,
                    total_items: 500,
                }),
                scrolled_to: None,
            },
            200.0,
            300.0,
        );
        d.init();
        d.process_request(&AgentRequest::GetTree {
            since: None,
            viewport: None,
        });
        d
    }

    fn wheel_at(d: &mut dewey::agent::driver::HeadlessDriver<Page>, x: f32, y: f32, delta_y: f32) {
        let response = d.process_request(&AgentRequest::InjectEvent {
            event: InjectedEvent::MouseScroll {
                x,
                y,
                delta_x: 0.0,
                delta_y,
            },
        });
        assert!(response.success, "{:?}", response.error);
    }

    #[test]
    fn the_wheel_scrolls_the_region_under_it() {
        let mut d = page();
        wheel_at(&mut d, 100.0, 50.0, -3.0);
        assert!(
            d.model().scroll.borrow().offset_y > 0.0,
            "a wheel turn over a scroll region moved nothing"
        );
    }

    /// Up at the top is not a scroll. Reporting one would have the region
    /// answer a turn that changed nothing.
    #[test]
    fn scrolling_up_at_the_top_does_nothing() {
        let mut d = page();
        wheel_at(&mut d, 100.0, 50.0, 3.0);
        assert_eq!(d.model().scroll.borrow().offset_y, 0.0);
    }

    /// The wheel goes to whatever is under it, which is the whole point of
    /// hit-testing it rather than handing it to the application.
    #[test]
    fn the_turn_goes_to_the_widget_under_the_pointer() {
        let mut d = page();
        wheel_at(&mut d, 100.0, 150.0, -1.0);
        assert_eq!(
            d.model().scroll.borrow().offset_y,
            0.0,
            "the turn was over the list and moved the scroll region"
        );
        assert!(
            d.model().scrolled_to.is_some(),
            "the turn was over the list and the list did not move"
        );
    }

    /// A virtual list scrolls in rows, because `scroll_to` takes an index and
    /// only the widget knows how tall a row is.
    #[test]
    fn a_virtual_list_scrolls_by_rows() {
        let mut d = page();
        // Rows are 20 tall and the offset starts at 200, so the first visible
        // row is 10. Three notches down is row 13.
        wheel_at(&mut d, 100.0, 150.0, -3.0);
        assert_eq!(d.model().scrolled_to, Some(13));
    }
}

// -- a drag no host could deliver --------------------------------------

/// `Event::DragDrop` shipped in v1.1 and reached an application on no host.
///
/// The vocabulary was complete — five kinds, four payload types, a source and
/// a target — and nothing produced any of it: agpu converted an
/// `agpu::Event::DragDrop` the agpu crate never constructs, the default
/// backend had no drag path, and the protocol could not inject a release, so
/// `handle_event` was never called with one by anybody.
///
/// The reading lives in `dewey::drag::DragTracker` and every host feeds it.
/// These drive the headless one, which is the host a test can drive.
mod dragdrop {
    use super::*;
    use dewey::event::{DragDropKind, DragPayload, Event};
    use dewey::widget::List;
    use dewey::widget::list::ListState;

    #[derive(Default)]
    struct Board {
        left: std::cell::RefCell<ListState>,
        right: std::cell::RefCell<ListState>,
        seen: std::cell::RefCell<Vec<String>>,
    }

    impl Model for Board {
        type Msg = ();

        fn update(&mut self, _m: ()) -> Command<()> {
            Command::None
        }

        fn handle_event(&self, event: Event) -> Option<()> {
            if let Event::DragDrop(drag) = event {
                let note = match &drag.kind {
                    DragDropKind::DragStart { source_id, .. } => format!("start:{source_id}"),
                    DragDropKind::DragOver { target_id } => format!("over:{target_id}"),
                    DragDropKind::DragLeave { target_id } => format!("leave:{target_id}"),
                    DragDropKind::Drop {
                        source_id,
                        target_id,
                        payload,
                    } => {
                        let index = match payload {
                            DragPayload::Index(i) => *i,
                            _ => usize::MAX,
                        };
                        format!("drop:{source_id}->{target_id}:{index}")
                    }
                    DragDropKind::DragCancel => "cancel".to_string(),
                };
                self.seen.borrow_mut().push(note);
            }
            None
        }

        fn view(&self, frame: &mut Frame<'_>) {
            List::new(vec!["one".into(), "two".into(), "three".into()])
                .draggable(true)
                .on_select("left", |_b: &mut Board, _i| {})
                .render(
                    Rect::new(0.0, 0.0, 100.0, 72.0),
                    frame,
                    &mut self.left.borrow_mut(),
                );
            List::new(vec!["a".into()])
                .on_select("right", |_b: &mut Board, _i| {})
                .render(
                    Rect::new(200.0, 0.0, 100.0, 72.0),
                    frame,
                    &mut self.right.borrow_mut(),
                );
        }
    }

    fn board() -> dewey::agent::driver::HeadlessDriver<Board> {
        let mut d = dewey::agent::driver::HeadlessDriver::new(Board::default(), 400.0, 200.0);
        d.init();
        d.process_request(&AgentRequest::GetTree {
            since: None,
            viewport: None,
        });
        d
    }

    fn press(d: &mut dewey::agent::driver::HeadlessDriver<Board>, x: f32, y: f32) {
        d.process_request(&AgentRequest::InjectEvent {
            event: InjectedEvent::MouseClick {
                x,
                y,
                button: "left".into(),
            },
        });
    }

    fn move_to(d: &mut dewey::agent::driver::HeadlessDriver<Board>, x: f32, y: f32) {
        d.process_request(&AgentRequest::InjectEvent {
            event: InjectedEvent::MouseMove { x, y },
        });
    }

    fn release(d: &mut dewey::agent::driver::HeadlessDriver<Board>, x: f32, y: f32) {
        let response = d.process_request(&AgentRequest::InjectEvent {
            event: InjectedEvent::MouseRelease {
                x,
                y,
                button: "left".into(),
            },
        });
        assert!(response.success, "{:?}", response.error);
    }

    fn seen(d: &dewey::agent::driver::HeadlessDriver<Board>) -> Vec<String> {
        d.model().seen.borrow().clone()
    }

    /// The whole gesture, end to end, on a host that could not deliver any
    /// part of it before.
    #[test]
    fn a_row_can_be_dragged_from_one_list_to_another() {
        let mut d = board();
        press(&mut d, 40.0, 30.0); // the second row: 24..48
        move_to(&mut d, 120.0, 30.0);
        move_to(&mut d, 240.0, 30.0);
        release(&mut d, 240.0, 30.0);

        let notes = seen(&d);
        assert!(
            notes.contains(&"start:left".to_string()),
            "no drag started: {notes:?}"
        );
        assert!(
            notes.contains(&"drop:left->right:1".to_string()),
            "the second row of `left` was not dropped on `right`: {notes:?}"
        );
    }

    /// A press and a release with no movement between them is a click. A
    /// widget that reported both would fire its action and announce a drag for
    /// the same gesture.
    #[test]
    fn a_click_on_a_draggable_row_is_not_a_drag() {
        let mut d = board();
        press(&mut d, 40.0, 30.0);
        release(&mut d, 40.0, 30.0);
        assert!(
            seen(&d).is_empty(),
            "a click reported a drag: {:?}",
            seen(&d)
        );
    }

    /// Letting go over nothing is a cancel, which is a different thing from a
    /// drop and is in the vocabulary for that reason.
    #[test]
    fn releasing_over_empty_space_cancels() {
        let mut d = board();
        press(&mut d, 40.0, 30.0);
        move_to(&mut d, 150.0, 150.0);
        release(&mut d, 150.0, 150.0);
        let notes = seen(&d);
        assert!(notes.contains(&"cancel".to_string()), "{notes:?}");
        assert!(
            !notes.iter().any(|n| n.starts_with("drop:")),
            "a release over nothing was reported as a drop: {notes:?}"
        );
    }

    /// A list that has not asked to be draggable is not.
    #[test]
    fn a_plain_list_offers_nothing_to_drag() {
        let mut d = board();
        press(&mut d, 240.0, 30.0); // the right-hand list, not draggable
        move_to(&mut d, 40.0, 30.0);
        release(&mut d, 40.0, 30.0);
        assert!(seen(&d).is_empty(), "{:?}", seen(&d));
    }

    /// Leaving is announced before arriving, so a target that highlights
    /// itself is never told it has two.
    #[test]
    fn crossing_between_targets_leaves_before_it_arrives() {
        let mut d = board();
        press(&mut d, 40.0, 30.0);
        move_to(&mut d, 40.0, 32.0); // still over `left`
        move_to(&mut d, 240.0, 30.0); // now over `right`
        let notes = seen(&d);
        let leave = notes.iter().position(|n| n == "leave:left");
        let over = notes.iter().position(|n| n == "over:right");
        assert!(
            leave.is_some() && over.is_some() && leave < over,
            "{notes:?}"
        );
    }
}

// -- a picker that had nothing to pick from ----------------------------

/// `ColorPicker` was a preview calling itself "HSV/hex color selection".
///
/// It painted a swatch of the current colour, the label and the hex value.
/// There was no hue strip and no saturation-value square, so nothing on screen
/// offered a colour to choose and `set_color` was reachable only through
/// `execute_action` — a display an agent could write to and a person could
/// only look at. It is the widget I claimed two rounds ago was not there.
mod picker {
    use super::*;
    use dewey::core::Color;
    use dewey::widget::color_picker::{ColorPicker, ColorPickerState};

    struct Paint {
        state: std::cell::RefCell<ColorPickerState>,
        open: bool,
    }

    impl Model for Paint {
        type Msg = ();

        fn update(&mut self, _m: ()) -> Command<()> {
            Command::None
        }

        fn view(&self, frame: &mut Frame<'_>) {
            ColorPicker::new("Ink")
                .open(self.open)
                .on_color("ink", |p: &mut Paint, change| {
                    let mut s = p.state.borrow_mut();
                    let c = s.color;
                    s.color = Color::rgba(
                        change.r.map_or(c.r, |v| v as f32 / 255.0),
                        change.g.map_or(c.g, |v| v as f32 / 255.0),
                        change.b.map_or(c.b, |v| v as f32 / 255.0),
                        change.a.map_or(c.a, |v| v as f32 / 255.0),
                    );
                })
                .on_open("ink", |p: &mut Paint, open| p.open = open)
                .render(
                    Rect::new(0.0, 0.0, 200.0, 260.0),
                    frame,
                    &mut self.state.borrow_mut(),
                );
        }
    }

    fn paint(open: bool, colour: Color) -> dewey::agent::driver::HeadlessDriver<Paint> {
        let mut d = dewey::agent::driver::HeadlessDriver::new(
            Paint {
                state: std::cell::RefCell::new(ColorPickerState::new(colour)),
                open,
            },
            300.0,
            300.0,
        );
        d.init();
        d.process_request(&AgentRequest::GetTree {
            since: None,
            viewport: None,
        });
        d
    }

    fn click_at(d: &mut dewey::agent::driver::HeadlessDriver<Paint>, x: f32, y: f32) {
        d.process_request(&AgentRequest::InjectEvent {
            event: InjectedEvent::MouseClick {
                x,
                y,
                button: "left".into(),
            },
        });
    }

    fn colour(d: &dewey::agent::driver::HeadlessDriver<Paint>) -> Color {
        d.model().state.borrow().color
    }

    #[test]
    fn clicking_the_swatch_opens_the_picker() {
        let mut d = paint(false, Color::rgba(1.0, 0.0, 0.0, 1.0));
        click_at(&mut d, 10.0, 10.0);
        assert!(
            d.model().open,
            "clicking the swatch did not open the picker"
        );
    }

    /// The square and the strip are painted. A picker nobody can see offers
    /// nothing to pick, which is the whole finding.
    #[test]
    fn an_open_picker_paints_a_square_and_a_strip() {
        let d = paint(true, Color::rgba(1.0, 0.0, 0.0, 1.0));
        let fills = d
            .painted()
            .iter()
            .filter(|op| matches!(op, dewey::backend::test::RenderOp::FillRect { .. }))
            .count();
        assert!(
            fills > 200,
            "an open picker drew {fills} filled rectangles; the square alone is \
             16x16 of them"
        );
    }

    /// The top-right corner of the square is full saturation and full value:
    /// the pure hue. Red in, red out.
    #[test]
    fn clicking_the_square_picks_a_saturation_and_a_value() {
        let mut d = paint(true, Color::rgba(1.0, 0.0, 0.0, 1.0));
        // The square sits below the swatch (32) and the hex line (20).
        // Bottom-left is saturation 0, value 0: black.
        click_at(&mut d, 1.0, 52.0 + 127.0);
        let c = colour(&d);
        assert!(
            c.r < 0.1 && c.g < 0.1 && c.b < 0.1,
            "the bottom-left of the square is black and gave {c:?}"
        );
    }

    /// The strip moves the hue and keeps the saturation and value. Reading
    /// those back out of the current colour is why `color_to_hsv` exists.
    #[test]
    fn clicking_the_strip_changes_the_hue_and_keeps_the_rest() {
        let mut d = paint(true, Color::rgba(1.0, 0.0, 0.0, 1.0));
        // A third of the way along the strip is around 120 degrees: green.
        click_at(&mut d, 128.0 / 3.0, 52.0 + 128.0 + 6.0 + 8.0);
        let c = colour(&d);
        assert!(
            c.g > c.r && c.g > c.b,
            "a third of the way along the hue strip should be green, got {c:?}"
        );
        assert!((c.g - 1.0).abs() < 0.1, "the value was not kept: {c:?}");
    }

    /// A click inside the widget and on neither control picks nothing.
    #[test]
    fn clicking_the_label_picks_nothing() {
        let mut d = paint(true, Color::rgba(1.0, 0.0, 0.0, 1.0));
        let before = colour(&d);
        click_at(&mut d, 120.0, 10.0);
        assert_eq!(colour(&d).r, before.r);
        assert_eq!(colour(&d).g, before.g);
    }
}

// -- commands that arrived nowhere -------------------------------------

/// `Command::AgentAction` is how a model drives one of its own widgets.
///
/// It was a `log::debug!` on all three hosts. Two were fixed; the third — the
/// headless driver, which is the host an agent actually drives — was not,
/// because the parity check written to catch it listed only the two backends.
///
/// These go through the driver rather than calling `update` directly, because
/// running the returned command is exactly the part that was missing.
mod commands {
    use super::*;

    struct Panel {
        /// Bumped by the button an agent clicks.
        first: i32,
        /// Bumped only by the widget `Command::AgentAction` reaches for.
        second: i32,
    }

    enum Msg {
        /// Fired by the first button: asks the runtime to press the second.
        Relay,
        /// Fired by the second button, and by nothing else.
        Bump,
        /// Asks for a window this host does not have.
        GoFullscreen,
    }

    impl Model for Panel {
        type Msg = Msg;

        fn update(&mut self, msg: Msg) -> Command<Msg> {
            match msg {
                Msg::Relay => {
                    self.first += 1;
                    Command::AgentAction {
                        agent_id: "second".into(),
                        action: "click".into(),
                        params: serde_json::Value::Null,
                    }
                }
                Msg::Bump => {
                    self.second += 1;
                    Command::None
                }
                Msg::GoFullscreen => Command::SetFullscreen(true),
            }
        }

        fn view(&self, frame: &mut Frame<'_>) {
            let rows = frame.area.rows_of(&[30.0, 30.0, 30.0]);
            Button::new("relay")
                .action("first", Msg::Relay)
                .render(rows[0], frame);
            Button::new("bump")
                .action("second", Msg::Bump)
                .render(rows[1], frame);
            Button::new("window")
                .action("window", Msg::GoFullscreen)
                .render(rows[2], frame);
        }
    }

    fn panel() -> dewey::agent::driver::HeadlessDriver<Panel> {
        let mut d = dewey::agent::driver::HeadlessDriver::new(
            Panel {
                first: 0,
                second: 0,
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

    fn press(d: &mut dewey::agent::driver::HeadlessDriver<Panel>, id: &str) {
        let response = d.process_request(&AgentRequest::ExecuteAction {
            agent_id: id.into(),
            action: "click".into(),
            params: serde_json::Value::Null,
        });
        assert!(response.success, "{:?}", response.error);
    }

    /// Pressing the first button returns `Command::AgentAction` naming the
    /// second. If the driver runs it, the second button's message arrives.
    #[test]
    fn an_agent_action_command_reaches_a_widget_on_the_headless_driver() {
        let mut d = panel();
        press(&mut d, "first");
        assert_eq!(d.model().first, 1, "the button itself did not fire");
        assert_eq!(
            d.model().second,
            1,
            "`Command::AgentAction` named `second` and never reached it — which \
             is what a `log::debug!` in that arm looks like from outside"
        );
    }

    /// A window command cannot be carried out on a windowless host, and is
    /// recorded so a test can assert the application asked. The comment over
    /// that arm claimed exactly this, above code that only logged.
    #[test]
    fn a_window_command_is_recorded_rather_than_dropped() {
        let mut d = panel();
        assert!(d.window_requests().is_empty());
        press(&mut d, "window");
        assert_eq!(
            d.window_requests(),
            ["set_fullscreen"],
            "the application asked to go fullscreen and the driver kept no \
             record of it"
        );
    }
}
