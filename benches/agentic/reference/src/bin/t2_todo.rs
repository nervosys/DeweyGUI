//! Reference solution for t2-todo. The verifier must score this 1.000.

use dewey::prelude::*;
use dewey::widget::Checkbox;

enum Msg {
    Quit,
}

struct Item {
    title: String,
    done: bool,
}

struct App {
    items: Vec<Item>,
}

impl App {
    fn remaining(&self) -> usize {
        self.items.iter().filter(|i| !i.done).count()
    }
}

impl Model for App {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) -> Command<Msg> {
        match msg {
            Msg::Quit => Command::Quit,
        }
    }

    fn view(&self, frame: &mut Frame<'_>) {
        let h = frame.area.height;
        let rows = frame.area.rows_of(&[32.0, h - 60.0, 28.0]);

        Button::new("Add")
            .on("add", |a: &mut App| {
                let n = a.items.len() + 1;
                a.items.push(Item {
                    title: format!("item {n}"),
                    done: false,
                });
            })
            .render(rows[0], frame);

        for (i, row) in (0..self.items.len()).zip(rows[1].rows(28.0)) {
            let cols = row.cols_of(&[24.0, row.width - 24.0]);
            Checkbox::new("", self.items[i].done)
                .on(format!("toggle_{i}"), move |a: &mut App| {
                    a.items[i].done = !a.items[i].done;
                })
                .render(cols[0], frame);
            Label::new(self.items[i].title.clone())
                .agent_id(format!("item_{i}"))
                .render(cols[1], frame);
        }

        Label::new(format!("{} left", self.remaining()))
            .agent_id("remaining")
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
    dewey_agentic_reference::run_contract(App { items: Vec::new() })?;
    Ok(())
}
