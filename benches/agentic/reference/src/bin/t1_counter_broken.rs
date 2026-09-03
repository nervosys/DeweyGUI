//! Deliberately wrong: the button renders and cannot be addressed.
//!
//! `selftest.py` uses this to prove the verifier can tell a working interface
//! from one that merely looks working.

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
        // No id. It draws, it looks right, and nothing can click it —
        // neither an agent nor the runtime's own hit map. The verifier must
        // score this below the reference, or it is not measuring operability.
        Button::new("+").render(rows[2], frame);
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
