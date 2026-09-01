# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Performance

- `UiNode::widget_type`, `UiNode::agent_id`, and hit-map ids are now
  `Cow<'static, str>` instead of `String`. Every call site passes a string
  literal, so agentic frames no longer allocate a `String` per widget per
  field: 18.0 → 11.0 allocations per row (−39%).
- `UiNode::state` is now a `Properties` vector with borrowed keys rather than a
  `serde_json::Value`. Widgets built a `serde_json` map every frame and
  allocated a `String` for each key, though the keys are string literals baked
  into the widget; `Properties` serializes identically but costs one vector
  allocation and no key allocations: 11.0 → 8.0 allocations per row (−27%),
  3173 → 2308 bytes. Agentic frame build improved 23% at 100 rows and 21% at
  1000 rows.
- Widgets now build their `UiNode` at the end of `render`, after painting has
  finished borrowing their fields, so owned values move into the state instead
  of being cloned. `json!(expr)` takes its argument by reference, so
  `json!(self.text)` cloned the string every frame — and `List`, `Select`, and
  `Tabs` cloned an entire `Vec<String>` every frame, one allocation per item.
  8.0 → 6.0 allocations per row; 18.0 → 6.0 (−67%) cumulatively. Node
  registration still follows render order. `Table` and the four widgets with
  early returns in `render` were left unchanged.
- `UiNode` shrank from 304 to 176 bytes (−42%). `Accessibility` is 136 bytes of
  mostly-`None` options and was stored inline in every node, though no widget
  in the library sets it; it is now `Option<Box<Accessibility>>`. Bytes
  allocated per row fell 2290 → 1767 (−23%) with the allocation count
  unchanged. The JSON wire format is unchanged.
- `OntologyMode` decides when the ontology tree is built, and defaults to
  `OnDemand`: the tree is built on the next agent query by a paint-free `view`
  pass rather than during every rendered frame. A UI at 60 fps queried 5 times
  a second spends 1.9× less on the ontology at 1000 rows and 3–4× less at 5000.
  `Model::view` takes `&self`, so the pass has no side effects and the tree is
  built from current model state rather than the last painted frame.
  `EveryFrame` restores the previous behavior; `Disabled` skips it entirely.
- `runtime::build_ontology_tree` builds a tree from a model without painting.
- `Frame::with_ontology` and `Frame::ontology_enabled` let a frame skip
  building the tree. Widgets check before constructing a `UiNode`, so the cost
  is skipped rather than discarded: 18.0 → 4.0 allocations per row and
  3210 → 598 bytes (−81%). Hit-testing and painting are unaffected.

### Added

- `benches/scaffold` — canonical counter app implemented in Dewey (with and
  without agent affordances), egui, and iced, with `measure.py` reporting what
  an agent must write and how long it waits for `cargo check`.
- `agent_loop` benchmark measuring the full headless discover → understand →
  act → verify loop over the agent protocol: 11.9 µs end to end, ≈84,000 loops
  per second.
- `Model::handle_event` has a default implementation returning `None`, so an
  application driven entirely by widget interaction and agent actions no longer
  has to write it.

- `benches/comparative` — a standalone crate benchmarking frame-build cost
  against egui 0.31 and iced 0.13, plus an `allocs` binary reporting
  deterministic per-frame allocation counts and a `timing` binary that
  interleaves frameworks and reports minimum frame time, so results stay
  meaningful on a contended machine.
- Regression tests covering the ontology gate: hitboxes and draw calls must be
  identical whether or not the ontology is built.

### Fixed

- Corrected the comparative performance table. The first version was measured
  with criterion on a machine at 100% CPU and overstated egui's cost by ~2× and
  iced's by ~3× at 5000 rows; it also ranked 3000 rows slower than 5000 rows.
  Re-measured with the interleaved minimum-time harness.

### Changed

- `UiNode.accessibility` is `Option<Box<Accessibility>>` rather than
  `Accessibility`. Read it with `UiNode::accessibility()`, which returns an
  empty set when unset; `with_accessibility` is unchanged and stores nothing
  for an all-default value.
- `ProgramOptions::ontology` is an `OntologyMode` rather than a `bool`, and the
  ontology tree is no longer rebuilt during every rendered frame by default.
  Code reading the ontology registry outside the agent request path should call
  `runtime::build_ontology_tree`, or select `OntologyMode::EveryFrame`.

- `UiNode.widget_type` and `UiNode.agent_id` changed type (`String` →
  `Cow<'static, str>`). Code that constructs these with literals or `String`s
  is unaffected; code that reads the fields as `&String` needs `&*` or
  `.as_ref()`.
- `UiNode.state` changed type (`serde_json::Value` → `Properties`). The JSON
  wire format is unchanged. `with_state` accepts anything convertible, so
  passing a `serde_json::Value` still works; code that called `Value` methods
  on the field directly should use `Properties::get`/`iter`, or `to_value()`
  for a `serde_json::Value`. `Properties` compares by content, not key order,
  so `StateChanged` is not emitted for a state that merely round-tripped
  through JSON.

## [1.0.0] - 2025-07-05

### Added

#### Core Architecture
- Elm architecture runtime (Model/Msg/Command/View)
- `Program` runner with `ProgramOptions` (width, height, tick rate)
- `DeweyApp` eframe integration with wgpu backend (`EguiPainter`)
- `Frame` abstraction with `Painter` trait (backend-agnostic rendering)
- `Painter` trait — 9 primitives (fill_rect, stroke_rect, fill_circle, stroke_circle, line, text, measure_text, push_clip, pop_clip)
- `NullPainter` for headless/no-op rendering
- `EguiPainter` for GPU-accelerated rendering via egui/wgpu
- All 30 widgets decoupled from egui — render exclusively through `Painter`
- `Rect` geometry type with hit-testing, splitting, padding
- Layout engine (Horizontal/Vertical, Constraint-based splits)
- Focus management (Tab/Shift+Tab ring navigation)
- Theme system (semantic tokens, dark/light presets, custom themes)
- Overlay manager for layered rendering
- Animation interpolation (linear, ease-in/out, spring, bounce)
- Error types (`DeweyError`, `DeweyResult`)

#### Task Command Execution
- Synchronous task execution in HeadlessDriver, RpcTransport, DeweyApp
- Async task support via `Command::Task` with `CancellationToken`
- Task cancellation and timeout handling

#### Agent Protocol
- JSON Lines stdin/stdout protocol (14 request types)
- `HeadlessDriver` for running apps without a window
- `RpcTransport` for bidirectional agent communication
- `AgentSession` managing subscriptions and state diffs
- Request types: Ping, Quit, QueryOntology, GetSchema, GetTree, GetState, ExecuteAction, InjectEvent, Subscribe, Unsubscribe, Screenshot, BatchActions, Negotiate, ListActions
- `RequestEnvelope` / `AgentResponse` with request ID correlation
- Screenshot implementation (returns UiTree snapshot)
- State diff subscriptions (only send changed fields)
- Batch action execution (multiple actions in one request)
- Agent capability negotiation handshake
- WebSocket transport (`WsTransport`) alternative to stdin/stdout (feature-gated `ws-transport`)
- Protocol versioning (v2) with backward compatibility

#### Ontology System
- `Discoverable` trait on every widget (schema, capabilities, actions, state)
- `WidgetSchema` with `SemanticRole` classification
- `AgentCapability` enum (22+ granular capabilities)
- `AgentAction` with typed parameter validation
- `OntologyRegistry` with filtering by role/query
- `UiTree` / `UiNode` for hierarchical widget introspection
- `Accessibility` struct (label, description, keyboard_shortcut, live_region)

#### Widgets (30 total)
- Button — clickable action with enabled/disabled state
- Label — static/dynamic text display
- Checkbox — boolean toggle
- Radio — single-selection indicator
- TextInput — single-line text entry with cursor state
- TextArea — multi-line text editing with selection
- Slider — numeric value selection with range
- ProgressBar — determinate progress indicator
- Select — dropdown selection with label
- List — scrollable item list with selection
- Tabs — tabbed navigation
- Table — columnar data with sortable headers
- Scroll — scrollable content container
- Container — styled box with padding/border
- Panel — named content section
- Menu — hierarchical menu with submenus
- Tooltip — hover text with label display
- Tree — hierarchical expand/collapse with path-based actions
- Canvas — custom drawing surface with DrawCommand API
- Image — URI and RGBA display with fit modes (Cover, Contain, Fill, Original)
- Modal — dialog overlay with backdrop dimming and input blocking
- ColorPicker — HSV/hex color selection with preview
- Toolbar — action grouping with separators
- Splitter — resizable panels (horizontal/vertical)
- CommandPalette — fuzzy-search command launcher
- VirtualList — virtualized scrolling for large datasets
- RichText — Markdown rendering with `TextSpan` and `parse_markdown()`
- DatePicker — calendar grid with date selection
- Chart — Line/Bar/Pie charts via `Series` data
- DragDrop — drag-and-drop support with `DragPayload` types

#### Framework Features
- Hot-reload support for theme changes (`ThemeWatcher`, `load_from_json`, `save_to_json`)
- Internationalization framework (`I18n`, `MessageCatalog`, locale fallback, `t_fmt()`)
- Plugin system (`Plugin` trait, `PluginRegistry`, `PluginContext`)

#### Backend & Platform
- Web backend (`WebPainter` with `WebRenderOp` for wasm32/Canvas 2D)
- Headless rendering to image buffer (`ImagePainter` software rasterizer)
- Software rasterizer (pixel-level fill_rect, fill_circle, line, stroke, alpha blending)
- Multi-window support (`WindowManager`, `WindowConfig`, focus tracking)
- System tray integration (`TrayBackend` trait, `TrayConfig`, `NullTrayBackend`)
- Native file dialogs (`DialogBackend` trait, `OpenFileDialog`, `SaveFileDialog`, `MessageBox`)

#### Performance & Polish
- GPU-accelerated canvas rendering (`RenderBatch`, `RenderPrimitive`, quad merging)
- Profiling instrumentation (`Profiler`, `FrameProfile`, FPS/timing/widget count tracking)
- Memory optimization (`Arena` bump allocator, `VecPool` buffer reuse, `InlineString`)

#### Utilities
- Fuzzy matching (Jaro-Winkler scoring)
- Undo/Redo stack with configurable depth
- Backend configuration builder (window size, transparency, icon, decorations)
- State persistence (`StateStore` with serde serialization)

#### Animation
- 34 easing functions (linear, quad, cubic, quart, quint, sine, expo, circ, back, elastic, bounce)
- `Tween` interpolation with duration and easing
- `Spring` physics-based animation
- `Timeline` for coordinated animations
- `KeyframeSequence` for multi-keyframe animations

#### Testing & CI
- 108 tests (67 unit + 34 integration + doc tests)
- Test backend with `Painter` impl for non-GPU validation
- Criterion benchmark suite (easing, tween, ontology, virtual list)
- CI/CD pipeline (check, test, clippy, fmt, doc)

#### Examples
- `hello` — minimal Dewey application
- `counter` — Elm architecture with key events
- `agent_demo` — headless agent protocol interaction
- `showcase` — all major widgets in one window
- `canvas_drawing` — interactive shape builder
- `ontology_explorer` — headless ontology discovery
- `agent_headless` — full TodoApp agent session simulation
- `chat` — AI chat interface with simulated streaming responses

[Unreleased]: https://github.com/nervosys/DeweyGUI/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/nervosys/DeweyGUI/releases/tag/v1.0.0
