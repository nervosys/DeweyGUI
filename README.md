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
headless with no GPU. Full methodology, raw numbers, and caveats in
[`benches/comparative/`](benches/comparative/README.md).

| Rows | Dewey            | egui 0.31  | iced 0.13    | Dewey vs egui | Dewey vs iced |
| ---- | ---------------- | ---------- | ------------ | ------------- | ------------- |
| 100  | **27–35 µs**     | 193–283 µs | 94–111 µs    | 5.4–8.0×      | 2.7–3.5×      |
| 1000 | **0.35–0.65 ms** | 3.1–8.0 ms | 1.02–1.08 ms | 4.8–12×       | 1.6–3.1×      |
| 5000 | **1.7–2.4 ms**   | 28–30 ms   | 51–55 ms     | 11.5–17×      | 21–32×        |

Dewey was fastest at every size across two independent runs. Reproduce with
`cd benches/comparative && cargo bench`.

Two caveats worth stating plainly: Dewey estimates text extents during frame
build rather than shaping glyphs — its GPU backends pay that cost at render
time, which narrows the 5000-row gap from ~12× to roughly ~4–5× — and Dewey
defers the interaction pass that egui performs inline, so part of the margin is
architectural rather than raw efficiency. Tessellation, rasterization, and
present are excluded for all three.

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
