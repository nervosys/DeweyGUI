//! Agent Demo — a window an agent drives while you watch.
//!
//! Run with: `cargo run --example agent_demo`, then send JSON Lines on stdin:
//!
//! ```json
//! {"id": "1", "request": {"type": "query_ontology"}}
//! {"id": "2", "request": {"type": "get_tree"}}
//! {"id": "3", "request": {"type": "execute_action", "agent_id": "increment_btn", "action": "click"}}
//! ```
//!
//! The third one increments the counter you are looking at, on the next frame.
//!
//! This example used to log "Pipe JSON Lines to stdin for agent control" and
//! read nothing at all, and the three requests it printed were in a format the
//! protocol has never accepted. Both are fixed, and the second could not have
//! been until `Program::with_agent` existed: until then a Dewey application was
//! agent-driven or windowed and never both.

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

        // `action` rather than `agent_id`: the id makes the button visible to
        // an agent, and the action is what makes clicking it — by hand or over
        // the protocol — do anything. The example had only the id, so the
        // request in the header above would have been answered and ignored.
        Button::new("Increment")
            .action("increment_btn", Msg::Increment)
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
    log::info!("Send JSON Lines requests on stdin; the window answers them");

    Program::new(App { count: 0 })
        .with_agent()
        .with_options(ProgramOptions {
            width: 400.0,
            height: 200.0,
            ..Default::default()
        })
        .run()
}
