//! Canonical complex app — TodoMVC, Dewey, with no agent affordances.
//!
//! Interactive widgets keep their ids, because a Dewey widget needs one to be
//! hit-testable at all. What is removed is the ids on read-only labels and the
//! `execute_action` handler for the text field — the parts that exist only so
//! an agent can drive and read the app.

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
            Label::new(self.todos[idx].title.clone()).render(cols[1], frame);
            Button::new("x")
                .action(format!("delete_{idx}"), Msg::Delete(idx))
                .render(cols[2], frame);
        }

        let foot =
            Layout::horizontal([Constraint::Fill(1.0), Constraint::Length(140.0)]).split(rows[3]);
        Label::new(format!("{} items left", self.remaining())).render(foot[0], frame);
        Button::new("Clear completed")
            .action("clear_completed", Msg::ClearCompleted)
            .render(foot[1], frame);
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
