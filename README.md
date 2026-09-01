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
- **Multi-Window** — `WindowManager` for opening, closing, focusing, and managing multiple windows
- **System Tray** — `TrayBackend` trait with configurable menus, tooltips, and tray icons
- **Native File Dialogs** — `DialogBackend` for open/save dialogs and message boxes
- **Drag & Drop** — Full drag-and-drop event pipeline with typed payloads (files, text, custom data)

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
| **Multi-Window**        | ✅ WindowManager              | ✅ Viewports       | ❌                 | ✅                      | ✅                | ✅                             | ✅                        | ✅                | ✅                    | ✅                        |
| **System Tray**         | ✅ TrayBackend                | ❌                 | ❌                 | ❌                      | ✅                | ✅                             | ❌ (plugin)               | ✅                | ✅                    | ❌                        |
| **Native Dialogs**      | ✅ DialogBackend              | ❌ (rfd crate)     | ❌                 | ❌                      | ✅                | ✅                             | ❌ (plugin)               | ✅                | ✅                    | ✅                        |
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
headless with no GPU. Fastest observed frame of 400/120/40 interleaved rounds,
best of two runs. Full methodology and caveats in
[`benches/comparative/`](benches/comparative/README.md).

| Rows | Dewey        | Dewey, agentic | egui 0.31 | iced 0.13 | vs egui | vs iced |
| ---- | ------------ | -------------- | --------- | --------- | ------- | ------- |
| 100  | **14.5 µs**  | 30.6 µs        | 115.1 µs  | 43.9 µs   | 7.9×    | 3.0×    |
| 1000 | **135.0 µs** | 293.3 µs       | 1.13 ms   | 436.5 µs  | 8.4×    | 3.2×    |
| 5000 | **736.3 µs** | 2.78 ms        | 9.48 ms   | 5.17 ms   | 12.9×   | 7.0×    |

All figures come from a single run — absolute times shift with machine load, so
the stable quantity is the ratio within a run. A second run agreed on every
ratio.

Reproduce with `cd benches/comparative && cargo run --release --bin timing`.

The *agentic* column shows what building the ontology costs when it happens.
By default it no longer happens per frame: `OntologyMode::OnDemand` builds the
tree on the next agent query instead, via a paint-free `view` pass. A UI at
60 fps queried 5 times a second spends 1.9× less at 1000 rows and 3× less at
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
- `TextInput::on_input` and `Slider::on_change` do the same for widgets that
  carry a value, so the TodoMVC sample now has no `execute_action` handler at
  all and the premium for being agent-driveable there is **+6%**.

Together those took the counter from 1.49× to **1.24×** egui's tokens and
TodoMVC from 1.86× to **1.63×**.

Verifying it afterwards is where that inverts. Dewey closes the full
discover → act → verify loop over the agent protocol headlessly — no window,
no GPU, no screenshot. A nine-step TodoMVC task (add two items, complete one,
switch filter, read the result back):

| Task                                       | Time        | Rate      |
| ------------------------------------------ | ----------- | --------- |
| counter: discover → understand → act → verify | **8.2 µs**  | 122,000/s |
| polling an unchanged screen (`get_tree since=`) | **100 ns**  | 110× less |
| todomvc: 9-step add/complete/filter/verify | **44.7 µs** | 22,000/s  |

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

egui and iced have no equivalent to measure: neither exposes a widget tree, a
typed action, or a readable state snapshot to an external process, so an agent
must open a real window, synthesise input at guessed coordinates, screenshot it
and ask a vision model what the label says. That returns a probabilistic
reading of an image where `get_state` returns `"1 items left"` — an agent can
assert on one and only guess at the other.

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
10. **Platform-complete** — Multi-window, system tray, native file dialogs, and drag-and-drop with typed payloads — all built in

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

## License

AGPL-3.0-or-later — free for open-source use. Commercial licenses are available from [NERVOSYS](https://nervosys.ai) for proprietary/closed-source applications.
