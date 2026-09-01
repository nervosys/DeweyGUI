# Agent Scaffolding Benchmarks

How expensive is it for an *agent* to build and then verify a GUI program in
each framework? Two costs, measured separately:

1. **Scaffolding** — what the agent has to write, and how long it waits to find
   out whether it compiles.
2. **Closing the loop** — whether the agent can then confirm the program works,
   and how fast.

```bash
cd benches/scaffold && python measure.py           # scaffolding cost
cd benches/comparative && cargo run --release --bin agent_loop   # counter loop
cd benches/scaffold   && cargo run --release --bin agent_task    # todomvc task
```

## The canonical programs

Two, because a counter is too small to show how a framework scales:

- **counter** — a 400×200 window showing `Count: N` above three buttons,
  Decrement / Reset / Increment, wired to the state.
- **todomvc** — the standard cross-framework complex app: a text input and Add
  button, a filter row (All / Active / Completed), a list of items each with a
  toggle checkbox and a delete button, an "N items left" counter, and Clear
  completed.

Each is implemented four ways — `*_dewey_plain` (no agent affordances),
`*_dewey` (agent-driveable), `*_egui`, `*_iced` — all idiomatic and minimal.

## 1. Scaffolding cost

`check` is `cargo check` after touching the file, warm deps, best of three —
the latency of one step in an agent's edit → compile → fix loop.

Comments are stripped before counting: they are prose for a human reader of
this benchmark, and counting them once made a strictly smaller program score
higher than the one it was derived from.

**counter**

| Framework     | Code lines | ~Tokens | vs egui | Was    |
| ------------- | ---------- | ------- | ------- | ------ |
| Dewey (plain) | 39         | 393     | 1.49×   | —      |
| Dewey (agent) | 41         | 400     | 1.52×   | 1.84×  |
| **egui 0.31** | **33**     | **264** | 1.00×   |        |
| iced 0.13     | 37         | 268     | 1.02×   |        |

**todomvc**

| Framework     | Code lines | ~Tokens  | vs egui | Was    |
| ------------- | ---------- | -------- | ------- | ------ |
| Dewey (plain) | 121        | 1196     | 1.86×   | —      |
| Dewey (agent) | 133        | 1357     | 2.11×   | 2.51×  |
| **egui 0.31** | **85**     | **643**  | 1.00×   |        |
| iced 0.13     | 110        | 788      | 1.23×   |        |

**Dewey still loses this half, but by less, and agent-driveability is now
nearly free.** `Button::action` and `Checkbox::action` cut the agent-driveable
counter 18% (487 → 400 tokens) and TodoMVC 16% (1612 → 1357). The premium an
app pays to be agent-driveable at all fell from +36% to **+2%** on the counter
and from +37% to **+13%** on TodoMVC — what remains is almost entirely the ids
on read-only labels, which exist so an agent can read values back.

A correction to earlier numbers in this file: the previous `*_dewey_plain`
samples set no `agent_id`, and a Dewey widget with no id registers no hitbox —
so those buttons did not work. They were measuring a program that could not be
clicked. The plain samples now keep ids on interactive widgets, which is what a
working Dewey app requires, and their token counts rose slightly as a result.
That is also the reason the agent premium is now so small: **in Dewey, wiring a
button so a person can click it is the same work as wiring it so an agent can.**

`cargo check` latency measured 2.4–4.2 s for all four, with the ranking
flipping between runs on a loaded machine; it does not discriminate at this
size and is not reported.

Where the remaining extra goes:

- **The Elm architecture is not free to write.** A `Msg` enum plus an `update`
  arm per message is real code that egui's immediate mode does not need — it
  mutates `self.count` inline at the click site.
- **Layout is explicit.** Dewey splits rectangles by constraint; egui and iced
  infer flow from widget order.
- **Agent affordances are now +2% / +13%**, down from +36% / +37%: just ids on
  read-only labels, plus a four-line `execute_action` for the text field, which
  carries state rather than a message.

Two framework changes produced the drop:

- **`Button::action(id, msg)` / `Checkbox::action(id, msg)`** — one call wires a
  widget for a person *and* an agent. The runtime routes a mouse click through
  the hit map to the message, and an agent's `execute_action(id, "click")`
  dispatches the same one. This deleted TodoMVC's entire dispatch handler,
  including the `toggle_0` / `delete_3` string-matching and index parsing.
- **`Rect::rows(h)` / `Rect::columns(w)`** — an iterator of successive rows,
  which removed the manual `y` cursor and overflow break from list rendering.

It also closed a functional hole: `HitMap::hit_test` was never called anywhere
in the codebase. Dewey built a hit map every frame and threw it away, so every
application had to route clicks itself by storing rectangles and comparing
coordinates in `handle_event`.

## 2. Closing the loop

This is the half the numbers above do not capture. Having written the program,
can the agent tell whether it works?

Dewey, over the agent protocol, headless — no window, no GPU, no screenshot:

```
closed-loop check: clicked inc, state now {"agent_id":"count", …,
                   "state":{"text":"Count: 1"}, "widget_type":"Label"}

1. discover    (get_tree)                         5.8 µs
2. understand  (query_ontology)                   1.1 µs
3. read schema (get_schema Button)                1.1 µs
4. act         (execute_action inc.click)         1.4 µs
5. verify      (get_state count)                  2.1 µs
--------------------------------------------------------
full discover→act→verify loop                    11.9 µs
```

**≈84,000 complete agent loops per second, single-threaded.**

`action` is not free: it boxes the message, costing one allocation per
interactive widget per frame (6.0 → 7.0 per row in the allocation benchmark,
about 90 bytes). Avoiding the box would mean making `Frame` generic over the
application's message type, which would change every widget signature in the
library. The box buys dispatch that previously did not exist at all.

On TodoMVC, a realistic nine-step task — add two items, complete one, switch
filter, and read the result back (`cargo run --release --bin agent_task`):

```
task verified: 2 added, 1 completed, filter=active, footer reads "1 items left"

discover        (get_tree)                   10.2 µs
type item 1     (set_text)                    2.8 µs
add item 1      (click add)                   2.7 µs
type item 2     (set_text)                    3.7 µs
add item 2      (click add)                   3.5 µs
complete item 1 (click toggle_0)              4.4 µs
filter active   (click filter_active)         4.4 µs
re-read tree    (get_tree)                   14.7 µs
verify counter  (get_state remaining)         4.3 µs
----------------------------------------------------
full 9-step task                             44.7 µs
```

Polling costs almost nothing once the agent passes the version it last saw:

```
polling an unchanged screen, interleaved, min of 2000:
  get_tree                   11.0 µs
  get_tree since=version      100 ns   (110x less)
```

Waiting for a screen to change is the common agent pattern, and it used to be
the most expensive request in the protocol.

An agent can additionally ask `validate` whether the interface is operable at
all — it reports id-less widgets that cannot be clicked, duplicate ids, and
zero-size or offscreen bounds, which is the class of fault that renders
perfectly and fails silently.

**≈22,000 complete task runs per second**, up from 51.9 µs and ≈19,000 before
the dispatch work — the same task now routes through widget messages rather
than a hand-written handler. Both harnesses assert the workflow
before timing it — the TodoMVC one checks that the completed item really has
disappeared from the tree under the Active filter and that the footer reads
`1 items left`, so it cannot degenerate into timing a no-op.

**egui and iced have no equivalent to measure.** Neither exposes a widget tree,
a typed action, or a readable state snapshot to an external process. An agent
verifying a counter written in either must:

1. launch a real window (GPU, display server, or a virtual one in CI),
2. drive it by synthesising OS-level input at guessed pixel coordinates,
3. screenshot it, and
4. ask a vision model what the label says.

That is not a slower version of the same loop — it is a different loop with a
different failure mode. Step 4 returns a *probabilistic reading of an image*
where `get_state` returns `"Count: 1"`. An agent can assert on the second and
only guess at the first. The honest comparison is therefore not a ratio; it is
that one column exists and the other does not.

## What this says

Dewey is worse at being *written* by an agent and uniquely good at being
*verified* by one. Whether that trade is worth it depends on how many
iterations the agent needs: verification cost is paid on every loop, and an
agent that cannot check its work reliably does more loops.

Two framework changes would close most of the scaffolding gap. Neither is
implemented; both are design changes rather than benchmark changes.

1. **Let widgets carry their message**: `Button::new("+").on_click(Msg::Add)`
   would delete the whole `execute_action` dispatch — including TodoMVC's
   `toggle_0` string parsing — and make agent-driveability close to free
   instead of +33% code.
2. **A flow layout for lists**: something like `Layout::stack(28.0)` yielding
   successive rows would remove the manual rectangle arithmetic that makes
   Dewey's disadvantage grow with app size.

## Caveats

- Token counts are a lexer-level proxy (identifiers, numbers, punctuation), not
  a specific model's tokeniser. Use the ratios, not the absolutes.
- This measures the *machine-side* costs an agent pays. It does not measure
  model thinking time, how many iterations each framework's API tends to need,
  or how well any model knows each library — all of which need real agent runs
  to answer, and none of which this harness attempts.
- `cargo check` latency was measured (interleaved, minimum of four rounds after
  touching each file) but is not reported: at 2.4–4.2 s for every framework the
  differences were inside machine noise and the ranking was not stable.
- The `agent_task` binary duplicates the model from `todo_dewey.rs` so that the
  scaffold-metric file stays a self-contained application; they are kept
  identical by hand.
