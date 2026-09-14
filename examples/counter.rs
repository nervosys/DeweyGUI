//! Counter — interactive Dewey application demonstrating the Elm architecture.
//!
//! Twelve paid runs of `benches/agentic/` read this file in every single one,
//! which makes it the first Dewey code most agents ever see. So it is worth
//! saying plainly what it demonstrates beyond the architecture:
//!
//! - **`.action(id, msg)` gives a widget its name and its behaviour at once.**
//!   A person clicking the button and an agent calling
//!   `execute_action("increment_btn", "click")` take the same path, because
//!   there is only one.
//! - **A widget with no id can be operated by nobody.** It renders, it looks
//!   right, and no click and no agent can reach it.
//! - **An id alone is not enough.** Until recently these three buttons carried
//!   `.agent_id(...)` and no handler, so they hit-tested and did nothing —
//!   the counter worked by keyboard only, and the check that should have
//!   caught it was blind to buttons.
//!
//! Prove any interface is operable, with no window and no display:
//!
//! ```text
//! cargo run --example counter -- --dewey-validate
//! ```
//!
//! `llms.txt` is the short index; `docs/agent-protocol.md` is the reference.

use dewey::prelude::*;

struct App {
    count: i32,
}

#[derive(Debug)]
enum Msg {
    Increment,
    Decrement,
    Reset,
}

impl Model for App {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) -> Command<Msg> {
        match msg {
            Msg::Increment => self.count += 1,
            Msg::Decrement => self.count -= 1,
            Msg::Reset => self.count = 0,
        }
        Command::None
    }

    fn view(&self, frame: &mut Frame<'_>) {
        let area = frame.area;

        // Layout: counter label on top, buttons below
        let chunks = Layout::new(
            Direction::Vertical,
            [Constraint::Length(40.0), Constraint::Length(40.0)],
        )
        .split(area);

        // Counter display
        Label::new(format!("Count: {}", self.count))
            .agent_id("counter_label")
            .render(chunks[0], frame);

        // Buttons in a horizontal row
        let btn_chunks = Layout::new(
            Direction::Horizontal,
            [
                Constraint::Percentage(33.3),
                Constraint::Percentage(33.3),
                Constraint::Percentage(33.3),
            ],
        )
        .split(chunks[1]);

        Button::new("- Decrement")
            .action("decrement_btn", Msg::Decrement)
            .render(btn_chunks[0], frame);

        Button::new("Reset")
            .action("reset_btn", Msg::Reset)
            .render(btn_chunks[1], frame);

        Button::new("+ Increment")
            .action("increment_btn", Msg::Increment)
            .render(btn_chunks[2], frame);
    }

    fn handle_event(&self, event: Event) -> Option<Msg> {
        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Char('+'),
                ..
            }) => Some(Msg::Increment),
            Event::Key(KeyEvent {
                code: KeyCode::Char('-'),
                ..
            }) => Some(Msg::Decrement),
            Event::Key(KeyEvent {
                code: KeyCode::Char('0'),
                ..
            }) => Some(Msg::Reset),
            _ => None,
        }
    }

    fn register_ontology(&self, registry: &mut OntologyRegistry) {
        registry.register_schema(WidgetSchema::new(
            "CounterApp",
            "A counter application",
            SemanticRole::Container,
        ));
    }

    fn title(&self) -> &str {
        "Dewey — Counter"
    }
}

fn main() -> std::result::Result<(), eframe::Error> {
    env_logger::init();
    Program::new(App { count: 0 })
        .with_options(ProgramOptions {
            width: 400.0,
            height: 200.0,
            ..Default::default()
        })
        .run()
}
