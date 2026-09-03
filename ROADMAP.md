# Dewey Roadmap

Agentic-first GUI framework for Rust with pluggable rendering backends.

- **Dewey** v1.0.0 — edition 2024, rust-version 1.85
- **agpu** v2.0.0 — edition 2024, rust-version 1.85

---

## Completed

### Core Architecture
- [x] Elm architecture runtime (Model/Msg/Command/View)
- [x] `Program` runner with `ProgramOptions` (width, height, tick rate)
- [x] `DeweyApp` eframe integration with wgpu backend (EguiPainter)
- [x] `Frame` abstraction with `Painter` trait (backend-agnostic rendering)
- [x] `Painter` trait — 9 primitives (fill_rect, stroke_rect, fill_circle, stroke_circle, line, text, measure_text, push_clip, pop_clip)
- [x] `NullPainter` for headless/no-op rendering
- [x] `EguiPainter` for GPU-accelerated rendering via egui/wgpu
- [x] All 30 widgets decoupled from egui — render exclusively through Painter
- [x] `Rect` geometry type with hit-testing, splitting, padding
- [x] Layout engine (Horizontal/Vertical, Constraint-based splits)
- [x] Focus management — Tab/Shift+Tab in render order, Enter and Space to
      activate, and a focus ring the runtime draws. Driven by the default
      backend, agpu and the headless driver through one implementation, so an
      agent can Tab through an interface exactly as a person does
- [x] Theme system (semantic tokens, dark/light presets, custom themes)
- [~] Overlay manager for layered rendering (`OverlayStack`) — no frame
      renders a stack and no backend hit-tests against one, so an overlay
      pushed here appears nowhere and blocks nothing. The `Modal` widget draws
      its own backdrop and does not go through it
- [x] Animation interpolation (linear, ease-in/out, spring, bounce)
- [x] Error types (`DeweyError`, `DeweyResult`)

### Task Command Execution
- [x] Synchronous task execution in HeadlessDriver, RpcTransport, DeweyApp
- [x] Async task support via `Command::Task` with `CancellationToken`
- [x] Task cancellation and timeout handling

### Agent Protocol
- [x] JSON Lines stdin/stdout protocol (14 request types)
- [x] `HeadlessDriver` for running apps without a window
- [x] `RpcTransport` for bidirectional agent communication
- [x] `AgentSession` managing subscriptions and state diffs
- [x] Request types: Ping, Quit, QueryOntology, GetSchema, GetTree, GetState, ExecuteAction, InjectEvent, Subscribe, Unsubscribe, Screenshot, BatchActions, Negotiate, ListActions
- [x] `RequestEnvelope` / `AgentResponse` with request ID correlation
- [x] Screenshot implementation (returns UiTree snapshot)
- [x] State diff subscriptions (only send changed fields)
- [x] Batch action execution (multiple actions in one request)
- [x] Agent capability negotiation handshake
- [x] `CancellationToken` for async task cancellation

### Ontology System
- [x] `Discoverable` trait on every widget (schema, capabilities, actions, state)
- [x] `WidgetSchema` with `SemanticRole` classification
- [x] `AgentCapability` enum (22+ granular capabilities)
- [x] `AgentAction` with typed parameter validation
- [x] `OntologyRegistry` with filtering by role/query
- [x] `UiTree` / `UiNode` for hierarchical widget introspection
- [x] `Accessibility` struct (label, description, keyboard_shortcut, live_region)

### Widgets (27 total)
- [x] Button — clickable action with enabled/disabled state
- [x] Label — static/dynamic text display
- [x] Checkbox — boolean toggle
- [x] Radio — single-selection indicator
- [x] TextInput — single-line text entry with cursor state
- [x] TextArea — multi-line text editing with selection
- [x] Slider — numeric value selection with range
- [x] ProgressBar — determinate progress indicator
- [x] Select — dropdown selection with label
- [x] List — scrollable item list with selection
- [x] Tabs — tabbed navigation
- [x] Table — columnar data with sortable headers
- [x] Scroll — scrollable content container
- [x] Container — styled box with padding/border
- [x] Panel — named content section
- [x] Menu — hierarchical menu with submenus
- [x] Tooltip — hover text with label display
- [x] Tree — hierarchical expand/collapse with path-based actions
- [x] Canvas — custom drawing surface with DrawCommand API (line, rect, circle, text)
- [x] Image — URI and RGBA image display with fit modes (Cover, Contain, Fill, Original)
- [x] Modal — dialog overlay with backdrop dimming and input blocking
- [x] ColorPicker — HSV/hex color selection with preview
- [x] Toolbar — action grouping with separators
- [x] Splitter — resizable panels (horizontal/vertical)
- [x] CommandPalette — fuzzy-search command launcher
- [x] VirtualList — virtualized scrolling for large datasets

### Utilities
- [x] Fuzzy matching (Jaro-Winkler scoring)
- [x] Undo/Redo stack with configurable depth
- [x] Backend configuration builder (window size, transparency, icon, decorations)
- [x] State persistence (`StateStore` with serde serialization)

### Animation
- [x] 34 easing functions (linear, quad, cubic, quart, quint, sine, expo, circ, back, elastic, bounce)
- [x] `Tween` interpolation with duration and easing
- [x] `Spring` physics-based animation
- [x] `Timeline` for coordinated animations
- [x] `KeyframeSequence` for multi-keyframe animations

### Testing
- [x] 101 unit tests across all modules (including 24 agpu backend tests)
- [x] 58 integration tests covering driver, widgets, protocol, focus, theme, accessibility
- [x] 6 property-based tests
- [x] 5 doctests
- [x] Test backend with `Painter` impl for non-GPU validation (records RenderOps)
- [x] Criterion benchmark suite (easing, tween, ontology, virtual list)
- [x] agpu crate: 213 standalone tests

### API Polish
- [x] `#[must_use]` on all pure-value constructors and builders
- [x] `Default` impls for `Position`, `Size`, `Shadow`
- [x] Module-level rustdoc on all public modules
- [x] CI/CD pipeline (check, test, clippy, fmt, doc)

### Examples
- [x] `hello` — minimal Dewey application
- [x] `counter` — Elm architecture with key events
- [x] `counter_agpu` — counter using agpu GPU backend
- [x] `agent_demo` — headless agent protocol interaction
- [x] `agent_headless` — full headless agent session
- [x] `showcase` — all major widgets in one window
- [x] `canvas_drawing` — interactive shape builder
- [x] `ontology_explorer` — headless ontology discovery
- [x] `chat` — chat interface demo
- [x] `chat_agpu` — LLM chat app with model selector using agpu GPU backend
- [x] `hello_agpu` — minimal agpu GPU backend window (agpu crate)

### v1.1 — Agent Protocol Enhancements
- [x] `Program::with_agent` — a windowed application answers the protocol on
      stdin/stdout while the window is open. Every transport owns the model
      and so does `run`, so until this an application was agent-driven or
      windowed and never both
- [x] WebSocket transport (`WsTransport`) alternative to stdin/stdout (feature-gated `ws-transport`)
- [x] Protocol versioning (v2) with backward compatibility (min v1, server capabilities)

### v1.1 — Widget Improvements (30 widgets total)
- [~] Drag-and-drop support (`DragDropEvent`, `DragDropKind`, `DragPayload`)
      — the vocabulary only. No backend emits one: agpu converts an
      `agpu::Event::DragDrop` that the agpu crate never constructs, the
      default backend has no drag-drop path, and the agent protocol cannot
      inject one, so `handle_event` is never called with it. File drops are
      a different event and do work on both backends
- [x] Rich text / Markdown rendering (`RichText` widget with `TextSpan` and `parse_markdown()`)
- [x] Data-bound Table with sorting (`SortDirection`), filtering, pagination
- [x] Date/time picker widget (`DatePicker` with calendar grid, `DateValue`, `DatePickerState`)
- [x] Chart widget (`Chart` with `Line`/`Bar`/`Pie` kinds via `Series` data)

### v1.2 — Framework Features
- [~] Hot-reload support for theme changes (`ThemeWatcher`, `load_from_json`,
      `save_to_json`) — an application polls `check()` itself; nothing in the
      runtime drives the watcher, and no widget reads an ambient theme, so
      applying a reloaded theme is the application's job too
- [x] Internationalization framework (`I18n`, `MessageCatalog`, locale fallback, `t_fmt()`)
- [x] Plugin system (`Plugin` trait, `PluginRegistry`, `PluginContext`,
      `plugin::initialise`) on both backends. It was listed as complete here
      while `Program` had no way to register a plugin at all, so the whole
      lifecycle ran only under the opt-in `agpu-backend`. What a plugin
      contributes to the theme and the message catalogue arrives at the
      application through `Model::plugins_ready`; no widget reads an ambient
      theme or looks a string up in the catalogue, so the framework itself
      consumes neither

### v1.2 — Backend & Platform
- [x] Web backend (`WebPainter` with `WebRenderOp` for wasm32/Canvas 2D)
- [x] Headless rendering to image buffer (`ImagePainter` software rasterizer)
- [x] Software rasterizer (pixel-level fill_rect, fill_circle, line, stroke, alpha blending)
- [ ] Upgrade `agpu` from wgpu 24 to wgpu 30, which unblocks moving the
      `eframe` pin off 0.31 and silences the present-mode warning. Six major
      wgpu versions and 263 call sites; the two wgpu versions cannot coexist
      because they force incompatible `windows` versions on a shared allocator
- [x] Window control commands (show, hide, focus, minimise, move, resize,
      always-on-top, fullscreen, title) on both backends
- [~] Multi-window (`WindowManager`, `WindowConfig`, focus tracking) — in-memory
      bookkeeping; does not create or raise real windows
- [~] System tray (`TrayBackend` trait, `TrayConfig`, `TrayIconImage`,
      `TrayEvent`, `NullTrayBackend`) — types only, no platform backend, not
      wired into the runtime
- [~] Native file dialogs (`DialogBackend` trait, `OpenFileDialog`,
      `SaveFileDialog`, `MessageBox`) — types only, no platform backend

### v1.3 — Performance & Polish
- [~] GPU-accelerated canvas rendering (`RenderBatch`, `RenderPrimitive`, quad
      merging) — no `Painter` builds a batch and nothing submits one, so no
      draw call has been saved by it. The agpu backend paints through agpu's
      own `ShapeRenderer` and `TextEngine` and does not pass through it
- [~] Profiling instrumentation (`Profiler`, `FrameProfile`, FPS/timing/widget
      count tracking) — driven only by the agpu backend, which is opt-in; the
      default backend has no profiling at all. Nothing reads `last_frame()` or
      `history()` either, so what agpu measures goes nowhere. Surfacing it
      wants a protocol request an agent can ask, which is not written
- [~] Memory optimization (`Arena` bump allocator, `VecPool` buffer reuse,
      `InlineString`) — offered to applications; no allocation in Dewey goes
      through any of them. The per-frame counts that did come down (18.0 to
      4.0 per row) came from not building the nodes and from borrowing keys
      that were being copied

### v1.4 — agpu GPU Backend
- [x] `AgpuBridgePainter` implementing Dewey's `Painter` via agpu's `ShapeRenderer` + `TextEngine`
- [x] `AgpuProgram` runner — winit event loop driving Dewey's `Model` with GPU rendering
- [x] Type conversion bridge (Dewey ↔ agpu core types: Rect, Position, Size, Color, TextStyle)
- [x] Event conversion (agpu/winit events → Dewey events)
- [x] MSAA support (configurable sample count, default 4x)
- [x] Ontology integration (UiTree built during each frame, agent actions validated)
- [x] Backend preference selection (Vulkan-first, OpenGL, platform default)
- [x] Feature-gated `agpu-backend` (no default, opt-in via `--features agpu-backend`)
- [x] Plugin lifecycle (PluginRegistry init/on_frame/on_shutdown hooks)
- [~] Profiler integration (begin_frame/start/stop/end_frame timing in render
      loop) — the timings are collected and never read
- [x] ProgramOptions parity (fullscreen, transparent window support)
- [x] Unit tests (24 tests — type conversion, event conversion, builder API)

---

## Known limits

### The ontology is only worth what the agent asks it

Every performance figure in this project assumes the agent uses the protocol.
A model that has not been told the application describes itself will read the
source instead — slower, far larger, and an answer about what the code *could*
do rather than what is on screen now. The ontology costs the same whether or
not anyone asks it, so an unprompted model turns a measured win into pure
overhead.

This is a limit of adoption, not of the mechanism, and the levers are the text
a model reads before it decides:

- [x] MCP `initialize` returns `instructions` saying the application describes
      itself, naming `get_tree` as the first call, and pointing at `since`,
      `viewport` and `validate`. This is the highest-leverage text in the
      project: it is what a client puts in front of the model.
- [x] Tool descriptions say why to call them rather than reading the source,
      and a test asserts they keep saying it
- [x] `benches/scaffold/src/bin/observation_cost.rs` prices five questions an
      agent has to answer, three ways: ask the application, read its source,
      or look at a picture. It found that a full `get_tree` costs more than
      this TodoMVC's entire source — the ontology wins on targeted reads, on
      change-polling, and on the three questions source cannot answer at any
      price, not on bulk. Run in CI
- [ ] A short prompt fragment applications can paste into an agent's system
      prompt, for clients that surface no MCP instructions

**The sibling project already ran the experiment, and it did not work.**
HawkTUI's `benchmarks/agentic/` drives a real model over 184 recorded runs.
Its finding: agents read the implementation in **100% of Hawk TUI runs**, 16–22
reads per run, the first at tool call #1 — against **6% of ratatui runs**, 0.1
reads per run, first at call #8. A model reaches for source when the framework
is one it was not trained on, and Dewey is in exactly that position.

Adding MCP tools raised ontology consultation from 4% to 42%, and adding
trigger prompts to 83% — and **the outcomes did not move**: score 1.000 in all
three arms, cost $0.78 / $0.79 / $0.78, and one task got monotonically worse
(19 → 37 → 53 turns). Consultation is not the metric. So the `instructions`
added to `src/agent/mcp.rs` are cheap and worth keeping, and there is no
evidence yet that they change what a model does.

- [ ] Measure with a model in the loop. Every Dewey benchmark prices a
      strategy; none observes a model choosing one, which is the only place
      the question is actually settled

---

## Planned

### v2.0 — Ecosystem
- [ ] Published to crates.io
- [ ] Semantic versioning policy
- [ ] Migration guide from egui/eframe

---

## Progress Summary

| Area                | Status         |
| ------------------- | -------------- |
| Core runtime        | Complete       |
| Agent protocol (14) | Complete       |
| Ontology system     | Complete       |
| Widgets (30)        | Complete       |
| Animation           | Complete       |
| Accessibility       | Complete       |
| State persistence   | Complete       |
| Testing (170 + 213) | Complete       |
| Benchmarks          | Complete       |
| API polish          | Complete       |
| Rustdoc             | Complete       |
| CI/CD               | Complete       |
| Examples (11)       | Complete       |
| agpu GPU backend    | Complete       |
| Async tasks         | Complete       |
| Painter abstraction | Complete       |
| Web backend         | Complete       |
| crates.io publish   | Planned (v2.0) |
