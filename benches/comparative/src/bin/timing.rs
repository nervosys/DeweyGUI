//! Contention-robust timing harness.
//!
//! Criterion reports mean/median, which on a loaded machine measure the load
//! rather than the code — it put 3000 rows above 5000 rows and moved an
//! unchanged path 3x between runs.
//!
//! This harness instead:
//!   1. interleaves the frameworks within each round, so drift in machine load
//!      hits all of them equally rather than whichever ran last;
//!   2. reports the MINIMUM observed time, which approximates the uncontended
//!      cost — some rounds get a clean CPU slice, and no round can run faster
//!      than the real work allows.
//! The median is printed alongside to show how much noise was rejected.

use std::hint::black_box;
use std::time::{Duration, Instant};

// ── Scenes (identical to benches/cross_framework.rs) ───────────────

fn dewey_scene(n: usize, agentic: bool, ontology: bool) -> usize {
    let mut painter = dewey::backend::test::TestBackend::new(1280.0, 720.0);
    dewey_scene_with(&mut painter, n, agentic, ontology)
}

/// The ontology-only pass `OntologyMode::OnDemand` runs: widgets and layout,
/// but a `NullPainter` instead of real drawing.
fn dewey_ontology_pass(n: usize) -> usize {
    let mut painter = dewey::paint::NullPainter;
    dewey_scene_with(&mut painter, n, true, true)
}

fn dewey_scene_with(
    painter: &mut dyn dewey::paint::Painter,
    n: usize,
    agentic: bool,
    ontology: bool,
) -> usize {
    use dewey::core::Rect;
    use dewey::event::HitMap;
    use dewey::runtime::Frame;
    use dewey::widget::{Button, Label, Widget};

    let mut hit_map = HitMap::new();
    let mut frame = Frame::with_ontology(
        Rect::from_size(1280.0, 720.0),
        &mut hit_map,
        painter,
        ontology,
    );
    for i in 0..n {
        let y = (i % 30) as f32 * 24.0;
        let l = Label::new(format!("Item {i}"));
        let b = Button::new(format!("Action {i}"));
        let (l, b) = if agentic {
            (l.agent_id("item"), b.agent_id("action"))
        } else {
            (l, b)
        };
        l.render(Rect::new(0.0, y, 200.0, 24.0), &mut frame);
        b.render(Rect::new(210.0, y, 120.0, 24.0), &mut frame);
    }
    frame.take_nodes().len()
}

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
    output.shapes.len()
}

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
    node.size().width as usize
}

// ── Harness ────────────────────────────────────────────────────────

const LABELS: [&str; 6] = [
    "dewey (no agent ids)",
    "dewey (agentic, ontology on)",
    "dewey (agentic, ontology off)",
    "dewey (on-demand ontology pass)",
    "egui 0.31",
    "iced 0.13",
];

fn fmt(d: Duration) -> String {
    let us = d.as_secs_f64() * 1e6;
    if us < 1.0 {
        return format!("{:.0} ns", d.as_secs_f64() * 1e9);
    }
    if us >= 1000.0 {
        format!("{:.2} ms", us / 1000.0)
    } else {
        format!("{us:.1} µs")
    }
}

fn main() {
    let ctx = egui::Context::default();
    let mut ir = iced_renderer::Renderer::new(Default::default(), iced_core::Pixels(16.0));
    let mut itree = iced_core::widget::Tree::empty();

    // Warm caches: egui's font atlas, iced's shaping caches, allocator arenas.
    for _ in 0..3 {
        black_box(dewey_scene(64, true, true));
        black_box(egui_scene(&ctx, 64));
        black_box(iced_scene(&mut ir, &mut itree, 64));
    }

    for (n, rounds) in [(100usize, 400usize), (1_000, 120), (5_000, 40)] {
        let mut samples: Vec<Vec<Duration>> = vec![Vec::with_capacity(rounds); LABELS.len()];
        for _ in 0..rounds {
            // Interleaved within the round so load drift is common-mode.
            let t = Instant::now();
            black_box(dewey_scene(n, false, true));
            samples[0].push(t.elapsed());

            let t = Instant::now();
            black_box(dewey_scene(n, true, true));
            samples[1].push(t.elapsed());

            let t = Instant::now();
            black_box(dewey_scene(n, true, false));
            samples[2].push(t.elapsed());

            let t = Instant::now();
            black_box(dewey_ontology_pass(n));
            samples[3].push(t.elapsed());

            let t = Instant::now();
            black_box(egui_scene(&ctx, n));
            samples[4].push(t.elapsed());

            let t = Instant::now();
            black_box(iced_scene(&mut ir, &mut itree, n));
            samples[5].push(t.elapsed());
        }

        println!("\n── {n} rows ({rounds} interleaved rounds) ──");
        println!("{:<32} {:>12} {:>12} {:>8}", "", "min", "median", "noise");
        let mut mins = Vec::new();
        for (i, label) in LABELS.iter().enumerate() {
            samples[i].sort_unstable();
            let min = samples[i][0];
            let med = samples[i][samples[i].len() / 2];
            mins.push(min);
            println!(
                "{label:<32} {:>12} {:>12} {:>7.1}x",
                fmt(min),
                fmt(med),
                med.as_secs_f64() / min.as_secs_f64()
            );
        }
        let base = mins[0].as_secs_f64();
        // Amortized: 60 fps with an agent querying the tree 5 times a second.
        let every_frame = 60.0 * mins[1].as_secs_f64();
        let on_demand = 60.0 * mins[0].as_secs_f64() + 5.0 * mins[3].as_secs_f64();
        println!(
            "  amortized/sec @60fps+5 queries:  every-frame {:.1} ms   on-demand {:.1} ms   ({:.1}x less)",
            every_frame * 1000.0,
            on_demand * 1000.0,
            every_frame / on_demand,
        );
        println!(
            "  dewey(plain) vs egui: {:.1}x   vs iced: {:.1}x   agentic-on vs plain: {:.1}x",
            mins[4].as_secs_f64() / base,
            mins[5].as_secs_f64() / base,
            mins[1].as_secs_f64() / base,
        );
    }
}
