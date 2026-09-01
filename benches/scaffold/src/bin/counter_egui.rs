//! Canonical counter — egui / eframe.

use eframe::egui;

struct App {
    count: i32,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label(format!("Count: {}", self.count));
            ui.horizontal(|ui| {
                if ui.button("- Decrement").clicked() {
                    self.count -= 1;
                }
                if ui.button("Reset").clicked() {
                    self.count = 0;
                }
                if ui.button("+ Increment").clicked() {
                    self.count += 1;
                }
            });
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([400.0, 200.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Counter",
        options,
        Box::new(|_cc| Ok(Box::new(App { count: 0 }))),
    )
}
