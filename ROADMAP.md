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
- [x] Click parameters (`ClickParams`) — a widget whose action takes a
      parameter says what a click on it supplies. `Slider`, `Tabs`, `List`,
      `Table` and `Toolbar` map the point; the rest declare that a click
      cannot answer and do nothing rather than applying a default
- [x] Theme system (semantic tokens, dark/light presets, custom themes)
- [~] Overlay manager for layered rendering (`OverlayStack`) — still nothing:
      no frame renders a stack and no backend hit-tests against one. Deferred
      drawing is done by `Frame::overlay`, which queues a closure to run after
      every widget has had its turn and is what `Select` opens its list with;
      it keeps no stack and needs none, since `HitMap` already orders by
      registration and z. `OverlayStack` remains unused, and this entry exists
      to say so rather than to imply the feature is missing
- [~] Animation interpolation (linear, ease-in/out, spring, bounce) — offered
      to applications; no host advances one, and `Event::Tick` carries no
      elapsed time, so an application must keep its own clock
- [x] Error types (`DeweyError`, `DeweyResult`)

### Task Command Execution
- [x] Synchronous task execution in HeadlessDriver, RpcTransport, DeweyApp
- [x] Async task support via `Command::Task` with `CancellationToken`
- [x] Task cancellation and timeout handling

### Agent Protocol
- [x] JSON Lines stdin/stdout protocol (15 request types)
- [x] `HeadlessDriver` for running apps without a window
- [x] `RpcTransport` for bidirectional agent communication
- [x] `AgentSession` managing subscriptions and state diffs
- [x] Request types: Ping, Quit, QueryOntology, GetSchema, GetTree, GetState, ExecuteAction, InjectEvent, Subscribe, Unsubscribe, Screenshot, BatchActions, Negotiate, Validate, GetPerformance
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
- [x] Select — dropdown selection with label. The option list is drawn: it
      goes through `Frame::overlay` so it appears over the field beneath it
      rather than under it, and a click picks the option it landed on
- [x] List — scrollable item list with selection
- [x] Tabs — tabbed navigation
- [x] Table — columnar data with sortable headers
- [x] Scroll — scrollable content container. The wheel scrolls it: it
      registers a hitbox and says what a turn over it means, where before a
      turn reached no widget on any host
- [x] Container — styled box with padding/border
- [x] Panel — named content section
- [x] Menu — a bar that opens. The items are drawn through `Frame::overlay`
      so they fall over what is beneath, a click on the bar toggles them and a
      click on an item chooses it; a disabled item is greyed and refuses. The
      node publishes the items, which it did not: it carried a title and
      nothing else, so an agent could not learn what there was to pick
- [x] Tooltip — the tip is drawn over everything else while the pointer is on
      the label, through `Frame::overlay`. It used to paint the label alone and
      publish the tip to the ontology, with a doc comment saying popups were
      "handled by the backend" — none was, so an agent could read a tooltip
      nobody could see
- [x] Tree — hierarchical expand/collapse with path-based actions, reachable
      by pointer and by Tab. It registered handlers and no hitbox, so an agent
      could expand a node and a person could not
- [x] Canvas — custom drawing surface with DrawCommand API (line, rect, circle, text)
- [x] Image — URI and RGBA image display with fit modes (Cover, Contain, Fill, Original)
- [x] Modal — dialog overlay with backdrop dimming and input blocking. The
      barrier registers at z 0, not `u32::MAX`: blocking is done by *being* a
      barrier, and a backdrop that also outranked everything meant the dialog's
      own buttons could not be pressed
- [x] ColorPicker — HSV/hex colour selection with preview. The swatch opens a
      saturation-value square and a hue strip, drawn through `Frame::overlay`
      as a grid of flat fills because `Painter` has no gradient primitive. A
      click in the square picks a saturation and a value, a click on the strip
      moves the hue and keeps the rest
- [x] Toolbar — action grouping with separators
- [x] Splitter — resizable panels (horizontal/vertical)
- [x] CommandPalette — fuzzy-search command launcher. A click on a result runs
      it and a click outside closes it; it used to cover the window with a
      hitbox and answer nothing
- [x] VirtualList — virtualized scrolling for large datasets, and the wheel
      moves it by rows rather than by pixels, since `scroll_to` takes an index

### Utilities
- [x] Fuzzy matching (Jaro-Winkler scoring)
- [x] Undo/Redo stack with configurable depth
- [x] Backend configuration builder (window size, transparency, icon, decorations)
- [x] State persistence (`StateStore` with serde serialization)

### Animation

Every type here works and is tested. None of it is driven: nothing in the
crate calls `Animation::tick`, so an animation moves only if the application
advances it from its own `update`. Doing that well wants an elapsed time the
runtime does not hand over — `Event::Tick` is a unit variant, and giving it a
duration breaks every `match` on `Event`, so it waits for a major version.

- [x] 34 easing functions (linear, quad, cubic, quart, quint, sine, expo, circ, back, elastic, bounce)
- [x] `Tween` interpolation with duration and easing
- [x] `Spring` physics-based animation
- [x] `Timeline` for coordinated animations
- [x] `KeyframeSequence` for multi-keyframe animations
- [~] Driven by the runtime — nothing calls `tick`, and `Event::Tick` carries
      no delta for an application to call it with

### Testing
- [x] 93 unit tests in the library (including 24 agpu backend tests)
- [x] 210 integration tests across 11 binaries — driver, widgets, protocol,
      focus, theme, accessibility, and the standing checks in
      `backend_parity`, `reachability`, `docs_conformance`, `click_position`
      and `frame_cost`
- [x] 6 property-based tests
- [x] 29 doctests
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
- [x] Drag-and-drop support (`DragDropEvent`, `DragDropKind`, `DragPayload`)
      — read out of press, movement and release by `drag::DragTracker`, which
      every host feeds. A widget offers a payload through `Frame::register_drag`
      and `List::draggable` is the first to; the protocol gained
      `mouse_release`, without which no agent could complete a drop. File drops
      are a different event and work on both backends
- [x] Rich text / Markdown rendering (`RichText` widget with `TextSpan` and `parse_markdown()`)
- [x] Data-bound Table with sorting (`SortDirection`), filtering, pagination
- [x] Date/time picker widget (`DatePicker` with calendar grid, `DateValue`,
      `DatePickerState`) — every cell answers for itself, and the month arrows
      are painted where the click map says they are. They used to be drawn
      inside one string with the month name between them, so their positions
      depended on the width of the month's name
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
- [~] Web backend (`WebPainter` with `WebRenderOp` for wasm32/Canvas 2D) — a
      painter, not a runner: it records operations for a host page to replay,
      and there is no event loop, no `main` and nothing that starts an
      application. Driving one is the embedder's job
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
      `TrayEvent`, `NullTrayBackend`, and `PlatformTray` behind the
      `system-tray` feature) — a real icon on Windows, macOS and Linux; still
      not wired into the runtime, so the application owns the backend and polls
      it from `update`
- [~] Native file dialogs (`DialogBackend` trait, `OpenFileDialog`,
      `SaveFileDialog`, `MessageBox`) — types only, no platform backend

### v1.3 — Performance & Polish
- [~] GPU-accelerated canvas rendering (`RenderBatch`, `RenderPrimitive`, quad
      merging) — no `Painter` builds a batch and nothing submits one, so no
      draw call has been saved by it. The agpu backend paints through agpu's
      own `ShapeRenderer` and `TextEngine` and does not pass through it
- [x] Profiling instrumentation (`Profiler`, `FrameProfile`, FPS/timing/widget
      count tracking) — driven by the headless driver and the default backend
      through `HeadlessDriver`, and read by the `get_performance` request.
      `FrameProfile::update` was filled from a timer no host ever started and
      read zero everywhere; every host now delivers messages through
      `HeadlessDriver::update_model`, which times them. `FrameProfile::layout`
      is still always zero and is not reported: Dewey lays out inside
      `Model::view`, so there is no separate pass to time
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
      loop) — off unless `with_profiling(true)`, and still read by nothing:
      this backend answers no agent requests, so `get_performance` cannot reach
      the profiler it keeps. Its `update` timer is never started either
- [x] ProgramOptions parity (fullscreen, transparent window support)
- [x] Unit tests (24 tests — type conversion, event conversion, builder API)

---

## Known limits

### The ontology is only worth what the agent asks it

Every performance figure in this project assumes the agent uses the protocol.
The ontology costs the same whether or not anyone asks it, so a model that does
not ask turns a measured win into pure overhead.

**Measured on 8 September 2026, and the prediction that used to stand here was
half right.** An agent writing a Dewey application does go around the ontology —
zero calls in twelve runs — but not to the source: it reads the *examples* and
`llms.txt`, and only three reads in twelve runs touched `src/` at all. It scored
1.000 every time, so nothing was lost by not asking.

The other half was wrong, and it was the part this section was built on. The
levers were supposed to be the text a model reads before it decides. Twelve runs
say that text changes nothing: the arm with the instructions delivered behaved
exactly like the arm without, and the arm that was additionally warned behaved
like both. What moved the needle was not persuasion but the absence of an
alternative — given a running program and a question only it can answer, an
agent asks, and typed tools make asking cheaper than a raw pipe.

The levers are kept because they are cheap, and because for a driving task the
tooled arm is genuinely the cheapest of three. They are no longer claimed to
change a writing agent's mind:

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
- [x] `docs/agent-prompt.md`, for clients that surface no MCP instructions,
      and `llms.txt`, the machine-readable index a model reads first. Both now
      report what the runs found rather than hedging about it, and `llms.txt`
      carries a compiled example — it had no Rust in it at all until twelve
      runs showed it was the third-most-read file in the repository

**The sibling project already ran the experiment, and it did not work.**
HawkTUI's `benchmarks/agentic/` drives a real model over 184 recorded runs.
Its finding: agents read the implementation in **100% of Hawk TUI runs**, 16–22
reads per run, the first at tool call #1 — against **6% of ratatui runs**, 0.1
reads per run, first at call #8. A model reaches for source when the framework
is one it was not trained on, and Dewey is in exactly that position.

Adding MCP tools raised ontology consultation from 4% to 42%, and adding
trigger prompts to 83% — and **the outcomes did not move**: score 1.000 in all
three arms, cost $0.78 / $0.79 / $0.78, and one task got monotonically worse
(19 → 37 → 53 turns). Consultation is not the metric.

Dewey's own runs agree with that last clause exactly and disagree with the
middle one. No arm differed on outcome here either — every `t1-counter` run
scored 1.000 whatever it was given. But where MCP tools took HawkTUI's
consultation from 4% to 42%, here they took it from zero to zero, and the
trigger-prompt arm moved nothing either. For a *driving* task the same tools
were the cheapest arm of three. The instructions are cheap and worth keeping;
what they are worth depends on whether the agent has an alternative.

- [x] `benches/agentic/` has been run. **24 paid runs, $19.15, 8 September
      2026**, three conditions, and the answer is two halves that point
      opposite ways.

      **Writing an application** (`t1-counter`, 12 runs): zero ontology calls
      in every single run — including the four where the MCP server reported
      `connected`, its tools were listed and `initialize` had delivered the
      instructions. Every run scored 1.000 anyway, reading
      `examples/counter.rs`, `examples/agent_headless.rs` and `llms.txt`. Only
      three reads in twelve runs touched `src/` at all, so the "reads the
      implementation" prediction below is half right: it does go around the
      ontology, but to the examples rather than the source.

      **Driving an application it did not write** (`t3-inspect`, 12 runs):
      12/12 correct, and the `mcp` arm was the cheapest of the three — 19.8
      turns at $0.37 against 29.0 and $0.58 bare. When the answer exists only
      in a running program the agent asks, and typed tools make asking cheaper.

      So: ship the MCP server for agents that *drive*; spend the effort on
      examples and `llms.txt` for agents that *write*. One model, n=4 per arm
      — a direction, not a measurement. Arm-by-arm results, the twelve harness
      defects paid for on the way, and the four runs excluded because the
      harness rather than the agent failed them, are in
      `benches/agentic/README.md`.

---

## Planned

### v2.0 — Ecosystem
- [ ] Published to crates.io
- [ ] Semantic versioning policy
- [ ] Migration guide from egui/eframe

---

## Progress Summary

| Area                | Status                                 |
| ------------------- | -------------------------------------- |
| Core runtime        | Complete                               |
| Agent protocol (15) | Complete                               |
| Ontology system     | Complete                               |
| Widgets (30)        | Complete                               |
| Animation           | Works, undriven — nothing calls `tick` |
| Accessibility       | Complete                               |
| State persistence   | Complete                               |
| Testing (332 + 213) | Complete                               |
| Benchmarks          | Complete                               |
| API polish          | Complete                               |
| Rustdoc             | Complete                               |
| CI/CD               | Complete                               |
| Examples (11)       | Complete                               |
| agpu GPU backend    | Complete                               |
| Async tasks         | Complete                               |
| Painter abstraction | Complete                               |
| Web backend         | A painter; no runner or event loop     |
| crates.io publish   | Planned (v2.0)                         |
