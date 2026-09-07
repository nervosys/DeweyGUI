//! `get_performance` answers what the running interface costs, and the numbers
//! come from somewhere.
//!
//! `Profiler` was written, driven by the opt-in agpu backend alone, and read
//! by nothing — the same shape as every other defect in this crate's history.
//! Worse, the half of a frame an application controls was never timed at all:
//! `FrameProfile::update` was filled from a timer no host started, so it read
//! zero everywhere. These tests are about the numbers being real, not about
//! them being any particular size — a benchmark asserts speed, a test asserts
//! that a measurement happened.

use std::time::Duration;

use dewey::agent::protocol::AgentRequest;
use dewey::prelude::*;

struct Slow {
    ticks: u32,
}

enum Msg {
    Work,
}

impl Model for Slow {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) -> Command<Msg> {
        match msg {
            // Long enough that a timer with any resolution at all sees it, and
            // long enough that a `0.0` in the reply means nobody measured.
            Msg::Work => {
                std::thread::sleep(Duration::from_millis(5));
                self.ticks += 1;
            }
        }
        Command::None
    }

    fn view(&self, frame: &mut Frame<'_>) {
        let rows = frame.area.rows_of(&[40.0, 40.0]);
        Label::new(format!("ticks: {}", self.ticks))
            .agent_id("ticks")
            .render(rows[0], frame);
        // `action` sends a message through `Model::update`, which is the half
        // of a frame `update_ms` is about. `on` mutates the model in place and
        // never reaches `update` at all.
        Button::new("work")
            .action("work", Msg::Work)
            .render(rows[1], frame);
    }
}

fn driver() -> dewey::agent::driver::HeadlessDriver<Slow> {
    let mut d = dewey::agent::driver::HeadlessDriver::new(Slow { ticks: 0 }, 200.0, 200.0);
    d.init();
    d
}

fn performance(d: &mut dewey::agent::driver::HeadlessDriver<Slow>) -> serde_json::Value {
    let response = d.process_request(&AgentRequest::GetPerformance);
    assert!(response.success, "{:?}", response.error);
    response.data.expect("a performance answer carries data")
}

/// Before anything has rendered there is nothing to report, and the reply says
/// so rather than reporting zeros that look like a very fast interface.
#[test]
fn nothing_measured_is_reported_as_nothing_measured() {
    let mut d = driver();
    let data = performance(&mut d);
    assert_eq!(data["frames_measured"], 0);
    assert_eq!(data["frames_total"], 0);
    assert!(data["last_frame"].is_null());
}

/// Asking what a frame costs must not render one. An answer that describes the
/// frame the question caused describes the question.
#[test]
fn asking_does_not_itself_draw_a_frame() {
    let mut d = driver();
    d.process_request(&AgentRequest::GetTree {
        since: None,
        viewport: None,
    });
    let after_one = performance(&mut d);
    assert_eq!(after_one["frames_total"], 1);

    for _ in 0..5 {
        performance(&mut d);
    }
    let later = performance(&mut d);
    assert_eq!(
        later["frames_total"], 1,
        "six performance requests rendered five frames between them"
    );
}

/// The render half of a frame is measured, and every widget is counted —
/// including the ones nested inside another.
#[test]
fn a_rendered_frame_is_timed_and_counted() {
    let mut d = driver();
    d.process_request(&AgentRequest::GetTree {
        since: None,
        viewport: None,
    });
    let data = performance(&mut d);
    let last = &data["last_frame"];

    assert!(
        last["render_ms"].as_f64().unwrap() > 0.0,
        "a frame that drew two widgets took no measurable time: {last}"
    );
    assert!(
        last["total_ms"].as_f64().unwrap() >= last["render_ms"].as_f64().unwrap(),
        "the whole frame is shorter than the render inside it: {last}"
    );
    assert_eq!(
        last["widget_count"], 2,
        "the label and the button are what rendered: {last}"
    );
}

/// `Model::update` is the half an application controls and the half nothing
/// timed. An action that sleeps for 5 ms must show up as roughly 5 ms.
#[test]
fn time_spent_in_update_is_attributed_to_the_frame_that_follows_it() {
    let mut d = driver();
    d.process_request(&AgentRequest::GetTree {
        since: None,
        viewport: None,
    });
    // The first frame delivered no messages, so it spent no time updating.
    assert_eq!(performance(&mut d)["last_frame"]["update_ms"], 0.0);

    let response = d.process_request(&AgentRequest::ExecuteAction {
        agent_id: "work".into(),
        action: "click".into(),
        params: serde_json::Value::Null,
    });
    assert!(response.success, "{:?}", response.error);
    // `execute_action` renders before it dispatches, so the update lands in
    // the frame after it — which is the next thing that renders.
    d.process_request(&AgentRequest::GetTree {
        since: None,
        viewport: None,
    });

    let update_ms = performance(&mut d)["last_frame"]["update_ms"]
        .as_f64()
        .unwrap();
    assert!(
        update_ms >= 4.0,
        "an update that slept 5 ms was measured at {update_ms} ms, so nothing \
         timed it"
    );
}

/// Frames per second describes a display loop. Headless there is none, and a
/// number that would really report how often the agent spoke is left out.
#[test]
fn frames_per_second_is_withheld_where_there_is_no_loop() {
    let mut d = driver();
    d.process_request(&AgentRequest::GetTree {
        since: None,
        viewport: None,
    });
    let data = performance(&mut d);
    assert_eq!(data["host"], "headless");
    assert!(
        data.get("avg_fps").is_none(),
        "headless reported frames per second: {data}"
    );
}
