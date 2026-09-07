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
