# Agent Protocol Specification

Version: 2 (backward-compatible with v1)

## Transport

Messages are exchanged as **JSON Lines** — one JSON object per line, delimited
by `\n`. Two transport options are available:

| Transport    | Feature flag   | Description                                                                             |
| ------------ | -------------- | --------------------------------------------------------------------------------------- |
| stdin/stdout | (always)       | Default. Agent writes requests to the application's stdin, reads responses from stdout. |
| WebSocket    | `ws-transport` | Optional. Bidirectional over a single WebSocket connection.                             |

## Framing

Every request is wrapped in a `RequestEnvelope`:

```json
{"id": "req-1", "request": {"type": "ping"}}
```

The `id` field is optional. When present, the response echoes it back for
correlation.

## Handshake (Negotiate)

On connect, the agent should negotiate capabilities:

```json
{"id": "1", "request": {"type": "negotiate", "client_version": 1, "capabilities": ["batch_actions", "state_diffs"]}}
```

Response:

```json
{"success": true, "id": "1", "data": {"protocol_version": 2, "min_version": 1, "server_capabilities": ["state_diffs", "batch_actions", "screenshot", "ws_transport", "protocol_v2"]}}
```

## Request Types

### ping

Health check / keepalive.

```json
{"type": "ping"}
```

Response: `{"success": true, "data": {"status": "pong", "framework": "dewey", "version": "1.0.0"}}`

### quit

Request graceful application shutdown.

```json
{"type": "quit"}
```

### query_ontology

Discover available widget types. Optional filters:

```json
{"type": "query_ontology", "query": "button", "role": "Action"}
```

Returns an array of matching `WidgetSchema` objects.

### get_schema

Get the full schema for a specific widget type.

```json
{"type": "get_schema", "widget_type": "Button"}
```

### get_tree

Get the current UI tree snapshot — a hierarchical representation of all
widgets and their states.

```json
{"type": "get_tree"}
```

Every reply carries a `version`. Pass the one you last saw as `since` and an
unchanged interface answers `{"unchanged": true, "version": N}` instead of
resending an identical tree:

```json
{"type": "get_tree", "since": 12}
```

This is the difference between a re-poll costing a render and a serialisation
and one costing a comparison — 100 ns against roughly 5 µs for a 100-row
interface, at 30 bytes instead of 40 kB. Re-polling is the commonest thing an
agent does, so it is worth threading the version through.

Pass `viewport` to be described only the widgets whose bounds intersect a
rectangle — the region the agent can actually see:

```json
{"type": "get_tree", "viewport": {"x": 0, "y": 0, "width": 480, "height": 800}}
```

The reply then carries `total_nodes` and `shown_nodes`, so a short list is
distinguishable from a window onto a long one. Without it the tree describes
every widget including those scrolled out of view: at 1000 rows that is 401 kB
against 11.7 kB clipped, and against 16.7 kB for a screenshot of the same
window.

Clipping happens after the frame is built, so it reduces what the agent reads
rather than what the framework builds — the reply's size stays flat as the list
grows but the time to produce it does not. A list long enough for that to
matter wants `VirtualList` in the view.

### validate

Check the rendered interface for structural faults — the ways a GUI can be
broken while compiling, rendering, and looking correct.

```json
{"type": "validate"}
```

```jsonc
{"ok": false, "errors": 1, "diagnostics": [
  {"severity": "error", "code": "unaddressable_widget", "widget_type": "Button",
   "message": "1 `Button` widget(s) rendered without an id, so they are not
               hit-testable and no agent can act on them; give each one
               `.action(id, msg)`"}]}
```

Codes:

| code | severity | meaning |
| ---- | -------- | ------- |
| `unaddressable_widget` | error | an interactive widget rendered with no id: it has no hitbox, no tree node, and nothing to name |
| `duplicate_agent_id` | error | two widgets share an id, so an action naming it is ambiguous |
| `zero_size_widget` | error | bounds with no area — it cannot be seen or clicked |
| `offscreen_widget` | warning | bounds outside the window |
| `unadvertised_action` | error | a handler is bound to an action its widget does not publish, so an agent following the ontology would call a name that does nothing |
| `unhandled_action` | warning | a widget wired for some of its actions accepts the rest and silently ignores them |
| `unreadable_text` | error | text drawn at a WCAG contrast below 1.6 against the fill behind it |
| `positional_id` | warning | sibling ids are numbered by position, so removing one renames the rest |

Pass `strict` when the application is meant to be driven unattended:

```json
{"type": "validate", "strict": true}
```

Strict promotes every warning to an error and adds one code that is otherwise
silent:

| code | meaning |
| ---- | ------- |
| `unwired_widget` | the widget publishes actions and has a handler for none of them |

That is legitimate by default — an application may answer through
`Model::execute_action` instead — and it is also exactly how `Canvas`, `Chart`
and `RichText` came to accept `clear` and do nothing.

`validate` reports structure, and one thing about appearance: text drawn
against a flat fill it cannot be read against. It does not check contrast
against a gradient or an image, overlapping widgets, or whether the layout is
any good — it is not a substitute for looking at the interface.

### get_state

Get the state of a specific widget by its `agent_id`.

```json
{"type": "get_state", "agent_id": "counter_label"}
```

### execute_action

Invoke an action on a specific widget.

```json
{"type": "execute_action", "agent_id": "inc_btn", "action": "click", "params": {}}
```

### inject_event

Inject a synthetic event into the application event pipeline.

#### Key press

```json
{"type": "inject_event", "event": {"kind": "key", "code": "+", "modifiers": []}}
```

#### Mouse click

```json
{"type": "inject_event", "event": {"kind": "mouse_click", "x": 100, "y": 200, "button": "left"}}
```

#### Mouse move

```json
{"type": "inject_event", "event": {"kind": "mouse_move", "x": 150, "y": 250}}
```

#### Mouse scroll

```json
{"type": "inject_event", "event": {"kind": "mouse_scroll", "x": 100, "y": 200, "delta_x": 0, "delta_y": -3}}
```

#### Text input

```json
{"type": "inject_event", "event": {"kind": "text_input", "text": "Hello"}}
```

#### Window resize

```json
{"type": "inject_event", "event": {"kind": "resize", "width": 1280, "height": 720}}
```

### subscribe

Subscribe to server-pushed events. Available event types:
- `state_changed` — widget state changes (diff-only)
- `render_update` — UI tree changes
- `action_result` — action completion results

```json
{"type": "subscribe", "events": ["state_changed"]}
```

### unsubscribe

```json
{"type": "unsubscribe", "events": ["state_changed"]}
```

### screenshot

Capture a snapshot of the current frame. Currently returns the UI tree in
JSON format.

```json
{"type": "screenshot", "format": "json"}
```

### batch_actions

Execute multiple actions in a single request:

```json
{
  "type": "batch_actions",
  "actions": [
    {"agent_id": "btn_a", "action": "click", "params": {}},
    {"agent_id": "slider_1", "action": "set_value", "params": {"value": 50}}
  ]
}
```

Response includes a `results` array with one entry per action.

## MCP

The same requests are available as MCP tools, one per request type, including
`validate` and `get_tree`'s `since`. See `McpServer`.

## Response Format

All responses share a common shape:

```json
{
  "success": true,
  "id": "req-1",
  "data": { ... },
  "error": null
}
```

| Field     | Type             | Description                                    |
| --------- | ---------------- | ---------------------------------------------- |
| `success` | `bool`           | Whether the request succeeded                  |
| `id`      | `string \| null` | Echoed request ID (if provided)                |
| `data`    | `object \| null` | Response payload                               |
| `error`   | `string \| null` | Error message (only when `success` is `false`) |

## Server-Pushed Events

Subscribe with `{"type": "subscribe", "events": ["state_changed"]}` and the
transport writes an event on the same stream — a further JSON line over stdio,
a further message over a WebSocket — after any request that changed something.
An event names the widget and carries its new state, so an agent does not have
to re-read the tree to find out what happened:

```json
{"type": "state_changed", "agent_id": "readout", "state": {"text": "count 1"}}
```

Only widgets whose state actually differs are reported, and a subscribed
session re-renders once per change rather than once per request. Nothing is
computed at all when nothing is subscribed.

When subscribed, the server pushes `AgentEvent` objects:

```json
{"type": "state_changed", "agent_id": "counter_label", "state": {"text": "Count: 5"}}
{"type": "render_update", "tree": { ... }}
{"type": "action_result", "agent_id": "btn_a", "action": "click", "result": {"clicked": true}}
{"type": "app_quit"}
{"type": "error", "message": "Widget not found: xyz"}
```

## Semantic Roles

Widgets are classified by role for ontology queries:

| Role         | Widgets                                                      |
| ------------ | ------------------------------------------------------------ |
| `Action`     | Button, Toolbar                                              |
| `Input`      | TextInput, TextArea, Slider, Select, ColorPicker, DatePicker |
| `Display`    | Label, ProgressBar, Image, Chart, RichText                   |
| `Toggle`     | Checkbox, Radio                                              |
| `Container`  | Container, Panel, Scroll, Splitter, Modal                    |
| `Navigation` | Menu, Tabs, CommandPalette                                   |
| `Data`       | List, Table, Tree, VirtualList                               |
| `Feedback`   | Tooltip                                                      |
| `Drawing`    | Canvas                                                       |
