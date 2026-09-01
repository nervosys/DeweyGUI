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

## Results

Windows 11, release profile, criterion, two runs of 8 s measurement time each.
Ranges below span the medians of both runs — this machine is noisy, so treat
these as order-of-magnitude, not precise ratios.

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

5. **Slint, Dioxus, and GTK are not included.** Slint requires build-time
   codegen, Dioxus targets a DOM/VDOM model, and GTK needs a display server —
   none has a comparable headless command-generation entry point, so any number
   would be measuring a different thing.
