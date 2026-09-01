# Comparative Benchmarks

CPU frame-build cost for Dewey, [egui](https://github.com/emilk/egui) 0.31, and
[iced](https://github.com/iced-rs/iced) 0.13 on an identical scene.

```bash
cd benches/comparative
cargo bench
```

This is a standalone crate, deliberately *not* a workspace member or a
dev-dependency of the framework, so `cargo test` and the published package never
pull egui, iced, or tiny-skia.

## What is measured

The same nominal scene in every framework: a vertical list of N rows, each row
holding one text label and one button, with per-row unique strings (`Item {i}`,
`Action {i}`) so nothing can be cached across rows.

Every framework runs headless. The timed region is **widget construction +
layout + render-command generation** — one frame's CPU work. No GPU, no
rasterization, no windowing, no presentation.

- **Dewey** — widgets rendered through `Frame` into `TestBackend`, which records
  each `Painter` call as a `RenderOp`.
- **egui** — `Context::run` with a fixed `RawInput`, producing `FullOutput`.
  Tessellation is *not* included, to match Dewey's pre-backend stopping point.
- **iced** — widget tree built, then `Widget::layout` + `Widget::draw` against
  the `tiny-skia` renderer, which accumulates primitives into layers.

## Allocation counts (deterministic)

Wall-clock numbers on a loaded machine are worthless — see the warning below —
so the primary metric here is heap traffic per frame, which does not care what
else the CPU is doing:

```bash
cd benches/comparative && cargo run --release --bin allocs
```

Per row (one `Label` + one `Button`), 1000 rows:

| Configuration            | Allocations/row | Bytes/row |
| ------------------------ | --------------- | --------- |
| No agent ids             | 4.0             | 500       |
| Agent ids, ontology on   | 18.0 → **8.0**  | 3210 → **2308** |
| Agent ids, ontology off  | 18.0 → **4.0**  | 3210 → **598**  |

Three optimizations produced this, cutting agentic frames from 18.0 to 8.0
allocations per row (−56%) with an agent attached, and to 4.0 without:

1. **`Cow<'static, str>` for `UiNode::widget_type`, `UiNode::agent_id`, and
   hit-map ids.** Every call site passes a string literal, so a frame used to
   allocate and free a `String` per widget per field for no reason. Agentic
   frames dropped from 18.0 to 11.0 allocations per row (−39%) with an agent
   attached.
2. **`Properties` replaces `serde_json::Value` for `UiNode::state`.** Every
   widget built a `serde_json` map per frame and allocated a fresh `String`
   for each key, though the keys are string literals baked into the widget.
   `Properties` is a flat `Vec<(Cow<'static, str>, Value)>` that serializes
   identically, so keys cost nothing and the map collapses to one vector
   allocation: 11.0 → 8.0 per row (−27%), 3173 → 2308 bytes.
3. **`Frame::with_ontology(..., false)`.** A frame no agent will inspect used to
   build a `UiNode` per widget and throw it away. Widgets now check
   `frame.ontology_enabled()` *before* constructing the node, so the cost is
   skipped rather than discarded: 18.0 → 4.0 allocations per row (−78%) and
   3210 → 598 bytes per row (−81%), identical to a UI with no agent ids at all.
   Set it via `ProgramOptions::ontology` (default `true`, so nothing changes
   unless you opt out).

`Properties` compares by content rather than insertion order, deliberately:
`AgentSession` diffs a widget's previous state against its current one to
decide whether to emit `StateChanged`, and a round trip through `serde_json`
reorders keys because its map is sorted. Order-sensitive equality would report
unchanged state as changed and flood subscribers.

Hit-testing is deliberately *not* gated — node registration and hitbox
registration live in the same guarded block in every widget, and gating both
would leave buttons that render correctly but are dead to the mouse.
`ontology_gate_skips_nodes_but_keeps_hitboxes` in `tests/integration.rs` locks
this down.

## Results

```bash
cd benches/comparative && cargo run --release --bin timing
```

Windows 11, release profile. Fastest observed frame of 400/120/40 interleaved
rounds, best of two independent runs.

| Rows | Dewey       | Dewey, ontology off | Dewey, agentic | egui 0.31 | iced 0.13 |
| ---- | ----------- | ------------------- | -------------- | --------- | --------- |
| 100  | **19.4 µs** | 20.3 µs             | 44.6 µs        | 139.8 µs  | 56.2 µs   |
| 1000 | **205 µs**  | 208 µs              | 566 µs         | 1.97 ms   | 872 µs    |
| 5000 | **1.25 ms** | 1.23 ms             | 7.77 ms        | 15.35 ms  | 14.10 ms  |

The agentic column improved 23% / 21% / 37% against the pre-optimization
figures of 57.7 µs, 717 µs, and 12.24 ms. The first two reproduced closely
across runs (44.6/44.9 µs, 566/628 µs); the 5000-row figure ranged 7.77–11.41 ms,
so treat that one as "improved, magnitude uncertain" — the allocation counts
above are the firm evidence there.

Speedup over Dewey with no agent ids:

| Rows | vs egui | vs iced |
| ---- | ------- | ------- |
| 100  | 7.2×    | 2.9×    |
| 1000 | 9.6×    | 4.2×    |
| 5000 | 12.3×   | 11.3×   |

Three things the columns show:

- **The ontology gate is free.** `Dewey, ontology off` lands within 1–2% of the
  no-agent-ids column at every size, in both runs. Skipping node construction
  really does recover the entire cost.
- **The ontology is still the expensive part.** With it on, Dewey costs 2.3×
  the plain path at 100 rows and ~6× at 5000. The gap grows with widget count,
  so the agentic column, not the plain one, is what a real Dewey app pays.
- **Dewey leads at every size** on the like-for-like comparison, but by less
  than an earlier version of this file claimed (see below).

### Methodology, and why not criterion

An earlier revision reported these numbers from `cargo bench` and got them
badly wrong: it published iced at 51–55 ms and egui at 28–30 ms for 5000 rows,
roughly 3× and 2× worse than they actually are. That run was taken on a machine
pinned at 100% CPU by unrelated work, where criterion's mean/median measure the
machine's load rather than the code — it ranked 3000 rows slower than 5000 rows,
and moved an untouched code path by 3× between runs.

`src/bin/timing.rs` is built for a contended machine instead:

1. **Interleaved** — every framework runs once per round, so load drift is
   common-mode rather than landing on whichever framework ran last.
2. **Minimum, not mean** — the fastest observed frame approximates the
   uncontended cost. No round can run faster than the real work allows, while
   any round can be arbitrarily slowed by a competing process.
3. **Noise is reported** — each row prints median/min, so a run whose numbers
   were shaped by load is visible rather than silently published. These runs
   sat at 1.1–1.5×.

Run it at raised priority for the cleanest result. The residual spread between
the two runs was ≤3% at 100 rows and 6–27% at 1000–5000 rows; treat the
one-significant-figure ratios as solid and the rest as approximate.

## Caveats — read before quoting these numbers

1. **Dewey does not shape text during frame build.** `TestBackend::measure_text`
   estimates extents arithmetically (`0.6 × font_size × len`); egui shapes real
   galleys and iced shapes through cosmic-text. The `egui_text_shaping_only`
   benchmark isolates this: shaping the same 2N strings costs ~40 µs at 100 rows,
   ~0.5 ms at 1000, and ~2.5–5.9 ms at 5000 — roughly **10% of egui's frame**.
   Dewey's GPU backends pay this cost at render time instead. Adding it back
   still leaves Dewey ahead at every size, but the 5000-row gap narrows from
   ~12× to roughly ~4–5×.

2. **Dewey's frame build does less bookkeeping by design.** egui runs a full
   interaction pass (widget ids, focus, hover/click responses) inside the timed
   region; iced allocates boxed `Element`s and diffs a retained widget tree.
   Dewey defers interaction to a separate hit-map pass. Part of the gap is
   architecture, not raw efficiency.

3. **iced is measured with tree reuse.** An earlier version of this benchmark
   rebuilt `Tree::new` every iteration and made iced look ~20× worse than it is.
   The harness now calls `Tree::diff` against a persistent tree, as a real iced
   application does.

4. **This is not end-to-end frame time.** Tessellation, rasterization, GPU
   upload, and present are all excluded. A framework that generates commands
   quickly but produces more expensive command streams would not show up here.

5. **The Dewey column has no agent ids set.** That is the fair comparison —
   egui and iced have no ontology to build — but it is not how a Dewey app is
   written. With agent ids on every widget and the ontology enabled, frame
   build costs roughly an order of magnitude more; the `dewey_agentic`
   benchmark measures exactly that, and the allocation work above is aimed at
   closing the gap.

6. **Slint, Dioxus, and GTK are not included.** Slint requires build-time
   codegen, Dioxus targets a DOM/VDOM model, and GTK needs a display server —
   none has a comparable headless command-generation entry point, so any number
   would be measuring a different thing.
