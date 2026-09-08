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
    const NOT_A_CLICK: [(&str, &str); 3] = [
        (
            "chart.rs",
            "add_series/remove_series/clear are data operations",
        ),
        ("rich_text.rs", "set_markdown/clear replace content"),
        ("scroll.rs", "scroll_to is a wheel or a drag, not a click"),
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
