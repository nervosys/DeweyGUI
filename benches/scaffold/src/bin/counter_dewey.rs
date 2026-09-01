//! Canonical counter — Dewey, agent-driveable.

use dewey::prelude::*;

struct App {
    count: i32,
}

impl Model for App {
    type Msg = ();

    fn update(&mut self, _msg: ()) -> Command<()> {
        Command::None
    }

    fn view(&self, frame: &mut Frame<'_>) {
        let rows = frame.area.rows_of(&[40.0, 40.0]);
        Label::new(format!("Count: {}", self.count))
            .agent_id("count")
            .render(rows[0], frame);

        let cols = rows[1].split_columns(3);
        Button::new("- Decrement")
            .on("dec", |a: &mut App| a.count -= 1)
            .render(cols[0], frame);
        Button::new("Reset")
            .on("reset", |a: &mut App| a.count = 0)
            .render(cols[1], frame);
        Button::new("+ Increment")
            .on("inc", |a: &mut App| a.count += 1)
            .render(cols[2], frame);
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
