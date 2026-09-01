//! Canonical complex app — TodoMVC, egui / eframe.

use eframe::egui;

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

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.input)
                        .hint_text("What needs doing?"),
                );
                if ui.button("Add").clicked() {
                    let title = self.input.trim().to_string();
                    if !title.is_empty() {
                        self.todos.push(Todo { title, done: false });
                        self.input.clear();
                    }
                }
            });

            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.filter, Filter::All, "All");
                ui.selectable_value(&mut self.filter, Filter::Active, "Active");
                ui.selectable_value(&mut self.filter, Filter::Completed, "Completed");
            });

            let mut delete = None;
            for (i, todo) in self.todos.iter_mut().enumerate() {
                let visible = match self.filter {
                    Filter::All => true,
                    Filter::Active => !todo.done,
                    Filter::Completed => todo.done,
                };
                if !visible {
                    continue;
                }
                ui.horizontal(|ui| {
                    ui.checkbox(&mut todo.done, "");
                    ui.label(&todo.title);
                    if ui.button("x").clicked() {
                        delete = Some(i);
                    }
                });
            }
            if let Some(i) = delete {
                self.todos.remove(i);
            }

            ui.horizontal(|ui| {
                let left = self.todos.iter().filter(|t| !t.done).count();
                ui.label(format!("{left} items left"));
                if ui.button("Clear completed").clicked() {
                    self.todos.retain(|t| !t.done);
                }
            });
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([480.0, 400.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Todo",
        options,
        Box::new(|_cc| {
            Ok(Box::new(App {
                todos: Vec::new(),
                filter: Filter::All,
                input: String::new(),
            }))
        }),
    )
}
