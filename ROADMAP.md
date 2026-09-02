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
- [x] Focus management (Tab/Shift+Tab ring navigation)
- [x] Theme system (semantic tokens, dark/light presets, custom themes)
- [x] Overlay manager for layered rendering
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
- [x] WebSocket transport (`WsTransport`) alternative to stdin/stdout (feature-gated `ws-transport`)
- [x] Protocol versioning (v2) with backward compatibility (min v1, server capabilities)

### v1.1 — Widget Improvements (30 widgets total)
- [x] Drag-and-drop support (`DragDropEvent`, `DragDropKind`, `DragPayload` types)
- [x] Rich text / Markdown rendering (`RichText` widget with `TextSpan` and `parse_markdown()`)
- [x] Data-bound Table with sorting (`SortDirection`), filtering, pagination
- [x] Date/time picker widget (`DatePicker` with calendar grid, `DateValue`, `DatePickerState`)
- [x] Chart widget (`Chart` with `Line`/`Bar`/`Pie` kinds via `Series` data)

### v1.2 — Framework Features
- [x] Hot-reload support for theme changes (`ThemeWatcher`, `load_from_json`, `save_to_json`)
- [x] Internationalization framework (`I18n`, `MessageCatalog`, locale fallback, `t_fmt()`)
- [x] Plugin system (`Plugin` trait, `PluginRegistry`, `PluginContext`)

### v1.2 — Backend & Platform
- [x] Web backend (`WebPainter` with `WebRenderOp` for wasm32/Canvas 2D)
- [x] Headless rendering to image buffer (`ImagePainter` software rasterizer)
- [x] Software rasterizer (pixel-level fill_rect, fill_circle, line, stroke, alpha blending)
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
- [x] GPU-accelerated canvas rendering (`RenderBatch`, `RenderPrimitive`, quad merging optimization)
- [x] Profiling instrumentation (`Profiler`, `FrameProfile`, FPS/timing/widget count tracking)
- [x] Memory optimization (`Arena` bump allocator, `VecPool` buffer reuse, `InlineString`)

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
- [x] Profiler integration (begin_frame/start/stop/end_frame timing in render loop)
- [x] ProgramOptions parity (fullscreen, transparent window support)
- [x] Unit tests (24 tests — type conversion, event conversion, builder API)

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
