# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
