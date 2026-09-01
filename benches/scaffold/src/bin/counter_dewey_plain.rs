//! Canonical counter — Dewey, with no agent affordances.
//!
//! A Dewey button needs an id to be hit-testable at all, so the buttons here
//! are identical to `counter_dewey`. The difference is only the id on the
//! read-only label, which exists purely so an agent can read the value back.

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
        let rows =
            Layout::vertical([Constraint::Length(40.0), Constraint::Length(40.0)]).split(frame.area);

        Label::new(format!("Count: {}", self.count)).render(rows[0], frame);

        let cols = Layout::horizontal([Constraint::Ratio(1, 3); 3]).split(rows[1]);
        Button::new("- Decrement").action("dec", Msg::Decrement).render(cols[0], frame);
        Button::new("Reset").action("reset", Msg::Reset).render(cols[1], frame);
        Button::new("+ Increment").action("inc", Msg::Increment).render(cols[2], frame);
    }
}

fn main() -> std::result::Result<(), eframe::Error> {
    Program::new(App { count: 0 })
        .with_options(ProgramOptions {
            width: 400.0,
            height: 200.0,
            ..Default::default()
        })
        .run()
}
