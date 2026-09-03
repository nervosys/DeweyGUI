# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Performance

- `get_tree` builds the reply as bytes rather than as a `serde_json::Value`
  that is then serialised. For a 100-row interface the intermediate `Value`
  cost 379 µs of 557. The transports use `process_request_json`, which skips
  it: the reply an agent receives went **557 µs → 87 µs**, against egui's
  AccessKit tree at 264 µs. `process_request` still returns a `Value`, since an
  in-process caller wants to inspect the reply rather than send it.

- `get_tree` takes a `viewport` and describes only the widgets whose bounds
  intersect it. The tree previously described every widget in the interface,
  including those scrolled out of sight — measured against a screenshot of the
  same 1000-row application it was 24× larger and 3.7× slower, the one axis
  where a structured observation lost outright to a picture. Windowed, the same
  list is **11.7 kB against a screenshot's 16.7 kB, and 971 µs against
  1.53 ms**. The reply carries `total_nodes` and `shown_nodes` so an agent can
  tell a short list from a window onto a long one, and a container is kept when
  any descendant is visible. Clipping happens after the frame is built, so a
  list long enough for that to matter still wants `VirtualList` in the view.

- `query_ontology` serves the widget catalogue from cached bytes rather than
  cloning a `serde_json::Value` per request.

- `get_tree` accepts `since`, the `version` from a previous reply, and answers
  `unchanged` without rendering or serialising anything when the interface has
  not moved: **11.0 µs → 100 ns, 110× less**. Polling until something changes
  is the common agent pattern, and it was previously the most expensive thing
  an agent could do. The version advances on any request that could mutate the
  model; it is deliberately over-eager, since a needless refresh costs time
  while a missed one would hand an agent a stale screen.

- The headless driver no longer re-renders the application for requests that do
  not read the UI tree. `query_ontology` and `get_schema` are answered from the
  registry alone and dropped from ~1.1 µs to below timer resolution; the
  five-step agent loop went from 11.9 µs to 8.2 µs.

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

- `benches/scaffold/src/bin/observation_cost.rs` prices what it costs an agent
  to find out what is on screen: five questions, answered by asking the
  application, by reading its source, or by looking at a picture, against
  Dewey, egui and iced. It contradicts the assumption the other benchmarks
  make. A full `get_tree` is 2021 estimated tokens and this TodoMVC's entire
  egui source is 801, so on an application that small an agent that reads the
  source once has paid less than one observation. The ontology wins on
  targeted reads (120 tokens), on change-polling (29), and on the three
  questions of five that source cannot answer at any price — not on bulk. On
  an application three times the size, which is still tiny, asking is ahead
  from the first observation. Run in CI, and its assertions are about that
  shape rather than the numbers.


- MCP `initialize` returns `instructions`, and the tool descriptions say why
  to call them rather than read the application's source. The performance case
  for an ontology assumes the agent asks it; a model that has not been told
  the application describes itself reads the source instead — slower, far
  larger, and an answer about what the code could do rather than what is on
  screen. This is the highest-leverage text in the project, because it is what
  a client puts in front of the model before it decides anything, and a test
  asserts it keeps saying so.


- `HitMap::register_blocking` and `Frame::register_barrier`, for a widget
  that must take the input it covers.


- The AccessKit tree carries which node has focus. A screen reader announces
  the focused node and nothing else, so the tree the bridge published was a
  list that could be read to a user but not walked — the same defect as Tab
  doing nothing, seen from the other side. `accesskit_bridge::publish` takes
  the widget the ring has just moved to and asks egui for focus on it, on the
  frame it moves and not on every frame after.


- **Keyboard focus works.** Tab and Shift+Tab walk the interactive widgets in
  render order, Enter and Space press the focused one by the same path a click
  takes, clicking a widget focuses it, and the runtime draws a focus ring. All
  three hosts drive it through `focus::handle_key` and `focus::draw_ring`, so
  an agent can Tab through an interface exactly as a person does.
  `FocusManager` had shipped as "Focus Management — Ring-buffer tab
  navigation" with nothing calling it, so pressing Tab did nothing at all.
- The ring is rebuilt from the hit map after every frame, so a widget is
  focusable exactly when it is interactive and has an id — there is no second
  registration to fall out of step with, and a `Label` is not a stop. The
  indicator is drawn centrally rather than by each widget: 29 widgets would be
  29 chances to forget, and a widget that forgot would be invisibly
  unreachable.
- `HitMap::focusables` and `HitMap::bounds_of`; `HeadlessDriver::focused_id`
  and `HeadlessDriver::painted`.


- `Program::with_agent` serves the agent protocol on stdin/stdout while the
  window is open. `RpcTransport`, the WebSocket transport and the MCP server
  each own the model, and so does `Program::run`, so an application was
  agent-driven or windowed and never both — the premise the project is built
  on held only if you picked one. A reader thread parses lines and hands each
  request to the frame loop, which answers it between frames with the model it
  is showing; an action an agent takes is drawn on that same frame.
- `agent::rpc::RequestSink`, `ChannelSink`, `answer_job` and `serve_stdio`.
  The reading, the line cap and the rate limit are the same whether or not
  there is a window, so they are now one loop rather than a second copy — the
  last two copies of that loop both answered `execute_action` with a
  `log::debug!`.
- `HeadlessDriver::model_mut`, `ontology_mut`, `reregister_ontology` and
  `set_window_size`, which is what the windowed backend needs to keep the
  driver in step with the window it is showing.


- `Program::with_plugin`, and `Model::plugins_ready`, which hands an
  application what its plugins contributed. See below: the plugin system had
  never run under the default backend.
- `plugin::initialise`, the shared initialisation both backends call. It is a
  free function because a backend that opens a window cannot be driven by a
  test, which is how the agpu version came to discard two of its three outputs
  unnoticed.

- `Command` gained eight window operations — `SetWindowVisible`,
  `FocusWindow`, `MinimiseWindow`, `SetWindowPosition`, `SetWindowSize`,
  `SetAlwaysOnTop`, `SetFullscreen` and `SetWindowTitle`. `Model::update` gets
  no `egui::Context` by design, and `Command` had no window operation but
  `Quit`, so "click the tray icon, show the window" could not be expressed at
  all. The egui backend queues them onto `ViewportCommand` and flushes them at
  the top of the next frame, before the running check, so a model that hid its
  window is not raced by one that quit; the agpu backend carries them out
  against its winit window immediately.
- `ProgramOptions` gained `always_on_top`, `decorated`, `position`, `min_size`
  and `max_size`. These existed in `WindowConfig` and never reached a window.

- `Validate { strict: true }` promotes every warning to an error and adds
  `unwired_widget`: a widget that publishes actions and has a handler for none
  of them. That stays silent by default, because answering through
  `Model::execute_action` is a different style rather than a fault — and it is
  also exactly how `Canvas`, `Chart` and `RichText` came to accept `clear` and
  do nothing. The test is not that strict passes; it is that every defect from
  this week fails it.
- `unreadable_text`, which computes the WCAG contrast ratio of painted text
  against what is behind it and reports what no structural check can see: a
  label painted white on white validates perfectly and cannot be read.
- `positional_id`, which reports agent ids naming a position rather than a
  thing (`button_3`, `row_0`), since such an id stops addressing the same
  widget the moment anything is inserted above it.

- The AccessKit bridge publishes the ontology to the platform accessibility
  tree, behind the `accesskit` feature, so a screen reader and an agent read
  the same interface. Widget text is taken from the `label`, `text`, `title`
  and `placeholder` properties rather than from a `label` field alone, which is
  where the widgets actually store it.

- MCP exposes `validate`, and `get_tree` accepts and declares `since`. Both
  were in the protocol and tested through the headless driver, and neither
  could be called by the client they were built for. `screenshot`'s `format`
  documents that `text` returns the golden-comparable rendering.

- `ontology::builtin::register_all` registers all 29 widget schemas in one
  call, with a module-count guard so a new widget module cannot be added
  without it.

- `scripts/check.sh` runs what CI runs, in fail-fastest order; `--all` adds the
  sibling `agpu` crate and both benchmark workspaces, which live outside this
  workspace and which `cargo check` here never compiles.

- Handlers on the remaining common interactive widgets, each bound to the
  action it advertises: `List::on_select`, `Select::on_select`,
  `Tabs::on_select`, `Table::on_select`, `TextArea::on_input`,
  `Splitter::on_change`, `Toolbar::on_item`, `Menu::on_item` and
  `Radio::on_select`. An application driven by any of these writes no
  `execute_action` handler.

- `TextInput::on_input(id, |model, text| ...)` and
  `Slider::on_change(id, |model, value| ...)` carry the change a value widget
  makes, including the new value. It arrives the same way whether a person
  edited the widget or an agent sent
  `execute_action(id, "set_text", {"text": ...})`, so an application writes no
  handler for it. The TodoMVC benchmark now has no `execute_action` at all.
- `runtime::ValueMutation<M>`, the type those widgets carry.

- `Button::on(id, |model| ...)` and `Checkbox::on(id, ...)` let a widget carry
  the change it makes instead of a message. The Elm loop asks for a message
  type and an `update` arm per message, which is the right shape for real state
  transitions and the wrong shape for a button that adds one to a number. An
  application can now have no `Msg` variants at all, and such a widget is
  driven by an agent exactly as a message-carrying one is. `action` remains for
  changes that must return a `Command`.
- `runtime::Mutation<M>`, the type a widget carries for the above.
- `Rect::rows_of`, `cols_of`, `split_rows` and `split_columns` for the common
  splits that previously needed a `Layout` and a named `Constraint` per band.

- `UiTree::snapshot` and `HeadlessDriver::snapshot` render the interface as
  stable text, with properties in sorted order and bounds rounded to whole
  pixels, so two renders of one interface are byte-identical. An agent can keep
  one as a golden file and diff against it to prove a change did what it
  intended and nothing else — the assertion a pixel diff cannot make, since it
  cannot say *what* moved. Also available over the protocol as
  `screenshot` with `format: "text"`.

- `AgentRequest::Validate` and `HeadlessDriver::validate` check a rendered
  interface for structural faults an agent cannot see from a screenshot:
  widgets that rendered without an id (unclickable and unaddressable, though
  they look correct), duplicate agent ids, zero-size bounds, and offscreen
  bounds. Returns `ontology::Diagnostic` values carrying a stable code, the
  widget id and type, and how to fix it. This is how an agent confirms the GUI
  it just scaffolded is operable rather than merely rendering.
- `Frame::note_unaddressable` / `take_unaddressable`, which is how `Button` and
  `Checkbox` report themselves when rendered with no id — such a widget never
  reaches the UI tree, so it cannot be noticed afterwards.

- `Button::action(id, msg)` and `Checkbox::action(id, msg)` wire a widget for a
  person and an agent in one call. The runtime routes a mouse click through the
  hit map to the message, and an agent's `execute_action(id, "click")`
  dispatches the same one, so an application no longer writes a
  `Model::execute_action` arm for ordinary buttons. Cuts the agent-driveable
  TodoMVC by 16% (1612 → 1357 tokens); the premium for being agent-driveable at
  all fell from +37% to +13%, and from +36% to +2% on a counter. Costs one
  boxed message per interactive widget per frame.
- `Rect::rows(height)` and `Rect::columns(width)` yield successive strips of a
  rectangle, replacing the hand-written cursor and overflow check that list
  rendering needed.

- `benches/scaffold` — canonical counter app implemented in Dewey (with and
  without agent affordances), egui, and iced, with `measure.py` reporting what
  an agent must write and how long it waits for `cargo check`.
- TodoMVC added to `benches/scaffold` as the complex canonical app, in all four
  variants, plus an `agent_task` benchmark driving a nine-step agent workflow
  against it: 51.9 µs end to end, ≈19,000 runs per second, asserted before it
  is timed.
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

- The MCP description of `batch_actions` still said "atomically". The protocol
  reference stopped claiming that when it turned out a failing entry did not
  even stop the ones after it; the same claim survived in the one place a
  coding agent actually reads. No tool description may promise atomicity, and
  a test enforces it.


- **A modal dialog blocked nothing.** `Modal` dimmed what was behind it and
  registered no bounds at all, so a click went straight through the backdrop
  and pressed the button underneath — and once Tab worked, it walked into the
  widgets the dialog was covering. The roadmap listed the widget as "dialog
  overlay with backdrop dimming and input blocking". It now registers a
  barrier over its area: everything drawn before it stops being clickable and
  stops being a focus stop, and everything drawn after it — the dialog's own
  buttons — behaves normally. A closed dialog blocks nothing, which is
  asserted, or the test for the open case would prove nothing.


- `Command::AgentAction` was a `log::debug!` and nothing else under the
  default backend — the same line, in the same position, as the one that left
  both network transports unable to act. A model returning it to drive one of
  its own widgets reached the widget under agpu and reached nothing here. It
  now dispatches through the handlers and validates parameters against the
  ontology, as agpu does. The existing command-parity test passed throughout,
  because it asks whether each backend *mentions* every variant, and
  mentioning it is exactly what the log line did.


- **Clicking a widget did nothing under the default backend.** It converted a
  mouse click into an `Event::Mouse`, handed it to `Model::handle_event`, and
  stopped: it never called `HitMap::hit_test` and held no `Handlers` at all.
  `Button::action`, `Button::on`, `Checkbox::on`, `TextInput::on_input`,
  `Slider::on_change` and the nine other widget handlers were therefore inert
  under the backend `Program::run` uses — the one the README's quick start
  runs on. The same wiring worked through the headless driver, so every test
  passed, and under `agpu`, which is opt-in and off by default. The default
  backend now hit-tests a click and activates the widget under it.

- All three hosts activate a widget through the new
  `Handlers::apply_primary`. Each had its own copy of "look up the primary
  action, then apply it", and a divergent third copy is how a handler came to
  be bound to the wrong action name earlier in this cycle.


- **The plugin system existed only under a backend that is not the default.**
  It shipped as a v1.2 framework feature and the lifecycle was driven solely by
  `AgpuProgram`, which is opt-in and off by default. `Program` had no
  `with_plugin` at all, so under the backend nearly everyone uses a plugin
  could not be registered, `init` / `on_frame` / `on_shutdown` were never
  called, and a plugin's ontology registrations never reached an agent. Both
  backends now drive it, and a test asserts both do — neither opens a window in
  a test, which is why nothing noticed.

- A plugin's theme and message-catalogue contributions were dropped as soon as
  `init` returned. The agpu backend built an `I18n` and a `Theme`, lent them to
  every plugin, and let both fall out of scope at the end of the block; only
  the ontology survived, because that borrow outlived it. Two of the four
  contributions the module advertises were discarded before the first frame.
  `initialise` returns them and both backends pass them to
  `Model::plugins_ready`.

- **The stdio and WebSocket transports could not act.** `RpcTransport` and
  `WsTransport` each carried their own copy of the request loop, written before
  `HeadlessDriver` grew most of what it knows. Both turned an `execute_action`
  into a `Command::AgentAction`, and both command loops answered it with a
  `log::debug!`. An agent connected over stdio or a WebSocket could read the
  interface and change nothing: no handler dispatch, no `Model::execute_action`,
  no version, no conditional `get_tree`, no `validate`, no refusal of an
  unadvertised action. Every feature of the preceding week existed only for the
  in-process driver. Both are now a frame around `HeadlessDriver`: they own the
  socket, the line cap and the rate limit, and the driver owns what a request
  means.

- `batch_actions` called `Model::execute_action` directly and never reached the
  handler a widget registers - the path every widget builder here produces — so
  a batch against a normal interface reported success and changed nothing. Each
  entry now takes the same path a single `execute_action` does. The protocol
  also called batches atomic: nothing rolled back, and a failing entry did not
  stop the ones after it, so an agent got a success and a half-applied change
  with no way to tell which half. A batch now stops at the first failure and
  reports `applied` and `failed_at`; it is still not atomic, and the reference
  says so and says why. A batch entry naming a widget that does not exist was
  also skipped in silence while a single `execute_action` refused it; the two
  now agree.

- `subscribe` accepted any event name at all, so an agent could subscribe to
  `render_update`, hold a success, and wait for something nothing was ever
  going to emit. Undeliverable names are refused with the list of what does
  arrive. `app_quit` is now delivered, announced once. A request over the cap
  of 100 names previously took as many as fit and returned success, subscribing
  an agent to some of what it asked for with no way to tell which; it is now a
  refusal naming the limit.

- `negotiate` reported `compatible: false` inside a *successful* response, so an
  incompatible client sailed past its own handshake. It now fails, with the
  version range in the error and `compatible` still in the data.

- `SERVER_CAPABILITIES` had not moved since before `validate`, strict
  validation, the tree viewport, conditional reads and the AccessKit bridge
  existed. A handshake that does not mention a feature is a feature that goes
  unused.

- An agent-injected `Resize` left the driver's own `window_size` unchanged, so
  the next `get_tree` rendered against the old window and `validate` measured
  `offscreen_widget` against it too. An agent testing a responsive layout was
  given answers about a window that had never changed.

- `Event::Resize` was emitted on every frame by the default backend, because
  `screen_rect` is set every frame whether or not it changed; it fired sixty
  times a second. It is now change-detected.

- The default backend emitted five of the twelve event kinds, and never
  delivered a dropped file at all. Both backends now emit all twelve, including
  `FileDrop`, `FileHover` and `FileHoverCancelled`, pinned by a backend-parity
  test.

- `ProgramOptions::fullscreen` was honoured by agpu and silently dropped by the
  default backend; the five newer window options were added to the default
  backend and not to agpu. The parity test now covers both directions.

- `agpu` advertised a complete ontology and registered none of it.
  `register_gpu_ontology` registers the five GPU schemas that were written and
  never wired up. Its `multiwindow`, `accessibility` and `plugin` modules now
  say plainly that nothing in the crate drives them.

- `Checkbox`'s handler answered `click`, but its ontology advertises `toggle`.
  An agent following the ontology called the name the widget published and
  nothing happened. Handlers are now bound to the advertised action, a mouse
  click fires whichever action the widget registered, and `validate` reports an
  `unadvertised_action` when the two disagree — the check exists because this
  shipped.

- A widget handler answers for its own action only. Dispatch previously fired
  on any `click`, so once value widgets existed an unrelated action could have
  run a text field's `set_text` handler with no value and silently cleared it.
  Handlers are now registered with their action name and matched against it.

- Mouse clicks now route to widgets. `HitMap::hit_test` was never called
  anywhere: Dewey built a hit map every frame and discarded it, so every
  application had to store widget rectangles and compare coordinates by hand in
  `handle_event`.

- Corrected the comparative performance table. The first version was measured
  with criterion on a machine at 100% CPU and overstated egui's cost by ~2× and
  iced's by ~3× at 5000 rows; it also ranked 3000 rows slower than 5000 rows.
  Re-measured with the interleaved minimum-time harness.

### Changed

- The default backend holds its application inside a `HeadlessDriver` rather
  than owning the model directly. Nothing about an application changes; it is
  what lets the same code answer an agent with or without a window, instead of
  the fourth copy of request handling being written for the windowed case.


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

### Documentation

- `examples/agent_demo.rs` told the reader to pipe three JSON Lines requests
  to a window that reads no stdin, and all three were in a format the protocol
  has never accepted: externally tagged, with a numeric `id`, naming
  `GetWidgetState` and `PerformAction`, neither of which exists. Doc
  conformance now reads every example's header as well as the README and the
  protocol reference — the worst protocol documentation in the repository was
  the only kind nothing checked.

- The README claimed "Focus Management — Ring-buffer tab navigation" and
  "Overlay System — Modal dialogs and overlay stacking". Both name real types
  that nothing drives; the entries now say what they are and what an
  application still has to do.


- Two more modules said they did a job nothing asks them to do. `focus`
  offered "ring-based Tab/Shift+Tab keyboard navigation" and no backend
  registers a widget in the ring, nothing routes Tab to `focus_next`, and no
  widget draws a focus indicator — pressing Tab in a Dewey application does
  nothing. `overlay` offered a stack "rendered above the main UI" and no frame
  renders one, so a pushed overlay appears nowhere and blocks nothing; the
  `Modal` widget draws its own backdrop and does not go through it. Both now
  say so, are `[~]` on the roadmap, and are covered by the reachability test.
  Agents are unaffected by the focus gap, since an agent addresses a widget by
  id rather than tabbing to it. Keyboard users are not.


- `Event::DragDrop` says that no backend emits one. The agpu backend converts
  an `agpu::Event::DragDrop`, and nothing in the agpu crate ever constructs
  one; the default backend has no drag-drop path; the agent protocol cannot
  inject one. `handle_event` is therefore never called with it, and the types
  are vocabulary for an application tracking a drag out of mouse events
  itself. File drops are a different event and do work on both backends.


- Three modules described themselves doing a job they do not do. `memory`
  offered "arena-based allocation for per-frame temporaries" and no allocation
  in Dewey goes through it; `gpu` offered a batch that "minimises draw calls"
  and nothing builds or submits one; `theme` said tokens let widgets be
  "theme-switched at runtime" when no widget reads an ambient theme at all.
  Each now says plainly that nothing in this crate drives it, and the roadmap
  marks all three `[~]`. The reachability test grew a third state for exactly
  this: a working implementation nobody calls, which is neither driven nor
  types-only.


- The README's headline protocol example — the one showing that any language
  able to write lines of JSON can drive a Dewey application — never parsed. It
  used a numeric `id` where the envelope takes a string, externally-tagged
  requests where the protocol is internally tagged, and three request names
  that never existed. It is the first thing a reader sees and the last thing
  anything checked; `tests/docs_conformance.rs` now deserialises every JSON
  block in the README and the protocol reference, and compares documented
  responses against what the server actually sends.

- The README quick start did not compile: it returned
  `Result<(), eframe::Error>`, where `Result` in this crate's prelude is
  Dewey's own one-parameter alias, and it declared two messages and sent
  neither. It now lives in `examples/quickstart.rs`, which cargo compiles on
  every build, and a test asserts the README block and the file are the same
  text.

- All three ignored doctests were hiding code that does not compile. The two
  `Widget` trait examples called `Painter::draw_text`, which has never existed,
  and passed a two-argument `fill_rect` that takes three; the web backend's
  example imported and called `WebRunner`, a type nobody ever wrote. A test now
  refuses a doctest marked `ignore` without a stated reason.

- The protocol reference's handshake example showed `min_version` where the
  field is `min_protocol_version`, omitted `compatible` and
  `supported_capabilities`, and listed five server capabilities where there are
  ten.

- Recorded the `wgpu` validation warning, its `wgpu_hal=off` workaround, and
  why the eframe pin cannot move: wgpu-hal 30 needs `windows 0.62` and agpu
  pins wgpu 24, which needs `windows 0.58`.

- `CONTRIBUTING.md` said to branch from and open pull requests against `main`.
  The branch is `master` — the same mismatch meant the CI workflow triggered on
  a branch that does not exist, so CI had never run.

### Internal

- CI runs. It was configured to trigger on `main` in a repository whose branch
  is `master`, and had never executed; the crate did not build on Linux when it
  was first switched on. The workflow now covers the sibling `agpu` crate and
  both benchmark workspaces, which are excluded from the package and so were
  never compiled by anything, and gates on allocation budgets and on three
  benchmarks that assert their own correctness before they time anything.
- New standing checks, each verified by breaking the thing it catches: strict
  validation, reachability in both crates, backend parity, protocol property
  tests, an example audit, and doc conformance.

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
