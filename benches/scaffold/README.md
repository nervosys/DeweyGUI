# Agent Scaffolding Benchmarks

How expensive is it for an *agent* to build and then verify a GUI program in
each framework? Two costs, measured separately:

1. **Scaffolding** — what the agent has to write, and how long it waits to find
   out whether it compiles.
2. **Closing the loop** — whether the agent can then confirm the program works,
   and how fast.

```bash
cd benches/scaffold && python measure.py           # scaffolding cost
cd benches/comparative && cargo run --release --bin agent_loop   # the loop
```

## The canonical program

The same application in every framework: a 400×200 window showing `Count: N`
above three buttons — Decrement, Reset, Increment — wired to the state. Four
implementations, all idiomatic and minimal:

| Bin                   | What it is                                        |
| --------------------- | ------------------------------------------------- |
| `counter_dewey_plain` | Dewey, no agent affordances                       |
| `counter_dewey`       | Dewey, agent-driveable (`agent_id` + action handler) |
| `counter_egui`        | egui / eframe                                     |
| `counter_iced`        | iced                                              |

## 1. Scaffolding cost

`check` is `cargo check` after touching the file, warm deps, best of three —
the latency of one step in an agent's edit → compile → fix loop.

| Framework      | Code lines | Bytes | ~Tokens | `cargo check` |
| -------------- | ---------- | ----- | ------- | ------------- |
| Dewey (plain)  | 39         | 1455  | 425     | 4.39 s        |
| Dewey (agent)  | 52         | 1685  | 495     | 4.65 s        |
| **egui 0.31**  | **33**     | 1021  | **274** | **2.70 s**    |
| **iced 0.13**  | 37         | **970** | 276   | **2.23 s**    |

**Dewey loses this half, and not narrowly.** A plain Dewey counter costs ~1.55×
egui's tokens and ~1.6× its compile-check latency; the agent-driveable version
costs ~1.8× the tokens. For an agent, that is more to generate, more surface to
get wrong, and a slower feedback loop on every iteration.

Where the extra goes:

- **The Elm architecture is not free to write.** A `Msg` enum plus an `update`
  arm per message is real code that egui's immediate mode does not need — it
  mutates `self.count` inline at the click site.
- **Layout is explicit.** Dewey splits rectangles by constraint; egui and iced
  infer flow from widget order.
- **Agent affordances are opt-in and hand-written**: +13 code lines and +70
  tokens for `agent_id` calls and an `execute_action` handler.

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

The clearest way to close the scaffolding gap would be to let widgets carry
their message directly — `Button::new("+").on_click(Msg::Increment)` — which
would delete both the `execute_action` handler and much of the `update` arm
wiring, making agent-driveability roughly free instead of +13 lines. That is a
framework design change, not a benchmark change, and is not implemented.

## Caveats

- Token counts are a lexer-level proxy (identifiers, numbers, punctuation), not
  a specific model's tokeniser. Use the ratios, not the absolutes.
- This measures the *machine-side* costs an agent pays. It does not measure
  model thinking time, how many iterations each framework's API tends to need,
  or how well any model knows each library — all of which need real agent runs
  to answer, and none of which this harness attempts.
- `cargo check` latency is one machine, warm cache, and includes type-checking
  the framework's generics.
