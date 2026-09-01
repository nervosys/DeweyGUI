//! Canonical complex app — TodoMVC, Dewey, agent-driveable.

use dewey::prelude::*;
use dewey::widget::input::TextInputState;
use dewey::widget::{Checkbox, StatefulWidget, TextInput};
use std::cell::RefCell;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Filter {
    All,
    Active,
    Completed,
}

struct Todo {
    title: String,
    done: bool,
}

struct App {
    todos: Vec<Todo>,
    filter: Filter,
    input: RefCell<TextInputState>,
}

#[derive(Debug)]
enum Msg {
    Add,
    Toggle(usize),
    Delete(usize),
    SetFilter(Filter),
    ClearCompleted,
}

impl App {
    fn visible(&self) -> Vec<usize> {
        (0..self.todos.len())
            .filter(|&i| match self.filter {
                Filter::All => true,
                Filter::Active => !self.todos[i].done,
                Filter::Completed => self.todos[i].done,
            })
            .collect()
    }

    fn remaining(&self) -> usize {
        self.todos.iter().filter(|t| !t.done).count()
    }
}

impl Model for App {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) -> Command<Msg> {
        match msg {
            Msg::Add => {
                let title = self.input.borrow().text.trim().to_string();
                if !title.is_empty() {
                    self.todos.push(Todo { title, done: false });
                    *self.input.borrow_mut() = TextInputState::new();
                }
            }
            Msg::Toggle(i) => {
                if let Some(t) = self.todos.get_mut(i) {
                    t.done = !t.done;
                }
            }
            Msg::Delete(i) => {
                if i < self.todos.len() {
                    self.todos.remove(i);
                }
            }
            Msg::SetFilter(f) => self.filter = f,
            Msg::ClearCompleted => self.todos.retain(|t| !t.done),
        }
        Command::None
    }

    fn view(&self, frame: &mut Frame<'_>) {
        let rows = Layout::vertical([
            Constraint::Length(36.0),
            Constraint::Length(32.0),
            Constraint::Fill(1.0),
            Constraint::Length(28.0),
        ])
        .split(frame.area);

        let top =
            Layout::horizontal([Constraint::Fill(1.0), Constraint::Length(80.0)]).split(rows[0]);
        TextInput::new()
            .placeholder("What needs doing?")
            .agent_id("new_todo")
            .render(top[0], frame, &mut self.input.borrow_mut());
        Button::new("Add").action("add", Msg::Add).render(top[1], frame);

        let f = Layout::horizontal([Constraint::Ratio(1, 3); 3]).split(rows[1]);
        Button::new("All").action("filter_all", Msg::SetFilter(Filter::All)).render(f[0], frame);
        Button::new("Active").action("filter_active", Msg::SetFilter(Filter::Active)).render(f[1], frame);
        Button::new("Completed").action("filter_completed", Msg::SetFilter(Filter::Completed)).render(f[2], frame);

        for (idx, row) in self.visible().into_iter().zip(rows[2].rows(28.0)) {
            let cols = Layout::horizontal([
                Constraint::Length(24.0),
                Constraint::Fill(1.0),
                Constraint::Length(28.0),
            ])
            .split(row);
            Checkbox::new("", self.todos[idx].done)
                .action(format!("toggle_{idx}"), Msg::Toggle(idx))
                .render(cols[0], frame);
            Label::new(self.todos[idx].title.clone())
                .agent_id(format!("item_{idx}"))
                .render(cols[1], frame);
            Button::new("x")
                .action(format!("delete_{idx}"), Msg::Delete(idx))
                .render(cols[2], frame);
        }

        let foot =
            Layout::horizontal([Constraint::Fill(1.0), Constraint::Length(140.0)]).split(rows[3]);
        Label::new(format!("{} items left", self.remaining()))
            .agent_id("remaining")
            .render(foot[0], frame);
        Button::new("Clear completed")
            .action("clear_completed", Msg::ClearCompleted)
            .render(foot[1], frame);
    }

    /// Only the text field still needs a handler: a `TextInput` carries state,
    /// not a message.
    fn execute_action(&mut self, id: &str, action: &str, p: &serde_json::Value) -> serde_json::Value {
        if (id, action) == ("new_todo", "set_text") {
            let text = p.get("text").and_then(|v| v.as_str()).unwrap_or("");
            *self.input.borrow_mut() = TextInputState::new().with_text(text);
        }
        serde_json::json!({ "todos": self.todos.len(), "remaining": self.remaining() })
    }
}

fn main() -> std::result::Result<(), eframe::Error> {
    Program::new(App {
        todos: Vec::new(),
        filter: Filter::All,
        input: RefCell::new(TextInputState::new()),
    })
    .with_options(ProgramOptions {
        width: 480.0,
        height: 400.0,
        ..Default::default()
    })
    .run()
}
