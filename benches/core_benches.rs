use criterion::{Criterion, black_box, criterion_group, criterion_main};
use std::time::Duration;

use dewey::animation::{Animation, Easing, Tween};
use dewey::ontology::{SemanticRole, UiNode, UiTree};
use dewey::widget::VirtualList;

// ── Animation benchmarks ───────────────────────────────────────────

fn bench_easing_apply(c: &mut Criterion) {
    let mut group = c.benchmark_group("easing");
    for easing in [
        Easing::Linear,
        Easing::EaseInOutCubic,
        Easing::EaseInOutExpo,
    ] {
        group.bench_with_input(
            criterion::BenchmarkId::new("apply", format!("{easing:?}")),
            &easing,
            |b, &easing| {
                b.iter(|| {
                    for i in 0..100 {
                        black_box(easing.apply(i as f32 / 100.0));
                    }
                });
            },
        );
    }
    group.finish();
}

fn bench_tween_tick(c: &mut Criterion) {
    c.bench_function("tween_tick_100_frames", |b| {
        b.iter(|| {
            let mut tween =
                Tween::new(0.0, 1.0, Duration::from_millis(300), Easing::EaseInOutCubic);
            let dt = Duration::from_millis(16);
            for _ in 0..100 {
                tween.tick(dt);
                black_box(tween.value());
            }
        });
    });
}

// ── Ontology benchmarks ────────────────────────────────────────────

fn bench_ontology_registry_build(c: &mut Criterion) {
    c.bench_function("ontology_build_100_node_tree", |b| {
        b.iter(|| {
            let mut root = UiNode::new("root", SemanticRole::Container);
            for i in 0..100 {
                let id = format!("widget_{i}");
                let node = UiNode::new(id.clone(), SemanticRole::Display)
                    .with_id(id)
                    .with_property("index", serde_json::json!(i));
                root.children.push(node);
            }
            let tree = UiTree::new(root);
            black_box(&tree);
        });
    });
}

fn bench_ontology_serialize(c: &mut Criterion) {
    let mut root = UiNode::new("root", SemanticRole::Container);
    for i in 0..50 {
        let id = format!("widget_{i}");
        let node = UiNode::new(id.clone(), SemanticRole::Display)
            .with_id(id)
            .with_property("value", serde_json::json!(i));
        root.children.push(node);
    }
    let tree = UiTree::new(root);

    c.bench_function("ontology_tree_serialize_50_nodes", |b| {
        b.iter(|| {
            let json = serde_json::to_string(black_box(&tree)).unwrap();
            black_box(json);
        });
    });
}

// ── Widget benchmarks ──────────────────────────────────────────────

fn bench_virtual_list_visible_range(c: &mut Criterion) {
    c.bench_function("virtual_list_visible_range_10k", |b| {
        b.iter(|| {
            let range = VirtualList::<fn(usize, dewey::core::Rect, &mut dewey::runtime::Frame<'_>)>::visible_range(
                black_box(5000.0),
                black_box(600.0),
                black_box(24.0),
                black_box(10_000),
                black_box(2),
            );
            black_box(range);
        });
    });
}

criterion_group!(
    benches,
    bench_easing_apply,
    bench_tween_tick,
    bench_ontology_registry_build,
    bench_ontology_serialize,
    bench_virtual_list_visible_range,
);
criterion_main!(benches);
