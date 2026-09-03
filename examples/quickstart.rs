//! The quick start from the README.
//!
//! Checked in as an example so cargo compiles it on every build.
//! `tests/docs_conformance.rs` asserts this file and the README block
//! are the same text, which is cheaper than compiling the block
//! separately and gives the same guarantee.

use dewey::prelude::*;

struct App {
    count: i32,
}

#[derive(Debug)]
enum Msg {
    Increment,
    Decrement,
}

impl Model for App {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) -> Command<Msg> {
        match msg {
            Msg::Increment => self.count += 1,
            Msg::Decrement => self.count -= 1,
        }
        Command::None
    }

    fn view(&self, frame: &mut Frame<'_>) {
        let rows = frame.area.rows_of(&[40.0, 40.0, 40.0]);
        Label::new(format!("Count: {}", self.count))
            .agent_id("count")
            .render(rows[0], frame);
        // `action` names the widget and gives it the message to send, so a
        // person clicking it and an agent calling `execute_action("inc",
        // "click")` take the same path.
        Button::new("+")
            .action("inc", Msg::Increment)
            .render(rows[1], frame);
        Button::new("-")
            .action("dec", Msg::Decrement)
            .render(rows[2], frame);
    }
}

fn main() -> std::result::Result<(), eframe::Error> {
    Program::new(App { count: 0 }).run()
}
