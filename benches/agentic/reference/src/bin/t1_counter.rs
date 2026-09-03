//! Reference solution for t1-counter. The verifier must score this 1.000.

use dewey::prelude::*;

enum Msg {
    Quit,
}

struct App {
    count: i32,
}

impl Model for App {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) -> Command<Msg> {
        match msg {
            Msg::Quit => Command::Quit,
        }
    }

    fn view(&self, frame: &mut Frame<'_>) {
        let rows = frame.area.rows_of(&[40.0, 40.0, 40.0]);
        Label::new("Counter").agent_id("title").render(rows[0], frame);
        Label::new(format!("Count: {}", self.count))
            .agent_id("count")
            .render(rows[1], frame);
        Button::new("+")
            .on("inc", |a: &mut App| a.count += 1)
            .render(rows[2], frame);
    }

    fn handle_event(&self, event: Event) -> Option<Msg> {
        matches!(
            event,
            Event::Key(KeyEvent {
                code: KeyCode::Char('q'),
                ..
            })
        )
        .then_some(Msg::Quit)
    }
}

fn main() -> std::io::Result<()> {
    dewey_agentic_reference::run_contract(App { count: 0 })?;
    Ok(())
}
