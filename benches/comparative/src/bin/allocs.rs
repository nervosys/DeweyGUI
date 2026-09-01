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

fn scene(n: usize, agentic: bool, ontology: bool) -> usize {
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

fn main() {
    const N: usize = 1000;
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
        println!(
            "{label}  n={N:>5}  nodes={nodes:>5}  allocs={allocs:>7}  ({:>5.1}/row)  bytes={bytes:>9} ({:>6.0}/row)",
            allocs as f64 / N as f64,
            bytes as f64 / N as f64,
        );
    }
}
