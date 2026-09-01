//! Canonical complex app — TodoMVC, Dewey, agent-driveable.

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

    fn add(&mut self) {
        let title = self.input.borrow().text.trim().to_string();
        if !title.is_empty() {
            self.todos.push(Todo { title, done: false });
            *self.input.borrow_mut() = TextInputState::new();
        }
    }
}

impl Model for App {
    type Msg = ();

    fn update(&mut self, _msg: ()) -> Command<()> {
        Command::None
    }

    fn view(&self, frame: &mut Frame<'_>) {
        let h = frame.area.height;
        let rows = frame.area.rows_of(&[36.0, 32.0, h - 96.0, 28.0]);

        let top = rows[0].cols_of(&[rows[0].width - 80.0, 80.0]);
        TextInput::new()
            .placeholder("What needs doing?")
            .on_input("new_todo", |a: &mut App, t: &str| {
                *a.input.borrow_mut() = TextInputState::new().with_text(t)
            })
            .render(top[0], frame, &mut self.input.borrow_mut());
        Button::new("Add").on("add", App::add).render(top[1], frame);

        let f = rows[1].split_columns(3);
        Button::new("All")
            .on("filter_all", |a: &mut App| a.filter = Filter::All)
            .render(f[0], frame);
        Button::new("Active")
            .on("filter_active", |a: &mut App| a.filter = Filter::Active)
            .render(f[1], frame);
        Button::new("Completed")
            .on("filter_completed", |a: &mut App| a.filter = Filter::Completed)
            .render(f[2], frame);

        for (i, row) in self.visible().into_iter().zip(rows[2].rows(28.0)) {
            let c = row.cols_of(&[24.0, row.width - 52.0, 28.0]);
            Checkbox::new("", self.todos[i].done)
                .on(format!("toggle_{i}"), move |a: &mut App| {
                    a.todos[i].done = !a.todos[i].done
                })
                .render(c[0], frame);
            Label::new(self.todos[i].title.clone())
                .agent_id(format!("item_{i}"))
                .render(c[1], frame);
            Button::new("x")
                .on(format!("delete_{i}"), move |a: &mut App| {
                    a.todos.remove(i);
                })
                .render(c[2], frame);
        }

        let foot = rows[3].cols_of(&[rows[3].width - 140.0, 140.0]);
        Label::new(format!("{} items left", self.remaining()))
            .agent_id("remaining")
            .render(foot[0], frame);
        Button::new("Clear completed")
            .on("clear_completed", |a: &mut App| a.todos.retain(|t| !t.done))
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
