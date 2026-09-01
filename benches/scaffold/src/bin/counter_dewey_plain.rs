//! Canonical counter — Dewey, with no agent affordances.
//!
//! Identical on screen to `counter_dewey`, but with the `agent_id` calls and
//! the `execute_action` handler removed. The difference between the two is
//! exactly what an agent-driveable Dewey app costs over a plain one.

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
        Button::new("- Decrement").render(cols[0], frame);
        Button::new("Reset").render(cols[1], frame);
        Button::new("+ Increment").render(cols[2], frame);
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
