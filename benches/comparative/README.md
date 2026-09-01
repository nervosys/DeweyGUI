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

| Configuration                      | Allocations/row | Bytes/row |
| ---------------------------------- | --------------- | --------- |
| No agent ids                       | 4.0             | 500       |
| Agent ids, ontology on — *before*  | 18.0            | 3210      |
| Agent ids, ontology on — *after*   | **11.0**        | 3173      |
| Agent ids, ontology off — *after*  | **4.0**         | 598       |

Two optimizations produced this:

1. **`Cow<'static, str>` for `UiNode::widget_type`, `UiNode::agent_id`, and
   hit-map ids.** Every call site passes a string literal, so a frame used to
   allocate and free a `String` per widget per field for no reason. Agentic
   frames dropped from 18.0 to 11.0 allocations per row (−39%) with an agent
   attached.
2. **`Frame::with_ontology(..., false)`.** A frame no agent will inspect used to
   build a `UiNode` per widget and throw it away. Widgets now check
   `frame.ontology_enabled()` *before* constructing the node, so the cost is
   skipped rather than discarded: 18.0 → 4.0 allocations per row (−78%) and
   3210 → 598 bytes per row (−81%), identical to a UI with no agent ids at all.
   Set it via `ProgramOptions::ontology` (default `true`, so nothing changes
   unless you opt out).

Hit-testing is deliberately *not* gated — node registration and hitbox
registration live in the same guarded block in every widget, and gating both
would leave buttons that render correctly but are dead to the mouse.
`ontology_gate_skips_nodes_but_keeps_hitboxes` in `tests/integration.rs` locks
this down.

## Results

> ⚠️ **These timings are provisional.** They were captured on a machine pinned
> at 100% CPU by unrelated work. A later run measured the *unchanged* plain
> path at 70 µs where it had previously measured 22 µs — a 3× swing on code
> that did not change — and measured 3000 rows as slower than 5000 rows, which
> is impossible. The ordering below was stable across two runs, but the ratios
> should be re-measured on an idle machine before being quoted anywhere.
> The allocation counts above are unaffected by load and can be trusted.

Windows 11, release profile, criterion, two runs of 8 s measurement time each.
Ranges below span the medians of both runs.

| Rows  | Dewey          | egui           | iced           | Dewey vs egui | Dewey vs iced |
| ----- | -------------- | -------------- | -------------- | ------------- | ------------- |
| 100   | **27–35 µs**   | 193–283 µs     | 94–111 µs      | 5.4–8.0×      | 2.7–3.5×      |
| 1000  | **0.35–0.65 ms** | 3.1–8.0 ms   | 1.02–1.08 ms   | 4.8–12×       | 1.6–3.1×      |
| 5000  | **1.7–2.4 ms** | 28–30 ms       | 51–55 ms       | 11.5–17×      | 21–32×        |

Dewey was fastest in all six comparisons across both runs. The ordering
Dewey < iced < egui held at 100 and 1000 rows; at 5000 rows iced overtakes egui
(iced's cost grows superlinearly as its layer stack and layout tree grow).

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
