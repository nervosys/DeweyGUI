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

The tree describes every widget, including those scrolled out of view; there is
no viewport or paging. For a very long list the reply is correspondingly large.

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

`validate` reports structure, not appearance. A label painted white on white
passes it.

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
