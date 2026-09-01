//! Canonical complex app — TodoMVC, iced.

use iced::widget::{button, checkbox, column, row, text, text_input};
use iced::{Element, Size};

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
    input: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            todos: Vec::new(),
            filter: Filter::All,
            input: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
enum Msg {
    Input(String),
    Add,
    Toggle(usize, bool),
    Delete(usize),
    SetFilter(u8),
    ClearCompleted,
}

impl App {
    fn update(&mut self, msg: Msg) {
        match msg {
            Msg::Input(s) => self.input = s,
            Msg::Add => {
                let title = self.input.trim().to_string();
                if !title.is_empty() {
                    self.todos.push(Todo { title, done: false });
                    self.input.clear();
                }
            }
            Msg::Toggle(i, v) => {
                if let Some(t) = self.todos.get_mut(i) {
                    t.done = v;
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
    }

    fn view(&self) -> Element<'_, Msg> {
        let items: Vec<Element<'_, Msg>> = self
            .todos
            .iter()
            .enumerate()
            .filter(|(_, t)| match self.filter {
                Filter::All => true,
                Filter::Active => !t.done,
                Filter::Completed => t.done,
            })
            .map(|(i, t)| {
                row![
                    checkbox("", t.done).on_toggle(move |v| Msg::Toggle(i, v)),
                    text(t.title.clone()),
                    button("x").on_press(Msg::Delete(i)),
                ]
                .into()
            })
            .collect();

        let left = self.todos.iter().filter(|t| !t.done).count();
        column![
            row![
                text_input("What needs doing?", &self.input).on_input(Msg::Input),
                button("Add").on_press(Msg::Add),
            ],
            row![
                button("All").on_press(Msg::SetFilter(0)),
                button("Active").on_press(Msg::SetFilter(1)),
                button("Completed").on_press(Msg::SetFilter(2)),
            ],
            column(items),
            row![
                text(format!("{left} items left")),
                button("Clear completed").on_press(Msg::ClearCompleted),
            ],
        ]
        .into()
    }
}

fn main() -> iced::Result {
    iced::application("Todo", App::update, App::view)
        .window_size(Size::new(480.0, 400.0))
        .run()
}
