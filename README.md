# Dewey — Agentic-First GUI Framework

**Dewey** is a backend-agnostic Rust GUI framework with a pluggable `Painter` rendering abstraction and optional GPU-accelerated rendering via [egui](https://github.com/emilk/egui)/[wgpu](https://wgpu.rs). It provides a complete **semantic ontology** over every UI widget. AI agents can discover, inspect, and control graphical applications through a structured JSON Lines protocol — no screen-scraping, no pixel matching, no accessibility-tree hacking.

> Dewey does for GUIs what [Louie](https://github.com/nervosys/louie) does for TUIs.
>
> Copyright (c) [NERVOSYS](https://nervosys.ai). All rights reserved.

## Features

### Core Architecture

- **Elm Architecture** — Immutable model, message-driven updates, pure view functions
- **29 Widgets** — Button, Label, Input, TextArea, Table, Tree, Menu, Modal, Canvas, ColorPicker, Toolbar, Splitter, CommandPalette, VirtualList, Chart, DatePicker, RichText, and more
- **Backend-Agnostic Rendering** — Abstract `Painter` trait with pluggable backends (egui/wgpu, web/wasm32, software rasterizer, test/headless)
- **Full Semantic Ontology** — Every widget exposes its schema, capabilities, actions, and semantic role
- **Layout Engine** — Constraint-based layout with flex distribution
- **Focus Management** — Ring-buffer tab navigation
- **Overlay System** — Modal dialogs and overlay stacking

### Agent & AI Integration

- **Agent Protocol** — JSON Lines over stdin/stdout with batch actions, protocol negotiation, and state-diff subscriptions
- **WebSocket Transport** — Same agent protocol over WebSocket for remote and cross-language agent control
- **Headless Driver** — Run and test apps without a GPU or display

### Rendering Backends (6)

- **GPU Backend** — egui/wgpu hardware-accelerated rendering
- **agpu Backend** — Vulkan-first wgpu/winit backend with complete GPU resource abstraction and full ontology (`agpu` crate)
- **Web Backend** — `WebPainter` targeting wasm32/Canvas 2D for browser deployment
- **Software Rasterizer** — `ImagePainter` for CPU-only rendering and screenshot generation
- **Test Backend** — `TestBackend` records every draw call for assertion; zero GPU required
- **Null Backend** — `NullPainter` for headless/CI environments

### Platform Integration

- **Cross-Platform** — Windows, macOS, Linux, and Web (wasm32)
- **Window control** — `Command::SetWindowVisible`, `FocusWindow`, `MinimiseWindow`,
  `SetWindowPosition`, `SetWindowSize`, `SetAlwaysOnTop`, `SetFullscreen`,
  `SetWindowTitle`, honoured by both backends
- **System Tray** — *types only.* `TrayBackend` is a trait to implement, with
  `TrayConfig`, `TrayMenuItem`, `TrayEvent` and `TrayIconImage`. No platform
  backend ships, and the runtime neither creates nor polls one
- **Native File Dialogs** — *types only.* `DialogBackend` is a trait to
  implement; no platform backend ships
- **Multi-Window** — `WindowManager` tracks windows in memory. It does not
  create or raise real ones; use the window commands above for that
- **Drag & Drop** — Files dropped onto the window reach `handle_event` on both
  backends, naming the widget they landed on. Widget-to-widget dragging is a
  typed pipeline (`DragPayload`) that an application drives itself

### Theming & Styling

- **Token-Based Themes** — Built-in dark and light presets with full JSON serialization
- **Theme Hot-Reload** — `ThemeWatcher` monitors theme files and live-reloads on change
- **Accessibility** — ARIA-like semantic roles on every UI node for screen readers and assistive agents

### Performance & Optimization

- **GPU Render Batching** — `RenderBatch` with automatic quad merging and draw-call optimization
- **Arena Allocator** — Bump allocator for zero-fragmentation per-frame allocation
- **Buffer Pooling** — `VecPool` for reusable buffer allocation with zero syscall overhead
- **Built-in Profiler** — `Profiler` with per-frame timing, FPS tracking, widget counting, and history
- **Animation Engine** — 34 easing functions, tweens, spring physics, timelines, keyframe sequences

### Extensibility

- **Plugin System** — `Plugin` trait with lifecycle hooks (`init`, `on_frame`, `on_shutdown`) and `PluginRegistry`
- **Internationalization** — `I18n` framework with locale fallback chains, message catalogs, and `t_fmt()` interpolation

### Data & State

- **Data-Bound Table** — Sorting, filtering, and pagination built into the Table widget
- **State Persistence** — Save/restore model state to disk as JSON
- **Virtual Scrolling** — Efficient rendering for lists with thousands of items
- **Async Tasks** — Cancellable tasks with timeout support

## Quick Start

Add Dewey to your `Cargo.toml`:

```toml
[dependencies]
dewey = "1"
```

```rust
use dewey::prelude::*;

struct App { count: i32 }

#[derive(Debug)]
enum Msg { Increment, Decrement }

impl Model for App {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) -> Command<Msg> {
        match msg {
            Msg::Increment => self.count += 1,
            Msg::Decrement => self.count -= 1,
        }
        Command::None
    }

    fn view(&self, frame: &mut Frame<'_>) {
        let area = frame.area;
        Label::new(format!("Count: {}", self.count)).render(area, frame);
    }

    fn handle_event(&self, _event: Event) -> Option<Msg> { None }
}

fn main() -> Result<(), eframe::Error> {
    Program::new(App { count: 0 }).run()
}
```

### Using agpu (Vulkan-First GPU Backend)

For a standalone Vulkan-first GPU backend without egui:

```toml
[dependencies]
dewey = { version = "1", default-features = false, features = ["agpu-backend"] }
```

```rust
use dewey::backend::agpu_backend::AgpuProgram;
use dewey::prelude::*;

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    AgpuProgram::new(App { count: 0 }).run()
}
```

## Agent Protocol

Dewey speaks JSON Lines over stdin/stdout. Any language that can read/write lines of JSON can drive the UI:

```json
{"id":1,"request":"QueryOntology"}
{"id":2,"request":{"GetWidgetState":{"agent_id":"counter_label"}}}
{"id":3,"request":{"PerformAction":{"agent_id":"increment_btn","action":"click","params":{}}}}
{"id":4,"request":"ListActions"}
{"id":5,"request":"Screenshot"}
```

## Architecture

```
┌──────────┐   Event   ┌──────────┐  Command  ┌──────────┐
│  Backend │ ────────→ │  Model   │ ────────→ │ Runtime  │
│(Painter) │           │ (update) │           │ (Program)│
│          │ ←──────── │  (view)  │ ←──────── │          │
└──────────┘   Frame   └──────────┘           └──────────┘
      ↕                      ↕
┌──────────┐         ┌──────────────┐
│  Widgets │         │  Ontology    │
│  (30)    │         │  (Registry)  │
└──────────┘         └──────────────┘
                             ↕
                       ┌──────────┐
                       │  Agent   │
                       │ Protocol │
                       └──────────┘
```

## Widget Inventory

| Widget         | Traits               | Semantic Role     | Agent Actions                      |
| -------------- | -------------------- | ----------------- | ---------------------------------- |
| Label          | Widget, Discoverable | Display           | —                                  |
| Button         | Widget, Discoverable | Action            | click                              |
| Input          | StatefulWidget       | Input             | set_text, clear                    |
| Checkbox       | Widget, Discoverable | Selection         | toggle                             |
| Radio          | Widget, Discoverable | Selection         | select                             |
| Slider         | StatefulWidget       | Input             | set_value                          |
| Progress       | Widget, Discoverable | Progress          | —                                  |
| Container      | Widget, Discoverable | Container         | —                                  |
| Panel          | Widget, Discoverable | Container         | —                                  |
| Scroll         | StatefulWidget       | Scrollable        | scroll_to                          |
| List           | StatefulWidget       | Selection         | select, next, previous             |
| Table          | StatefulWidget       | DataVisualization | select_row                         |
| Tabs           | StatefulWidget       | Tab               | select_tab                         |
| TextArea       | StatefulWidget       | Input             | set_text, insert_text              |
| Select         | StatefulWidget       | Selection         | select                             |
| Tree           | Widget, Discoverable | TreeNode          | expand, collapse                   |
| Menu           | Widget, Discoverable | Menu              | select_item                        |
| Modal          | Widget, Discoverable | Modal             | open, close                        |
| Tooltip        | Widget, Discoverable | Display           | —                                  |
| Image          | Widget, Discoverable | Media             | —                                  |
| Canvas         | Widget, Discoverable | Canvas            | draw, clear                        |
| ColorPicker    | StatefulWidget       | Input             | set_color, get_color               |
| Toolbar        | Widget, Discoverable | Toolbar           | click_item, list_items             |
| Splitter       | StatefulWidget       | Container         | set_ratio                          |
| CommandPalette | StatefulWidget       | Navigation        | search, execute, open, close, list |
| VirtualList    | StatefulWidget       | Scrollable        | scroll_to, get_visible_range       |
| Chart          | Widget, Discoverable | DataVisualization | —                                  |
| DatePicker     | StatefulWidget       | Input             | set_date, get_date                 |
| RichText       | Widget, Discoverable | Display           | —                                  |

## Competitive Landscape

### Feature Comparison Matrix

| Feature                 | Dewey                        | egui              | Iced              | Slint                  | GTK 4            | Qt 6                          | Flutter                  | Electron         | Tauri                | Avalonia                 |
| ----------------------- | ---------------------------- | ----------------- | ----------------- | ---------------------- | ---------------- | ----------------------------- | ------------------------ | ---------------- | -------------------- | ------------------------ |
| **Language**            | Rust                         | Rust              | Rust              | Rust/Slint             | C/Rust           | C++/Python                    | Dart                     | JS/TS            | Rust+JS              | C#/XAML                  |
| **Rendering**           | Painter (6 backends)         | wgpu/glow         | wgpu              | Femtovg/Skia/Software  | Cairo/Vulkan     | Custom/RHI                    | Impeller/Skia            | Chromium         | WebView              | Skia                     |
| **Architecture**        | Elm (Model/Msg/Cmd)          | Immediate         | Elm               | Declarative            | OOP/Signals      | OOP/Signals                   | Reactive                 | Component        | Component            | MVVM                     |
| **Agent Protocol**      | ✅ JSON Lines + WebSocket     | ❌                 | ❌                 | ❌                      | ❌                | ❌                             | ❌                        | ❌                | ❌                    | ❌                        |
| **Semantic Ontology**   | ✅ Full                       | ❌                 | ❌                 | ❌                      | ❌                | ❌                             | ❌                        | ❌                | ❌                    | ❌                        |
| **Headless Testing**    | ✅ Built-in (TestBackend)     | ❌                 | Partial           | Partial                | ❌                | ✅ QTest                       | ✅ Flutter Test           | ✅ Playwright     | ✅ WebDriver          | ❌                        |
| **Widget Count**        | 30                           | ~25               | ~20               | ~25                    | ~80              | ~100+                         | ~170                     | Unlimited (HTML) | Unlimited (HTML)     | ~60                      |
| **Accessibility**       | ✅ Semantic roles             | ✅ AccessKit       | ✅ AccessKit       | ✅                      | ✅ ATK/AT-SPI     | ✅                             | ✅ SemanticsNode          | ✅ ARIA           | ✅ ARIA               | ✅ UIA                    |
| **Theming**             | ✅ Token-based + JSON         | ✅ Visuals         | ✅                 | ✅                      | ✅ CSS-like       | ✅ QSS                         | ✅ ThemeData              | ✅ CSS            | ✅ CSS                | ✅ Styles                 |
| **Hot Reload**          | ✅ Theme hot-reload           | ❌                 | ❌                 | ✅                      | ❌                | ✅ QML                         | ✅                        | ✅ HMR            | ✅ HMR                | ✅ XAML                   |
| **Animation**           | ✅ 34 easings + spring        | Basic             | ✅                 | ✅                      | ✅                | ✅ QPropertyAnimation          | ✅                        | ✅ CSS/JS         | ✅ CSS/JS             | ✅                        |
| **Layout Engine**       | Constraint-based             | Manual rects      | Flexbox-like      | Grid/Box               | Box/Grid/Custom  | Box/Grid/Form                 | Flex/Stack/Custom        | CSS Flexbox/Grid | CSS Flexbox/Grid     | Panel/Grid/Stack         |
| **Plugin System**       | ✅ Plugin trait + registry    | ❌                 | ❌                 | ❌                      | ❌                | ✅ QPlugin                     | ✅ Packages               | ✅ npm            | ✅ npm                | ❌                        |
| **i18n / Localization** | ✅ Built-in (I18n)            | ❌                 | ❌                 | ✅                      | ✅ gettext        | ✅ Qt Linguist                 | ✅ intl                   | ✅ i18next        | ✅ i18next            | ❌                        |
| **Multi-Window**        | ⚠️ Bookkeeping only           | ✅ Viewports       | ❌                 | ✅                      | ✅                | ✅                             | ✅                        | ✅                | ✅                    | ✅                        |
| **System Tray**         | ⚠️ Trait, no backend          | ❌                 | ❌                 | ❌                      | ✅                | ✅                             | ❌ (plugin)               | ✅                | ✅                    | ❌                        |
| **Native Dialogs**      | ⚠️ Trait, no backend          | ❌ (rfd crate)     | ❌                 | ❌                      | ✅                | ✅                             | ❌ (plugin)               | ✅                | ✅                    | ✅                        |
| **Drag & Drop**         | ✅ Typed payloads             | ✅ Basic           | ❌                 | ❌                      | ✅                | ✅                             | ✅                        | ✅                | ✅                    | ✅                        |
| **GPU Render Batching** | ✅ Automatic quad merging     | ✅                 | ✅                 | ✅                      | ✅                | ✅                             | ✅                        | ✅                | N/A                  | ✅                        |
| **Built-in Profiler**   | ✅ Per-frame + FPS + history  | ❌                 | ❌                 | ❌                      | ❌                | ❌                             | ✅ DevTools               | ✅ DevTools       | ❌                    | ❌                        |
| **Memory Optimization** | ✅ Arena + VecPool            | ❌                 | ❌                 | ❌                      | ❌                | ❌                             | ❌                        | ❌                | ❌                    | ❌                        |
| **Software Rasterizer** | ✅ ImagePainter               | ❌                 | ❌                 | ✅                      | ❌                | ✅                             | ❌                        | ❌                | ❌                    | ✅                        |
| **State Persistence**   | ✅ JSON serde                 | ❌ Manual          | ❌ Manual          | ❌                      | ❌                | ✅ QSettings                   | ✅ SharedPrefs            | ✅ localStorage   | ✅ Various            | ✅                        |
| **Cross-Platform**      | Win/Mac/Linux/Web            | Win/Mac/Linux/Web | Win/Mac/Linux/Web | Win/Mac/Linux/Embedded | Win/Mac/Linux    | Win/Mac/Linux/Mobile/Embedded | Win/Mac/Linux/Web/Mobile | Win/Mac/Linux    | Win/Mac/Linux/Mobile | Win/Mac/Linux/Web/Mobile |
| **Binary Size**         | ~3 MB                        | ~2 MB             | ~5 MB             | ~2 MB                  | ~20 MB (runtime) | ~30 MB (runtime)              | ~10 MB                   | ~150 MB          | ~5 MB                | ~15 MB                   |
| **Memory Usage**        | Very Low                     | Low               | Low               | Low                    | Medium           | High                          | Medium                   | Very High        | Medium               | Medium                   |
| **Backend-Agnostic**    | ✅ Painter trait (6 backends) | ❌                 | ❌                 | ✅                      | ❌                | ✅                             | ❌                        | ❌                | ❌                    | ✅                        |
| **License**             | AGPLv3/Commercial            | MIT/Apache        | MIT               | GPL/Commercial         | LGPL             | GPL/Commercial                | BSD                      | MIT              | MIT/Apache           | MIT                      |

### Measured Performance

CPU frame-build cost — widget construction, layout, and render-command
generation for a list of N rows, each with a label and a button, running
headless with no GPU. Fastest observed frame of 400/120/40 interleaved rounds.
Full methodology and caveats in
[`benches/comparative/`](benches/comparative/README.md).

| Rows | Dewey        | Dewey, agentic | egui 0.31 | iced 0.13 | vs egui | vs iced |
| ---- | ------------ | -------------- | --------- | --------- | ------- | ------- |
| 100  | **13.8 µs**  | 27.6 µs        | 108.3 µs  | 41.7 µs   | 7.8×    | 3.0×    |
| 1000 | **129.3 µs** | 257.4 µs       | 1.01 ms   | 406.7 µs  | 7.8×    | 3.1×    |
| 5000 | **678.3 µs** | 1.95 ms        | 8.22 ms   | 3.40 ms   | 12.1×   | 5.0×    |

Re-measured on a quiet machine (~20% background load, run-to-run spread at or
under 1.1× on every row except iced's largest). Earlier printings of this table
were taken while the machine was saturated and read 8.4× and 12.9× against
egui, and 7.0× against iced at 5000 rows. iced's 5000-row time is the least
stable figure here, spanning 3.40–4.74 ms across runs; the ratio above uses the
run most favourable to iced.

Reproduce with `cd benches/comparative && cargo run --release --bin timing`.

The *agentic* column shows what building the ontology costs when it happens.
By default it no longer happens per frame: `OntologyMode::OnDemand` builds the
tree on the next agent query instead, via a paint-free `view` pass. A UI at
60 fps queried 5 times a second spends 1.8× less at 1000 rows and 2.4× less at
5000 rows than building it every frame — and because `Model::view` takes
`&self`, the tree an agent reads is built from current state rather than from
the last painted frame. egui and iced have no equivalent to build, so the
like-for-like comparison is the first column. Text shaping is a second
asymmetry — Dewey estimates text extents during frame build and shapes in the
backend, where egui and iced shape inline (~10% of egui's frame). Tessellation,
rasterization, and present are excluded for all three.

### Agent Scaffolding and Verification

Frame time is only half of what an agent-first framework should be judged on.
The other half is what an agent pays to *build* a program and then confirm it
works. Full methodology in [`benches/scaffold/`](benches/scaffold/README.md).

Writing the same app in each framework — a counter, and TodoMVC as the complex
case (input, filters, dynamic list, per-item toggle and delete, live count):

| Framework     | counter ~tokens | todomvc ~tokens | todomvc vs egui |
| ------------- | --------------- | --------------- | --------------- |
| Dewey (plain) | 328             | 1047            | 1.63×           |
| Dewey (agent) | 335             | 1110            | 1.73×           |
| **egui 0.31** | **264**         | **643**         | 1.00×           |
| iced 0.13     | 268             | 788             | 1.23×           |

On the counter Dewey is now the same 33 lines as egui, and on TodoMVC it is
shorter than iced (97 lines against 110).

**Dewey still costs more to scaffold, but much less than it did, and
agent-driveability is nearly free.** Three changes did it:

- `Button::action(id, msg)` wires a widget for a person *and* an agent in one
  call, so the premium for being agent-driveable fell from **+36% to +2%** on
  the counter and +37% to +13% on TodoMVC.
- `Button::on(id, |model| ...)` lets a widget carry the change it makes rather
  than a message, so an application needs no `Msg` variants at all — the Elm
  loop is the right shape for real state transitions and the wrong shape for a
  button that adds one to a number. `action` remains for changes returning a
  `Command`.
- `Rect::rows_of` / `split_columns` replace a `Layout` and a named `Constraint`
  per band.
- `TextInput::on_input`, `Slider::on_change` and the same on every other
  widget carrying a value, each bound to the action its ontology advertises.
  The TodoMVC sample now has no `execute_action` handler at all and the premium
  for being agent-driveable there is **+6%**.
- Widgets advertising several actions take one handler for all of them, passed
  a typed change rather than raw JSON: `Tree::on_change` receives a
  `TreeChange`, `Table::on_change` a `TableChange`, and so on through
  `DatePicker`, `CommandPalette`, `ColorPicker`, `Modal`, `Chart` and
  `RichText`. **Every widget in the library that advertises a mutating action
  now has a builder that answers it**, and `validate` reports a widget wired
  for only some of what it publishes.

Together those took the counter from 1.49× to **1.24×** egui's tokens and
TodoMVC from 1.86× to **1.63×**.

What the ontology changes about *writing* a GUI is measured separately, in
`benches/scaffold/src/bin/dev_loop.rs`. It does not make an agent write less
code. What it changes is how the agent finds out it was wrong: five authoring
mistakes that compile, render and look correct — a button with no id, an id
copy-pasted onto two widgets, layout arithmetic that leaves a widget no room, a
widget positioned past the edge, a widget wired for one of its actions — are all
caught by `validate` in single-digit microseconds, and none of them by the
compiler. The two sets do not overlap: types check that a call is well formed,
`validate` checks that the interface the call builds can be operated. An agent
looking up one widget reads **377 tokens** against 11,194 for the equivalent
documentation, and can ask a question — *which widget answers to "dropdown"* —
that prose cannot be asked at all.

Verifying it afterwards is where that inverts. Dewey closes the full
discover → act → verify loop over the agent protocol headlessly — no window,
no GPU, no screenshot. A nine-step TodoMVC task (add two items, complete one,
switch filter, read the result back):

| Task                                            | Time        | Rate      |
| ----------------------------------------------- | ----------- | --------- |
| counter: discover → act → verify → validate     | **12.6 µs** | 79,000/s  |
| polling an unchanged screen (`get_tree since=`) | **100 ns**  | 50× less  |
| todomvc: 9-step add/complete/filter/verify      | **45.2 µs** | 22,000/s  |
| session setup: `query_ontology`, 29 widget types | **500 ns**  | once      |
| one screen of a 1000-row list (`viewport`)      | **488 µs**  | 11.7 kB   |

`query_ontology` reads the whole widget catalogue, which never changes, so a
transport serves it from bytes serialised once for the process rather than
deep-cloning and re-serialising a `serde_json::Value` per caller — 36.7 µs to
500 ns. An in-process caller that asks for a `Value` still pays 54 µs to have
one built, which is the cost of what it asked for rather than waste. The counter figure previously read 8.2 µs and included a
`query_ontology` step. That step was returning an empty catalogue — schemas were registered
only by an application that chose to, so for an ordinary program there was
nothing to return and the call cost nothing. Reading the catalogue is now
real, and it is session setup rather than part of a loop: an agent learns
what a `Button` is once and then works.

`benches/comparative/src/bin/ontology.rs` measures this against the only real
alternative — the same application driven from pixels and coordinates — and
reports where the ontology loses as well as where it wins. Two findings worth
stating up front. **A coordinate captured one observation earlier toggled the
wrong row after a single line was inserted above the list, and reported
success**; the id was still correct, and the two cost the same, so what the
ontology buys there is correctness rather than speed. And the tree used to
describe every widget including the ones nobody could see, which at 1000 rows
made it 3.7× slower and 24× larger than a screenshot — `get_tree` now takes a
`viewport`, and decides before building a node rather than clipping a finished
tree, so the same read is **3.5× faster and 30% smaller** than the picture. What
it still does not do is stop laying out and painting the widgets it declines to
describe, so the time grows with the list even though the reply does not.

An agent can also ask whether the interface it just built is *operable*, which
a screenshot cannot tell it:

```jsonc
-> {"type": "validate"}
<- {"ok": false, "errors": 1, "diagnostics": [
     {"severity": "error", "code": "unaddressable_widget", "widget_type": "Button",
      "message": "1 `Button` widget(s) rendered without an id, so they are not
                  hit-testable and no agent can act on them; give each one
                  `.action(id, msg)`"}]}
```

It can also keep a golden snapshot and diff against it, rather than comparing
pixels or re-reading fields it does not care about:

```
root
  Label #count [0,0 400x40] text="Count: 1"
  Button #inc [268,40 132x40] enabled=true label="+ Increment"
```

Two renders of one interface produce byte-identical text, so a diff shows
exactly what moved.

`validate` catches faults that render perfectly: id-less widgets that cannot be
clicked, duplicate ids that make an action ambiguous, and zero-size or offscreen
bounds. The first of those is not hypothetical — it was made while writing this
project's own benchmarks, where a button looked right and was simply dead.

**egui does have an equivalent, and an earlier version of this section said it
did not.** With the `accesskit` feature, every egui frame emits an
`accesskit::TreeUpdate` — roles, labels, bounds — and egui accepts an
`AccessKitActionRequest` back, so an agent can read and act without a
screenshot. It is standardised, predates this project, and is understood by
every screen reader on three platforms. `benches/comparative/src/bin/agent_surface.rs`
measures the two against each other. egui wins on one axis: its generated node
ids make an unaddressable or duplicated widget impossible to write, where an
author who names widgets can name one wrong. It won on observation cost too —
366 µs against 557 µs — until that measurement exposed an intermediate
`serde_json::Value` in `get_tree` that cost 379 µs of the 557 and bought
nothing a transport wanted; the same reply now costs **87 µs**.

Where it loses is what a screen-reader tree is *for*. AccessKit node ids are
opaque hashes; in the measured case, filtering one row out of a list left the
id an agent had captured pointing at the **next row down**, the same failure as
a stale coordinate. The clickable widgets in that list — the checkboxes — carry
no accessible name at all, so they can only be located by tree position, which
is what moved. And AccessKit's action vocabulary is 24 fixed verbs for all
software: of five ordinary intents (sort by a column, set a date, set a colour,
expand a path, go to page 2) two cannot be expressed at all and two collapse
into `SetValue` with a string whose format is undocumented.

iced 0.13 has no accessibility feature, so there an agent really does have only
pixels.

Since that comparison was written, DeweyGUI publishes an AccessKit tree of its
own: `features = ["accesskit"]` mirrors every addressable widget into the
platform accessibility API, so a screen reader — and any harness that already
speaks AccessKit — sees the same interface an agent does, without learning this
protocol. The bridge had existed for some time and nothing called it, which
meant a Dewey application was unusable with a screen reader.

So: Dewey is worse at being *written* by an agent, and uniquely good at being
*verified* by one. Verification cost is paid on every iteration, and an agent
that cannot check its work reliably does more iterations.

### Frame allocation cost

Heap traffic per row (one `Label` + one `Button`), measured with a counting
allocator via `cargo run --release --bin allocs` — deterministic, unlike wall
clock:

| Configuration           | Allocations/row | Bytes/row       |
| ----------------------- | --------------- | --------------- |
| No agent ids            | 4.0             | 500             |
| Agent ids, ontology on  | 18.0 → **6.0**  | 3210 → **1767** |
| Agent ids, ontology off | 18.0 → **4.0**  | 3210 → **598**  |

Building the agent ontology is the dominant per-frame cost in an agentic UI.
Four changes cut it by 67%:

- `UiNode` ids and widget-type names are `Cow<'static, str>` rather than
  freshly allocated `String`s.
- `UiNode::state` is a flat `Properties` vector with borrowed keys, not a
  `serde_json` map that allocated a `String` per key per frame.
- Widgets build their node at the end of `render`, so owned values *move* into
  the state instead of being cloned — for `List`, `Select`, and `Tabs` that
  turns one allocation per item per frame into zero.
- `ProgramOptions::ontology` (an `OntologyMode`) decides when the tree is
  built: `OnDemand` by default, or `EveryFrame`, or `Disabled` for an
  application no agent will ever drive — the latter two measuring within a few
  percent of a UI with no agent ids at all.

- `UiNode` shrank from 304 to 176 bytes: `Accessibility` is 136 bytes of
  mostly-`None` options that was stored inline in every node although no
  widget sets it, and is now boxed behind `UiNode::accessibility()`.

Hit-testing and painting are unaffected by all of it, and the agent wire
format is unchanged throughout.

### Dewey's Unique Advantages

1. **Agent-native** — The only GUI framework with a built-in semantic protocol for AI agents (JSON Lines + WebSocket)
2. **Full ontology** — Every widget exposes structured schema, capabilities, typed actions, and semantic roles — no screen-scraping or accessibility-tree hacking
3. **6 rendering backends** — GPU (egui/wgpu), agpu (Vulkan-first), Web (wasm32/Canvas 2D), software rasterizer (`ImagePainter`), test (`TestBackend`), and null — swap backends without touching widget code
4. **Headless-first testing** — `TestBackend` records every draw call for assertion; zero GPU, zero display required
5. **Elm architecture** — Predictable state management with immutable models and message-driven updates
6. **Memory-optimized** — Arena bump allocator and `VecPool` buffer reuse eliminate per-frame allocation overhead
7. **Built-in profiler** — Per-frame timing, FPS tracking, widget counting, and configurable history — no external tools needed
8. **Plugin-extensible** — `Plugin` trait with full lifecycle hooks and `PluginRegistry` for modular architecture
9. **i18n-ready** — Built-in `I18n` framework with locale fallback chains and message catalogs — no third-party crate required
10. **Window control from the model** — show, hide, focus, move, resize and
    retitle the window by returning a `Command`, on both backends. Tray and
    native dialogs are traits to implement against, not implementations

\* Electron/Tauri support agent interaction only through fragile accessibility trees or DOM scraping.

## Examples

```bash
cargo run --example hello               # Minimal window
cargo run --example counter             # Interactive counter (Elm architecture)
cargo run --example agent_demo          # Agent protocol demo
cargo run --example showcase            # Widget showcase gallery
cargo run --example canvas_drawing      # Canvas drawing demo
cargo run --example ontology_explorer   # Headless ontology discovery
cargo run --example agent_headless      # Full headless agent session
cargo run --example chat                # Chat interface demo
cargo run --example counter_agpu --features agpu-backend --no-default-features  # Counter using agpu GPU backend
```

## Known issues

### `Unrecognized present mode 1000361000` on recent GPU drivers

Every Dewey application on a current NVIDIA driver logs a warning per surface
configuration, in release as well as debug:

```
WARN wgpu_hal::vulkan::conv] Unrecognized present mode 1000361000
```

Nothing is wrong. The driver advertises `VK_PRESENT_MODE_FIFO_LATEST_READY_KHR`,
which postdates the Vulkan headers `wgpu-hal 24` was built against, and wgpu
falls back to FIFO correctly. wgpu 30 demoted the same line to `log::debug!`.

Silence it in the meantime:

```rust
env_logger::builder().parse_filters("info,wgpu_hal=off").init();
```

**Why it is not simply fixed.** Moving the `eframe` pin from 0.31 to 0.36 does
resolve it — that pulls `wgpu-hal 30.0.1`, which is the version with the fix.
But the `agpu` sibling crate pins `wgpu 24`, and the two cannot coexist: both
depend on `gpu-allocator`, `wgpu-hal 24` requires `windows 0.58` and
`wgpu-hal 30` requires `windows 0.62`, and the resolver hands one of them an
allocator built against the other's bindings. The build fails inside
`wgpu-hal`, not in this crate.

So the pin moves when `agpu` moves, which is six major wgpu versions and 263
call sites — its own piece of work rather than a version bump. Reported
downstream by the Tabinator build as finding 7, and left open deliberately.

## License

AGPL-3.0-or-later — free for open-source use. Commercial licenses are available from [NERVOSYS](https://nervosys.ai) for proprietary/closed-source applications.
