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
| Agent ids, ontology on  | 18.0 → **6.0**  | 3210 → **1767** |
| Agent ids, ontology off | 18.0 → **4.0**  | 3210 → **598**  |

Step by step, with an agent attached: 18.0 → 11.0 (borrowed ids) → 8.0
(borrowed property keys) → 6.0 (moved owned values), a 67% reduction. The
remaining 6.0 is 4.0 that the plain path also pays plus one `Properties`
vector per widget.

Bytes fell further than allocations because `UiNode` itself shrank from **304
to 176 bytes (−42%)**: `Accessibility` is 136 bytes of mostly-`None` options
and was stored inline in every node, though no widget in the library sets it.
It is now `Option<Box<Accessibility>>`, read through `UiNode::accessibility()`,
which returns an empty set when unset. No widget pays a box it does not use,
and the wire format is unchanged — absent when unset, a plain object when set.
This is a memory-bandwidth win rather than an allocation win: the count stayed
at 6.0, bytes per row fell 2290 → 1767 (−23%), and the agentic-vs-plain ratio
at 100 rows went 2.1× → 1.8×, with the larger sizes inside run-to-run noise.

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
because absolute times move with whatever else the machine is doing — an
earlier printing of this table was taken with the machine saturated, and a
later run measured egui 20% faster with egui's code untouched. The stable,
load-independent quantity is the ratio *within* a run, so that is what the
optimization claims rest on.

These figures come from a quiet machine (~20% background load). The harness
reports a noise ratio per row — median over minimum — and it is 1.0× on every
row here except egui and iced at 5000 (1.1× and 1.3×), against swings of 3×
when this was first measured.

| Rows | Dewey        | Ontology off | Agentic  | On-demand pass | egui 0.31 | iced 0.13 |
| ---- | ------------ | ------------ | -------- | -------------- | --------- | --------- |
| 100  | **13.8 µs**  | 14.2 µs      | 27.6 µs  | 21.6 µs        | 108.3 µs  | 41.7 µs   |
| 1000 | **129.3 µs** | 130.3 µs     | 257.4 µs | 200.3 µs       | 1.01 ms   | 406.7 µs  |
| 5000 | **678.3 µs** | 681.3 µs     | 1.95 ms  | 1.52 ms        | 8.22 ms   | 3.40 ms   |

Speedup over Dewey with no agent ids:

| Rows | vs egui | vs iced |
| ---- | ------- | ------- |
| 100  | 7.8×    | 3.0×    |
| 1000 | 7.8×    | 3.1×    |
| 5000 | 12.1×   | 5.0×    |

iced's 5000-row frame is the least reproducible number in this table: two runs
minutes apart gave 3.40 ms and 4.74 ms, a 1.4× spread, where every other row
moved by under 10%. The table takes the faster one, so the ratio quoted against
iced is the smaller of the two.

## Against egui and iced directly

`cargo run --release --bin agent_surface`

**egui has an agent-readable surface, and an earlier version of these notes
said it did not.** With the `accesskit` feature every egui frame emits an
`accesskit::TreeUpdate` — roles, labels, bounds — and egui accepts an
`AccessKitActionRequest` back, so the loop closes without a screenshot. It is
standardised, predates this project, and is understood by every screen reader
on Windows, macOS and Linux. iced 0.13 has no accessibility feature, so there
an agent really does have only pixels.

The question is therefore not who has a structure. It is what a structure
designed for *screen readers* can express against one designed for agents.

### 1. One observation, 100 rows

| Framework | nodes | bytes | time |
| --------- | ----- | ----- | ---- |
| DeweyGUI, `process_request` (Value) | 202 | 40,036 | 479 us |
| egui + AccessKit | 301 | 77,104 | 264 us |
| **DeweyGUI, transport path** | 202 | 40,036 | **87 us** |
| iced 0.13 | — | — | — |

All three cover the whole path to something an agent could receive: produce it,
then serialise it.

**egui won this until the intermediate `serde_json::Value` came out of
`get_tree`.** Building that Value cost 379 us of the original 557, where
writing the same tree straight out as bytes costs 44 — so the transports no
longer build one. The first row is what an in-process caller gets, because it
wants a `Value` to inspect; the third is what a real agent over stdio or a
WebSocket receives.

egui is still doing more work per frame — it lays out and tessellates, where
DeweyGUI paints into a recording backend that draws nothing, and its tree falls
out of a frame it had to run anyway.

### 2. Naming the widget you want

An agent reads the tree, decides to act on the 18th row, and then one row is
filtered out of the list above it.

- **egui**: the node for `"item 17"` is `NodeId(9103526755343559150)`. After the
  filter, that id labels **`"item 18"`** — it followed the position, not the
  item. Of the 200 nodes that accept a `Click`, 100 carry any text at all; the
  checkboxes, which are what an agent would actually click, have no accessible
  name, so they cannot be found by text either — only by position in the tree,
  which is exactly what moved.
- **DeweyGUI**: the id is `"toggle_17"`, written by whoever wrote the view.
  `execute_action("toggle_17", "toggle")` changes the 18th row.

This is the same failure as a stale screen coordinate, at a structured API.

### 3. Action vocabulary

| Intent | DeweyGUI | AccessKit |
| ------ | -------- | --------- |
| sort a table by column 2, descending | `sort {column, direction}` | **no representation** |
| set a date field to 2026-09-01 | `set_date {year, month, day}` | `SetValue(string)`, format undocumented |
| set a colour to #204060 | `set_color {r,g,b,a \| hex}` | `SetValue(string)` |
| expand the tree node `root/a` | `expand {path}` | `Expand`, on that node if found |
| show page 2 of a table | `page {page}` | **no representation** |

AccessKit has 24 fixed actions for all software. That is the right design for a
screen reader, which must work with programs it has never seen — and it is why
two of five ordinary intents cannot be expressed and two more collapse into
`SetValue`.

### 4. Checking the interface is operable

| Fault | DeweyGUI | findable in AccessKit? |
| ----- | -------- | ---------------------- |
| button rendered with no id | `unaddressable_widget` | no — it assigns ids itself |
| two widgets share one id | `duplicate_agent_id` | no — generated ids cannot collide |
| widget laid out at zero size | `zero_size_widget` | yes — bounds are in the tree |
| widget wired for one of its actions | `unhandled_action` | no — nothing declares what it should support |

**The first two are not faults in egui because they cannot occur.** Generated
ids cannot be forgotten or duplicated; that is a real advantage, and it is the
direct price of the naming win in section 2. Neither framework ships a checker.

### Summary

egui beats this project on a class of authoring mistake it makes structurally
impossible, and its surface is a standard rather than one framework's
invention. It also beat it on observation cost until measuring this exposed a
`serde_json::Value` sitting in the middle of `get_tree` for no reason. DeweyGUI's ontology buys names an agent can be told in
advance instead of discovering by matching display text, per-widget actions with
typed parameters, a type catalogue to write against, and a check that what was
built can be operated at all.

## What the ontology buys, and what it does not

`cargo run --release --bin ontology`

The other benchmarks here measure how fast a frame is built. This one measures
what that cost is being spent *on*, and looks as hard for where the ontology
loses as for where it wins.

The baseline here is the same application driven from pixels and screen
coordinates, which is what an agent must do when no structure is available.
That is the honest baseline for iced 0.13 and *not* for egui, which has an
AccessKit tree — see `agent_surface` below for the direct comparison against
it. Both paths exist in this crate, so both run.

### 1. Seeing

| Rows | tree, all | | screenshot, one viewport | | **tree, one viewport** | | unchanged |
| ---- | ------- | ------- | ------- | ------- | ------- | ------- | ------ |
|      | time | bytes | time | bytes | time | bytes | bytes |
| 10   | 28.1 us | 4.5 kB | 979.5 us | 12.7 kB | **9.9 us** | 4.5 kB | 30 B |
| 100  | 368.7 us | 40.2 kB | 1.65 ms | 16.7 kB | **46.6 us** | 11.7 kB | 30 B |
| 1000 | 7.48 ms | 401.5 kB | 1.50 ms | 16.7 kB | **288.5 us** | 11.7 kB | 30 B |

The middle pair grows the window until every row is drawn, which flatters the
tree. A real screenshot is the third pair: one viewport, near constant however
long the list, because it shows only what is on screen. The tree describes
every widget including the ones nobody can see.

Given the same viewport the screenshot gets, the tree is **5x faster and 30%
smaller** at 1000 rows. It was not always: the unclipped tree describes every
widget including the ones nobody can see, and at 1000 rows that was 3.7x slower
and 24x bigger than a picture. The viewport now decides *before* a `UiNode` is
built rather than clipping a finished tree, which took the clipped 1000-row
read from 971 us to 288 us.

What remains: the clipped time still grows with the list, because an
off-screen widget is still laid out and still painted — only the description is
skipped. A list long enough for that to matter wants `VirtualList` in the view.
A re-poll of an unchanged screen costs 100 ns and 30 bytes at every size.

### 2. Acting

An agent observes the position of `toggle_17`, then one row is inserted above
the list — nothing added or removed, every row simply one row lower — and both
agents act.

| Path | reported | actually changed |
| ---- | -------- | ---------------- |
| click at the observed coordinate | success | `toggle_16` — the wrong row |
| `execute_action("toggle_17")` | success | `toggle_17` — correct |

The two cost the same: 25.6 us by id against 25.2 us by coordinate. The
ontology buys nothing on speed here. What it buys is that one of them was still
correct after the screen moved — and note that **neither reported otherwise**.
The coordinate click was a success by every measure the agent could see.

### 3. Verifying

Six interfaces, each broken in a way that compiles and renders.

| Fault | `validate` | screenshot |
| ----- | ---------- | ---------- |
| button rendered with no id | `unaddressable_widget` | — |
| two widgets share one id | `duplicate_agent_id` | — |
| widget laid out at zero size | `zero_size_widget` | visible |
| widget laid out off screen | `offscreen_widget` | visible |
| wired for `select_row`, not `sort` | `unhandled_action` | — |
| white text on white ground | — | visible |

5 of 6 against 3 of 6. The last row is the one worth dwelling on: white on
white is structurally perfect — correct id, real bounds, on screen, fully
wired — and `validate` passes it. The ontology says nothing about appearance
and cannot substitute for looking.

### 4. Paying

| Rows | plain frame | on demand | every frame | penalty |
| ---- | ----------- | --------- | ----------- | ------- |
| 10   | 3.1 us   | 0.2 ms/s  | 0.3 ms/s  | 1.58x |
| 100  | 24.5 us  | 1.7 ms/s  | 2.6 ms/s  | 1.80x |
| 1000 | 232.2 us | 16.2 ms/s | 27.5 ms/s | 1.97x |

Amortized at 60 fps against 5 agent queries a second. With no agent attached
the cost is not reduced — it is not incurred, because `ontology_enabled()` is
checked before a node is built rather than before it is registered.

### Summary

**Buys:** an action that still reaches the right widget after the screen moves;
a re-poll costing 100 ns and 30 bytes instead of a render; three structural
faults out of six that no screenshot would show.

**Does not buy:** anything about appearance; any speed advantage per action;
scale past one screenful, where an unpaginated tree loses to a screenshot on
both time and size; and it is not free — a tree-building frame is 1.6-2.0x a
plain one.

### On-demand ontology, amortized

`OntologyMode::OnDemand` (the default) does not build the tree during a frame.
It builds one on the next agent query, by running a paint-free `view` pass —
the *On-demand pass* column above. A UI redrawing at 60 fps with an agent
querying 5 times a second therefore pays:

| Rows | Every frame  | On demand   | Saving   |
| ---- | ------------ | ----------- | -------- |
| 100  | 1.7 ms/s     | 0.9 ms/s    | 1.8×     |
| 1000 | 15.4 ms/s    | 8.8 ms/s    | 1.8×     |
| 5000 | 116.8 ms/s   | 48.3 ms/s   | 2.4×     |

Two consecutive runs agreed to one decimal place on all three. Earlier
printings read 1.9× / 1.9× / 3.0× and 1.8× / 1.8× / 4.1× — the 5000-row saving
is the one that moves, because it is dominated by the same iced-scale
allocation noise the row above it shows. The saving grows with widget count and
with the gap between frame rate and query rate: the tree was being built 60
times a second to be read 5 times.

Because `Model::view` takes `&self`, the extra pass has no side effects, and
the tree an agent reads is built from current model state rather than from
whatever the last painted frame happened to contain.

Two runs on the quiet machine agreed to within 0.1× on every ratio against
egui (7.7×/7.8×, 7.6×/7.8×, 12.3×/12.1×).

### The agentic path, measured as a within-run ratio

How much an agent-driven UI costs over the same UI with no agent ids — the
number the optimization work was aimed at, and the one that does not care how
loaded the machine is:

| Rows | After borrowed ids + gate | After borrowed keys | After moved values |
| ---- | ------------------------- | ------------------- | ------------------ |
| 100  | 3.0×                      | 2.3×                | **2.1×**           |
| 1000 | 4.2×                      | 2.7×                | **2.1×**           |
| 5000 | 9.1×                      | 6.2×                | **3.8–5.7×**       |

And with `OntologyMode::OnDemand` a rendered frame pays none of it at all —
the ratio applies only to the occasional query pass.

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
