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

**counter**

| Framework     | Code lines | Bytes   | ~Tokens | vs egui |
| ------------- | ---------- | ------- | ------- | ------- |
| Dewey (plain) | 39         | 1455    | 425     | 1.55×   |
| Dewey (agent) | 52         | 1685    | 495     | 1.81×   |
| **egui 0.31** | **33**     | 1021    | **274** | 1.00×   |
| **iced 0.13** | 37         | **970** | 276     | 1.01×   |

**todomvc**

| Framework     | Code lines | Bytes    | ~Tokens  | vs egui |
| ------------- | ---------- | -------- | -------- | ------- |
| Dewey (plain) | 140        | 4641     | 1245     | 1.90×   |
| Dewey (agent) | 186        | 6197     | 1648     | 2.51×   |
| **egui 0.31** | **85**     | **2807** | **656**  | 1.00×   |
| iced 0.13     | 110        | 3169     | 799      | 1.22×   |

**Dewey loses this half, and the gap widens with complexity** — from 1.55× to
1.90× egui's tokens for a plain app, and from 1.81× to 2.51× for an
agent-driveable one. For an agent that means more to generate and more surface
to get wrong, and it gets worse as the app grows, which is the wrong direction.

`cargo check` latency measured 2.4–4.2 s for all four, with the ranking
flipping between runs on a loaded machine; it does not discriminate at this
size and is not reported.

Where the extra goes:

- **The Elm architecture is not free to write.** A `Msg` enum plus an `update`
  arm per message is real code that egui's immediate mode does not need — it
  mutates `self.count` inline at the click site.
- **Layout is explicit.** Dewey splits rectangles by constraint; egui and iced
  infer flow from widget order.
- **Agent affordances are opt-in and hand-written**: +33% code in both apps —
  `agent_id` calls, and an `execute_action` handler that in TodoMVC has to
  string-match `toggle_0`, `delete_3` and parse the index back out.
- **Lists are the worst case.** TodoMVC's item rows need manual rectangle
  arithmetic (`let row = Rect::new(x, y, w, 28.0); y += 28.0;`) plus a manual
  overflow break, where egui and iced simply push widgets into a flow. This is
  most of why the gap grows from 1.55× to 1.90×.

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
full 9-step task                             51.9 µs
```

**≈19,000 complete task runs per second.** Both harnesses assert the workflow
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
