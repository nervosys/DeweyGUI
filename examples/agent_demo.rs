//! Agent Demo — an application built to be driven by an agent.
//!
//! Run with: `cargo run --example agent_demo`
//!
//! **This window does not read stdin.** It said it did: it logged "Pipe JSON
//! Lines to stdin for agent control" and nothing anywhere read a line, and the
//! three requests it printed were in a format the protocol has never accepted
//! — externally tagged, with a numeric `id`, naming `GetWidgetState` and
//! `PerformAction`, which do not exist.
//!
//! `Program::run` opens a window and serves no agent endpoint; the protocol is
//! served by `HeadlessDriver`, `RpcTransport` and the MCP server, which own the
//! model themselves. See `examples/agent_headless.rs` for an application under
//! agent control.
//!
//! What this example shows is the other half: a view written so that every
//! widget carries an id and an action, which is what makes the same
//! application drivable when it is run headless.

use dewey::prelude::*;

struct App {
    count: i32,
}

#[derive(Debug)]
enum Msg {
    Increment,
}

impl Model for App {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) -> Command<Msg> {
        match msg {
            Msg::Increment => self.count += 1,
        }
        Command::None
    }

    fn view(&self, frame: &mut Frame<'_>) {
        let area = frame.area;

        let chunks = Layout::new(
            Direction::Vertical,
            [Constraint::Length(40.0), Constraint::Length(40.0)],
        )
        .split(area);

        Label::new(format!("Count: {}", self.count))
            .agent_id("counter_label")
            .render(chunks[0], frame);

        Button::new("Increment")
            .agent_id("increment_btn")
            .render(chunks[1], frame);
    }

    fn handle_event(&self, event: Event) -> Option<Msg> {
        if let Event::Key(KeyEvent {
            code: KeyCode::Char('+'),
            ..
        }) = event
        {
            Some(Msg::Increment)
        } else {
            None
        }
    }

    fn title(&self) -> &str {
        "Dewey — Agent Demo"
    }
}

fn main() -> std::result::Result<(), eframe::Error> {
    env_logger::init();
    log::info!("Starting Dewey Agent Demo");
    log::info!("Every widget here is addressable; run it headless to drive it");

    Program::new(App { count: 0 })
        .with_options(ProgramOptions {
            width: 400.0,
            height: 200.0,
            ..Default::default()
        })
        .run()
}
