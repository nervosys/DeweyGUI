//! Deterministic allocation counter for one frame build.
//!
//! Wall-clock benchmarks are unreliable on a loaded machine; allocation counts
//! are not. This measures heap traffic per frame, which is what the ontology
//! hot path actually spends its time on.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);

struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(l.size(), Ordering::Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
}

#[global_allocator]
static A: Counting = Counting;

fn reset() {
    ALLOCS.store(0, Ordering::Relaxed);
    BYTES.store(0, Ordering::Relaxed);
}
fn read() -> (usize, usize) {
    (ALLOCS.load(Ordering::Relaxed), BYTES.load(Ordering::Relaxed))
}

/// Carries the index a real application would act on. Never read here —
/// this benchmark counts allocations, it does not run `update`.
#[derive(Debug)]
#[allow(dead_code)]
enum BenchMsg {
    Clicked(usize),
}

fn scene(n: usize, agentic: bool, ontology: bool) -> usize {
    scene_inner(n, agentic, ontology, false)
}

/// Same scene, but buttons carry their message via `action`, which boxes it.
fn scene_actions(n: usize, ontology: bool) -> usize {
    scene_inner(n, true, ontology, true)
}

fn scene_inner(n: usize, agentic: bool, ontology: bool, actions: bool) -> usize {
    use dewey::backend::test::TestBackend;
    use dewey::core::Rect;
    use dewey::event::HitMap;
    use dewey::runtime::Frame;
    use dewey::widget::{Button, Label, Widget};

    let mut painter = TestBackend::new(1280.0, 720.0);
    let mut hit_map = HitMap::new();
    let mut frame = Frame::with_ontology(
        Rect::from_size(1280.0, 720.0),
        &mut hit_map,
        &mut painter,
        ontology,
    );

    for i in 0..n {
        let y = (i % 30) as f32 * 24.0;
        let l = Label::new(format!("Item {i}"));
        let b = Button::new(format!("Action {i}"));
        let (l, b) = if actions {
            (l.agent_id("item"), b.action("action", BenchMsg::Clicked(i)))
        } else if agentic {
            (l.agent_id("item"), b.agent_id("action"))
        } else {
            (l, b)
        };
        l.render(Rect::new(0.0, y, 200.0, 24.0), &mut frame);
        b.render(Rect::new(210.0, y, 120.0, 24.0), &mut frame);
    }
    frame.take_nodes().len()
}

/// What a row of an agent-driveable interface is allowed to cost.
///
/// These are the figures published in the README, and unlike a wall-clock
/// timing they do not move with the machine — which is what makes them worth
/// gating on. Raise one deliberately when a change is worth it, and say so;
/// do not let it drift.
///
/// Each allowance is the measured figure plus the small fixed cost of setting
/// a scene up, which is why they are not the round numbers printed per row: a
/// thousand rows costing 4020 allocations is 4.02 each, not 4.0.
struct Budget {
    label: &'static str,
    allocs_per_row: f64,
    bytes_per_row: f64,
}

const BUDGETS: &[Budget] = &[
    Budget {
        label: "plain, ontology on   ",
        allocs_per_row: 4.03,
        bytes_per_row: 540.0,
    },
    Budget {
        label: "agentic, ontology on ",
        allocs_per_row: 6.04,
        bytes_per_row: 1800.0,
    },
    Budget {
        label: "agentic, ontology off",
        allocs_per_row: 4.03,
        bytes_per_row: 610.0,
    },
    Budget {
        label: "action(), ontology on ",
        allocs_per_row: 7.04,
        bytes_per_row: 1900.0,
    },
    Budget {
        label: "action(), ontology off",
        allocs_per_row: 5.03,
        bytes_per_row: 730.0,
    },
];

/// Records a measurement against its budget; returns whether it fits.
fn within_budget(label: &str, allocs: f64, bytes: f64) -> bool {
    let Some(budget) = BUDGETS.iter().find(|b| b.label == label) else {
        return true;
    };
    let ok = allocs <= budget.allocs_per_row && bytes <= budget.bytes_per_row;
    if !ok {
        eprintln!(
            "OVER BUDGET {label}: {allocs:.1} allocs/row (max {:.1}), \
             {bytes:.0} bytes/row (max {:.0})",
            budget.allocs_per_row, budget.bytes_per_row
        );
    }
    ok
}

fn main() {
    const N: usize = 1000;
    let mut all_within = true;
    // Warm up so lazy statics / atlas init are not counted.
    let _ = scene(16, true, true);

    let cases = [
        ("plain, ontology on   ", false, true),
        ("agentic, ontology on ", true, true),
        ("agentic, ontology off", true, false),
    ];
    for (label, agentic, ontology) in cases {
        reset();
        let nodes = scene(N, agentic, ontology);
        let (allocs, bytes) = read();
        let (per_alloc, per_byte) = (allocs as f64 / N as f64, bytes as f64 / N as f64);
        all_within &= within_budget(label, per_alloc, per_byte);
        println!(
            "{label}  n={N:>5}  nodes={nodes:>5}  allocs={allocs:>7}  ({per_alloc:>5.1}/row)  bytes={bytes:>9} ({per_byte:>6.0}/row)",
        );
    }

    // What `Button::action` costs: one boxed message per interactive widget.
    let _ = scene_actions(16, true);
    for (label, ontology) in [("action(), ontology on ", true), ("action(), ontology off", false)] {
        reset();
        let nodes = scene_actions(N, ontology);
        let (allocs, bytes) = read();
        let (per_alloc, per_byte) = (allocs as f64 / N as f64, bytes as f64 / N as f64);
        all_within &= within_budget(label, per_alloc, per_byte);
        println!(
            "{label}  n={N:>5}  nodes={nodes:>5}  allocs={allocs:>7}  ({per_alloc:>5.1}/row)  bytes={bytes:>9} ({per_byte:>6.0}/row)",
        );
    }

    if !all_within {
        eprintln!(
            "\nAllocation budgets exceeded. These figures are published and do \
             not move with the machine; a change that raises one should raise \
             the budget in this file deliberately."
        );
        std::process::exit(1);
    }
    println!("\nall within budget");
}
