//! Canonical counter — iced.

use iced::widget::{button, column, row, text};
use iced::{Element, Size};

#[derive(Default)]
struct App {
    count: i32,
}

#[derive(Debug, Clone, Copy)]
enum Msg {
    Increment,
    Decrement,
    Reset,
}

impl App {
    fn update(&mut self, msg: Msg) {
        match msg {
            Msg::Increment => self.count += 1,
            Msg::Decrement => self.count -= 1,
            Msg::Reset => self.count = 0,
        }
    }

    fn view(&self) -> Element<'_, Msg> {
        column![
            text(format!("Count: {}", self.count)),
            row![
                button("- Decrement").on_press(Msg::Decrement),
                button("Reset").on_press(Msg::Reset),
                button("+ Increment").on_press(Msg::Increment),
            ]
        ]
        .into()
    }
}

fn main() -> iced::Result {
    iced::application("Counter", App::update, App::view)
        .window_size(Size::new(400.0, 200.0))
        .run()
}
