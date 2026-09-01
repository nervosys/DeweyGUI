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

| Configuration           | Allocations/row | Bytes/row       |
| ----------------------- | --------------- | --------------- |
| No agent ids            | 4.0             | 500             |
| Agent ids, ontology on  | 18.0 → **6.0**  | 3210 → **2290** |
| Agent ids, ontology off | 18.0 → **4.0**  | 3210 → **598**  |

Step by step, with an agent attached: 18.0 → 11.0 (borrowed ids) → 8.0
(borrowed property keys) → 6.0 (moved owned values), a 67% reduction. The
remaining 6.0 is 4.0 that the plain path also pays plus one `Properties`
vector per widget.

Four optimizations produced this:

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
3. **Owned values move into the state instead of being cloned.** `json!(expr)`
   takes its argument by reference, so `json!(self.text)` cloned the string
   every frame — and for `List`, `Select`, and `Tabs`, cloned an entire
   `Vec<String>` every frame. Widgets now build their `UiNode` at the *end* of
   `render`, after painting has finished borrowing those fields, and move them
   in: 8.0 → 6.0 allocations per row, and for a list widget one allocation per
   item per frame becomes zero. Node registration still happens in render
   order, pinned by `node_registration_order_follows_render_order`.
   `Table` and the four widgets with early returns in `render` were left alone
   — moving registration past a `return` would silently drop it.
4. **`Frame::with_ontology(..., false)`.** A frame no agent will inspect used to
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

Windows 11, release profile, raised priority. Fastest observed frame of
400/120/40 interleaved rounds. **All figures below come from a single run**,
because absolute times move with whatever else the machine is doing — a later
run measured egui 20% faster with egui's code untouched. The stable, load-
independent quantity is the ratio *within* a run, so that is what the
optimization claims rest on.

| Rows | Dewey        | Dewey, ontology off | Dewey, agentic | egui 0.31 | iced 0.13 |
| ---- | ------------ | ------------------- | -------------- | --------- | --------- |
| 100  | **13.6 µs**  | 14.5 µs             | 29.2 µs        | 111.2 µs  | 43.5 µs   |
| 1000 | **132.3 µs** | 136.2 µs            | 283.7 µs       | 1.56 ms   | 492.1 µs  |
| 5000 | **926.5 µs** | 1.01 ms             | 5.27 ms        | 15.04 ms  | 15.25 ms  |

Speedup over Dewey with no agent ids:

| Rows | vs egui | vs iced |
| ---- | ------- | ------- |
| 100  | 8.2×    | 3.2×    |
| 1000 | 11.8×   | 3.7×    |
| 5000 | 16.2×   | 16.5×   |

A second run agreed on every ratio (8.1× / 11.8× / 13.0× vs egui, and the
agentic ratios below identical to one decimal place).

### The agentic path, measured as a within-run ratio

How much an agent-driven UI costs over the same UI with no agent ids — the
number the optimization work was aimed at, and the one that does not care how
loaded the machine is:

| Rows | After borrowed ids + gate | After borrowed keys | After moved values |
| ---- | ------------------------- | ------------------- | ------------------ |
| 100  | 3.0×                      | 2.3×                | **2.1×**           |
| 1000 | 4.2×                      | 2.7×                | **2.1×**           |
| 5000 | 9.1×                      | 6.2×                | **5.7×**           |

Three things the columns show:

- **The ontology gate is free.** `Dewey, ontology off` lands within 1–9% of the
  no-agent-ids column at every size, in both runs. Skipping node construction
  recovers essentially the entire cost.
- **The ontology is still the expensive part**, but far less so: an agentic
  frame cost 4.2× the plain path at 1000 rows before this work and 2.1× after.
  It still grows with widget count, so the agentic column, not the plain one,
  is what a real Dewey app pays.
- **Dewey leads at every size** on the like-for-like comparison.

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
