//! Cross-framework CPU benchmark.
//!
//! Same nominal scene in every framework: a vertical list of N rows, each row
//! holding one text label and one button. Every framework runs headless with a
//! no-op / recording backend, so the numbers are CPU frame-build cost only:
//! widget construction + layout + render-command generation. No GPU, no
//! rasterization, no windowing.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

const SIZES: [usize; 3] = [100, 1_000, 5_000];

// ── Dewey ──────────────────────────────────────────────────────────

fn dewey_scene(n: usize) -> usize {
    use dewey::backend::test::TestBackend;
    use dewey::core::Rect;
    use dewey::event::HitMap;
    use dewey::runtime::Frame;
    use dewey::widget::{Button, Label, Widget};

    let mut painter = TestBackend::new(1280.0, 720.0);
    let mut hit_map = HitMap::new();
    let mut frame = Frame::new(Rect::from_size(1280.0, 720.0), &mut hit_map, &mut painter);

    for i in 0..n {
        let y = (i % 30) as f32 * 24.0;
        Label::new(format!("Item {i}")).render(Rect::new(0.0, y, 200.0, 24.0), &mut frame);
        Button::new(format!("Action {i}")).render(Rect::new(210.0, y, 120.0, 24.0), &mut frame);
    }
    n
}

// ── egui ───────────────────────────────────────────────────────────

fn egui_scene(ctx: &egui::Context, n: usize) -> usize {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1280.0, 720.0),
        )),
        ..Default::default()
    };
    let output = ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            for i in 0..n {
                ui.horizontal(|ui| {
                    ui.label(format!("Item {i}"));
                    let _ = ui.button(format!("Action {i}"));
                });
            }
        });
    });
    black_box(&output.shapes).len()
}

// ── iced ───────────────────────────────────────────────────────────

fn iced_scene(
    renderer: &mut iced_renderer::Renderer,
    tree: &mut iced_core::widget::Tree,
    n: usize,
) -> usize {
    use iced_core::{layout, mouse, renderer::Style, Element, Renderer as _, Size};
    use iced_widget::{button, column, row, text};

    type Theme = iced_widget::Theme;
    type R = iced_renderer::Renderer;

    let rows: Vec<Element<'_, (), Theme, R>> = (0..n)
        .map(|i| {
            row![
                text(format!("Item {i}")),
                button(text(format!("Action {i}")))
            ]
            .into()
        })
        .collect();

    let content: Element<'_, (), Theme, R> = column(rows).into();
    let widget = content.as_widget();

    renderer.clear();
    tree.diff(&content);
    let limits = layout::Limits::new(Size::ZERO, Size::new(1280.0, 720.0));
    let node = widget.layout(tree, renderer, &limits);
    let layout = layout::Layout::new(&node);

    widget.draw(
        tree,
        renderer,
        &Theme::default(),
        &Style::default(),
        layout,
        mouse::Cursor::Unavailable,
        &iced_core::Rectangle::with_size(Size::new(1280.0, 720.0)),
    );
    black_box(node.size().width) as usize
}

// ── Isolated text shaping (the main asymmetry) ─────────────────────

/// Shapes the same 2N strings the scene draws, and nothing else. Dewey's
/// TestBackend estimates text extents arithmetically instead of shaping, so
/// this is the slice of egui's frame that Dewey's number does not contain.
fn egui_text_shaping(ctx: &egui::Context, n: usize) -> usize {
    let font = egui::FontId::proportional(14.0);
    let mut total = 0.0f32;
    ctx.fonts(|f| {
        for i in 0..n {
            let a = f.layout_no_wrap(format!("Item {i}"), font.clone(), egui::Color32::WHITE);
            let b = f.layout_no_wrap(format!("Action {i}"), font.clone(), egui::Color32::WHITE);
            total += a.size().x + b.size().x;
        }
    });
    black_box(total) as usize
}

// ── Harness ────────────────────────────────────────────────────────

fn bench_frame_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("frame_build");
    let ctx = egui::Context::default();
    // Warm egui's font atlas and style caches so the first sample is not an outlier.
    let _ = egui_scene(&ctx, 10);
    let mut iced_renderer_inst =
        iced_renderer::Renderer::new(Default::default(), iced_core::Pixels(16.0));
    let mut iced_tree = iced_core::widget::Tree::empty();
    let _ = iced_scene(&mut iced_renderer_inst, &mut iced_tree, 10);

    for n in SIZES {
        group.bench_with_input(BenchmarkId::new("dewey", n), &n, |b, &n| {
            b.iter(|| black_box(dewey_scene(black_box(n))))
        });
        group.bench_with_input(BenchmarkId::new("egui", n), &n, |b, &n| {
            b.iter(|| black_box(egui_scene(&ctx, black_box(n))))
        });
        group.bench_with_input(BenchmarkId::new("egui_text_shaping_only", n), &n, |b, &n| {
            b.iter(|| black_box(egui_text_shaping(&ctx, black_box(n))))
        });
        group.bench_with_input(BenchmarkId::new("iced", n), &n, |b, &n| {
            b.iter(|| black_box(iced_scene(&mut iced_renderer_inst, &mut iced_tree, black_box(n))))
        });
    }
    group.finish();
}

criterion_group!(benches, bench_frame_build);
criterion_main!(benches);
