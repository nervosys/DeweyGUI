//! A windowed application answers the agent protocol.
//!
//! `Program::run` owns the model on the thread that owns the window, and so
//! does every transport, so an application was agent-driven or windowed and
//! never both — the premise the project is built on held only if you picked
//! one. The transport thread now hands each request to the frame loop.
//!
//! The window cannot be opened in a test, so what is tested is the part that
//! is not the window: a request crossing to another thread, being answered by
//! the driver that owns the model, and coming back. That is the whole of the
//! new machinery; `Program::with_agent` connects it to eframe.

use dewey::agent::driver::HeadlessDriver;
use dewey::agent::protocol::{AgentRequest, RequestEnvelope};
use dewey::agent::rpc::{ChannelSink, RequestSink, answer_job};
use dewey::prelude::*;

struct Counter {
    count: i32,
}

impl Model for Counter {
    type Msg = ();

    fn update(&mut self, _m: ()) -> Command<()> {
        Command::None
    }

    fn view(&self, frame: &mut Frame<'_>) {
        let rows = frame.area.rows_of(&[40.0, 40.0]);
        Label::new(format!("Count: {}", self.count))
            .agent_id("count")
            .render(rows[0], frame);
        Button::new("+")
            .on("inc", |c: &mut Counter| c.count += 1)
            .render(rows[1], frame);
    }
}

/// Stand in for the frame loop: answer everything, then stop.
fn frame_loop(
    jobs: std::sync::mpsc::Receiver<dewey::agent::rpc::AgentJob>,
    mut driver: HeadlessDriver<Counter>,
) -> std::thread::JoinHandle<HeadlessDriver<Counter>> {
    std::thread::spawn(move || {
        while let Ok(job) = jobs.recv() {
            answer_job(job, &mut driver);
        }
        driver
    })
}

fn envelope(id: &str, request: AgentRequest) -> RequestEnvelope {
    RequestEnvelope {
        id: Some(id.to_string()),
        request,
    }
}

/// A request crosses to the frame loop and comes back answered.
#[test]
fn a_request_is_answered_by_the_thread_that_owns_the_model() {
    let (sender, jobs) = std::sync::mpsc::channel();
    let mut driver = HeadlessDriver::new(Counter { count: 0 }, 200.0, 200.0);
    driver.init();
    let loop_handle = frame_loop(jobs, driver);

    let woken = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = woken.clone();
    let mut sink = ChannelSink::new(
        sender,
        Box::new(move || {
            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }),
    );

    let reply = sink.answer(&envelope("1", AgentRequest::Ping));
    let value: serde_json::Value = serde_json::from_str(&reply).expect("a JSON reply");
    assert_eq!(value["success"], serde_json::json!(true), "{reply}");
    assert_eq!(value["id"], serde_json::json!("1"), "{reply}");
    assert!(sink.is_running());

    assert_eq!(
        woken.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "the frame loop was not woken. An idle window runs `update` only when \
         something asks it to, so a request that does not wake it is never \
         answered at all"
    );

    drop(sink);
    let _ = loop_handle.join();
}

/// An action taken over the protocol changes the model the window is showing.
#[test]
fn an_agent_action_reaches_the_model_behind_the_window() {
    let (sender, jobs) = std::sync::mpsc::channel();
    let mut driver = HeadlessDriver::new(Counter { count: 0 }, 200.0, 200.0);
    driver.init();
    let loop_handle = frame_loop(jobs, driver);

    let mut sink = ChannelSink::new(sender, Box::new(|| {}));
    let reply = sink.answer(&envelope(
        "1",
        AgentRequest::ExecuteAction {
            agent_id: "inc".into(),
            action: "click".into(),
            params: serde_json::Value::Null,
        },
    ));
    let value: serde_json::Value = serde_json::from_str(&reply).expect("a JSON reply");
    assert_eq!(value["success"], serde_json::json!(true), "{reply}");

    drop(sink);
    let driver = loop_handle.join().expect("frame loop");
    assert_eq!(
        driver.model().count,
        1,
        "the action was reported successful and the model did not change — the \
         shape this project keeps finding"
    );
}

/// A stopped frame loop is reported, not waited on forever.
#[test]
fn a_gone_window_is_reported_rather_than_hung_on() {
    let (sender, jobs) = std::sync::mpsc::channel::<dewey::agent::rpc::AgentJob>();
    drop(jobs);

    let mut sink = ChannelSink::new(sender, Box::new(|| {}));
    let reply = sink.answer(&envelope("1", AgentRequest::Ping));
    let value: serde_json::Value = serde_json::from_str(&reply).expect("a JSON reply");
    assert_eq!(value["success"], serde_json::json!(false), "{reply}");
    assert!(
        !sink.is_running(),
        "the transport must stop once the application it speaks for is gone"
    );
}
