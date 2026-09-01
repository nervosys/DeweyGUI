//! Canonical complex app — TodoMVC, Dewey, with no agent affordances.
//!
//! Identical on screen to `todo_dewey`; the difference between the two is what
//! agent-driveability costs in a non-trivial application.

use dewey::prelude::*;
use dewey::widget::input::TextInputState;
use dewey::widget::{Checkbox, StatefulWidget, TextInput};
use std::cell::RefCell;

#[derive(Clone, Copy, PartialEq)]
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
    SetFilter(u8),
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
            Msg::SetFilter(f) => {
                self.filter = match f {
                    1 => Filter::Active,
                    2 => Filter::Completed,
                    _ => Filter::All,
                }
            }
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

        // New-todo row: input + Add
        let top = Layout::horizontal([Constraint::Fill(1.0), Constraint::Length(80.0)])
            .split(rows[0]);
        TextInput::new()
            .placeholder("What needs doing?")
            .render(top[0], frame, &mut self.input.borrow_mut());
        Button::new("Add").agent_id("add").render(top[1], frame);

        // Filter row
        let filters = Layout::horizontal([Constraint::Ratio(1, 3); 3]).split(rows[1]);
        for (i, (label, id)) in [
            ("All", "filter_all"),
            ("Active", "filter_active"),
            ("Completed", "filter_completed"),
        ]
        .iter()
        .enumerate()
        {
            Button::new(*label).agent_id(*id).render(filters[i], frame);
        }

        // Todo list
        let visible = self.visible();
        let mut y = rows[2].y;
        for idx in visible {
            let row = Rect::new(rows[2].x, y, rows[2].width, 28.0);
            let cols = Layout::horizontal([
                Constraint::Length(24.0),
                Constraint::Fill(1.0),
                Constraint::Length(28.0),
            ])
            .split(row);
            Checkbox::new("", self.todos[idx].done)
                .render(cols[0], frame);
            Label::new(self.todos[idx].title.clone())
                .render(cols[1], frame);
            Button::new("x")
                .render(cols[2], frame);
            y += 28.0;
            if y > rows[2].bottom() - 28.0 {
                break;
            }
        }

        // Footer
        let foot = Layout::horizontal([Constraint::Fill(1.0), Constraint::Length(140.0)])
            .split(rows[3]);
        Label::new(format!("{} items left", self.remaining()))
            .render(foot[0], frame);
        Button::new("Clear completed")
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
