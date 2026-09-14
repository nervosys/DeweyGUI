# Handoff

**Date:** 8 September 2026 · **Range:** `65f67d0..HEAD`, 46 commits ·
**State:** green, 407 tests across 14 binaries, five features built

Read this before picking the work up. It says what was wrong, what is fixed,
what is knowingly still broken, and what the money bought.

---

## The two patterns

Nearly every defect in this range was one of two shapes.

**Something reported success and did nothing.** It compiled, it was documented,
it was on the roadmap as complete, and no code path reached it. Or it reached
somewhere nobody asked for: a click carries a point, every host passed `null`
as the action's parameters, and fourteen widgets applied their handler's
`unwrap_or` default instead — a 0..100 slider went to 0 wherever you clicked.

**Something could not be tested, so it was not.** The stdio transport, the MCP
loop, the examples: each owned stdin, stdout or a window, and none could be
driven. Every one of them turned out to be correct. The risk was never that the
code was broken; it was that nothing would have said so. Untestable and untested
are one fact seen from two sides, and the fix each time was the same move —
separate the part that owns the world from the part that has the logic.

The third thing worth knowing is that most of the later finds came from
**distrusting a check rather than trusting it**. A check that passes tells you
nothing until you know what it would fail on. `both_backends_handle_every_command`
asked whether each host *mentioned* each variant, and a `log::debug!` mentions
it; `documented_responses_match_what_the_server_sends` had a floor of one and was
comparing two pairs; `examples_validate` read source text and never ran an
example. Each of those, interrogated, gave up real defects.


The ones a user would notice, in that order:

| what | it was |
|---|---|
| **Clicking a button** | inert under the default backend. It converted a click to an `Event::Mouse`, handed it to `handle_event` and stopped — no `hit_test`, no `Handlers`. `Button::action`, `Checkbox::on`, `TextInput::on_input` and nine more did nothing under the backend `Program::run` uses. Worked headless, so all tests passed; worked under agpu, which is opt-in. |
| **Pressing Tab** | did nothing anywhere. `FocusManager` was complete and nothing called it. |
| **A modal dialog** | dimmed the screen and blocked no input; clicks landed on the button behind it. |
| **`Command::AgentAction`** | a `log::debug!` under the default backend — the same line, in the same position, that had made both network transports unable to act. |
| **The plugin system** | ran only under `agpu-backend`. `Program` had no `with_plugin` at all, and agpu dropped two of the four contributions plugins make. |
| **The AccessKit tree** | published every widget and marked none focused, so a screen reader could be read the interface but not walk it. |
| **Clicking a slider, tab, list, table, toolbar, text field or splitter** | did something other than what the click said. A click carries a point and the hosts passed no parameters, so each handler applied its own `unwrap_or` default: a 0..100 slider went to 0, any tab selected the first, a text field cleared itself. Fourteen widgets, and nothing reported it — the action succeeded. |
| **A button inside a modal** | could not be pressed. The backdrop blocked correctly *and* outranked the dialog's own widgets, because it registered at `u32::MAX` and nothing can register above that. |
| **A `Tree`** | could be expanded by an agent and by no person: it registered handlers and no hitbox, so no click reached it and Tab could not either. |
| **`Event::DragDrop`** | could be delivered by no host. The vocabulary was complete — five kinds, four payloads, a source and a target — and agpu converted an event the agpu crate never constructs. A drag is a reading of a press, some movement and a release, and `drag::DragTracker` is that reading; every host feeds it. |
| **A wheel turn** | reached no widget at all. `ScrollArea` and `VirtualList` advertise `scroll_to`, neither registered a hitbox, and no host hit-tested a scroll — so every application caught `Event::Mouse` and did the arithmetic hit-testing exists to do. |
| **`Select`, `Menu`, `Tooltip`, `ColorPicker`** | described more than they drew. A dropdown that never dropped down, a menu bar with no items, a tip drawn by nothing while an agent could read it, and "HSV/hex color selection" that was a swatch. All four are drawn now, through `Frame::overlay`. |
| **`Command::SetTickRate`** | was dropped by the default backend, whose arm was a comment saying egui handled it. egui schedules from a field that arm never wrote. |
| **Nine widgets in the examples** | advertised actions with no handler, six of them in `showcase.rs` — the file an agent reads to learn what these widgets do. The examples were compiled and never run; `--dewey-validate` runs them now, in CI. |
| **`ping` in the protocol reference** | documented a `version` field the server has never sent. It was written as prose rather than a fenced block, so the check that compares documented replies against real ones never parsed it. |

Three modules described work they do not do (`memory`, `gpu`, `theme`), two
more were undriven (`focus` — now driven — and `overlay`), and the README and
ROADMAP claimed several of these as finished.

### Why this kept happening

The three hosts — the default egui backend, agpu, and `HeadlessDriver` — each
had their own copy of the same logic, and **the copy nobody could test was the
one that was wrong**. Tests drive the headless driver; neither backend opens a
window in CI.

The structural fix is that a click, an agent action and a keypress now go
through one implementation each: `Handlers::apply_primary`,
`focus::handle_key`, `focus::draw_ring`. `tests/backend_parity.rs` fails if any
host stops calling them.

---

## What now stops it recurring

Every one of these was verified by breaking the thing it catches.

| check | refuses |
|---|---|
| `tests/reachability.rs` | a subsystem that is neither driven, declared types-only, nor carrying the exact sentence *"Nothing in this crate drives it."* It caught its own entry going stale when focus was wired. |
| `tests/backend_parity.rs` | a click, an `AgentAction`, a window option, an event kind, the focus ring, a frame measurement, the pointer position, a wheel turn, a drag or the deferred-overlay pass reaching one host and not another. Also a `Command` arm that is only a comment or a log line — the older check asked whether each host *mentioned* each variant, which a `log::debug!` does. |
| `tests/click_position.rs` | a widget whose action takes a parameter and does not say what a click supplies; a widget reachable by no pointer at all; and wiring registered inside `if frame.describes(..)`, which exists only on frames that build an ontology tree — a `Menu` had a handler when an agent looked and none when a person clicked. |
| `tests/frame_cost.rs` | frame timings that are not measurements |
| `tests/stdio_transport.rs`, `tests/mcp_server.rs` | the two protocol loops every agent's traffic passes through, which had no tests at all because they held stdin and stdout directly |
| `tests/derive_widget.rs` | `#[derive(Widget)]`, which was re-exported, documented and used by nothing |
| `--dewey-validate` | an example whose interface cannot be operated. CI runs all six. |
| `tests/agent_view.rs` | a widget that does not tell a reader what an agent can do with it; a block that disagrees with the widget it describes; an action that calls itself repeatable when it is not; and a new `Discoverable` implementor that no test builds, so the check cannot be outgrown quietly. The blocks are generated from each widget's own `schema`, `actions`, `agent_state` and `capabilities`, so renaming an action fails the build until the comment follows. |
| `tests/docs_conformance.rs` | a documented request that does not deserialise; a documented response field the server does not send, at any depth, with the handshake's capability list compared element by element; an `ignore`d doctest with no stated reason; a README quick start or `llms.txt` sample that has drifted from `examples/quickstart.rs`; and an `llms.txt` claim that is not true |
| `src/agent/mcp.rs` tests | a tool description that stops telling a model not to read the source, or promises atomicity |
| `scripts/check.sh` | nothing — it is what CI runs, in one command. `--all` adds the sibling crate and both benchmark workspaces. |

`scripts/check.sh` exists because this session pushed a red Test job once and a
red Format job once. It passes end to end, and it needs a warm target
directory: on a cold one it dies part-way through whichever heavy build it
reaches first, with exit 127 and no error message, which looks exactly like a
failing check and is not one. Run it twice. Observed on the release build of
`benches/comparative` once and on an example the next time, after a version bump
invalidated the cache — so it is any large cold build, not one target. Most of
this session mistook it for lock contention, and the first note about it here
blamed one specific target. Run it before pushing — and run it with `--all`, which adds
the sibling crate, both benchmark workspaces, the six examples, the two free
agentic self-tests and the three features CI had never compiled.

Two of these were added after the check that should have caught the defect
failed to. Interrogating a passing check is the highest-yield thing in this
repository, and the questions that worked were always the same: what would this
fail on, and is that the thing I care about?

---

## Known limits, all recorded in ROADMAP.md

These are honest `[~]` entries, not oversights:

- **`Arena`/`VecPool`/`InlineString`**, `RenderBatch`, `ThemeWatcher`,
  `OverlayStack` — working code nothing calls
- **agpu’s own profiler** is off unless `with_profiling(true)` and read by
  nothing: that backend answers no agent requests, so `get_performance`
  cannot reach it. The other two hosts are wired and read
- **tray and native dialogs** — types only, no platform backend
- **multi-window** — in-memory bookkeeping that opens no windows
- **the eframe pin** cannot move: wgpu-hal 30 needs `windows 0.62`, agpu pins
  wgpu 24 which needs `windows 0.58`

---

## The question, and what it cost to answer

**Does an agent actually use the ontology?** Every performance number this
project publishes assumes it does. An ontology nobody queries costs the same
and buys nothing.

**Answered, 8 September 2026, for $19.15 across 24 valid runs. In two halves
that point opposite ways.**

| | writing an application (`t1-counter`) | driving one it did not write (`t3-inspect`) |
|---|---|---|
| runs | 12, four per arm | 12, four per arm |
| score | 1.000 every run | 12/12 correct |
| ontology calls | **0 in every run** | 3.8 / 8.2 / 6.0 per run by arm |
| the `mcp` arm | server `connected`, tools listed, instructions delivered — **not one tool invoked** | cheapest of the three: 19.8 turns at $0.37 against 29.0 and $0.58 |

Writing: it read `examples/counter.rs` (12 reads), `examples/agent_headless.rs`
(9) and `llms.txt` (7). Three reads in twelve runs touched `src/` at all. It did
not grind through the crate — it read the curated static files, and preferred a
good example to a live interface it was told about.

Driving: it has no choice, and it does it well. The best run took a `get_tree`,
checked it against a text `screenshot`, then used `get_performance` to confirm
`widget_count` matched the tree before trusting it.

**So: typed tools earn their maintenance for agents that operate an interface,
and appear not to for agents writing code against the crate.** If you are
shipping an application for agents to drive, ship the MCP server. If you want a
model to write Dewey code, spend the effort on examples and `llms.txt` — that is
what twelve runs actually read, and `llms.txt` had no Rust in it at all until
those runs said so.

Read all of it as a direction: one model, n=4 per arm, two tasks.

What is measured (`benches/scaffold/src/bin/observation_cost.rs`, in CI):

- a full `get_tree` is ~2000 estimated tokens and this TodoMVC's whole egui
  source is 801 — **on an application that small, reading is cheaper than one
  observation**
- the ontology wins on targeted reads (120 tokens), on change-polling (29),
  and on the three of five questions source cannot answer at any price
- on an application three times the size, asking is ahead from the first
  observation

What `observation_cost` does **not** measure is a model choosing, and its
header now says so: every saving it prices assumes an agent that asks, and on a
writing task twelve runs did not. `benches/agentic/` is what measured the
choosing.

### Its state

Everything that does not cost money is checked and passing in CI —
`runner/selftest.py` scores the verifier, and `runner/selftest_pipeline.py`
builds what a perfect attempt would have written and scores it **1.000 through
the same code a real run uses**. The task is passable and the plumbing works.

The results are in `benches/agentic/README.md`, arm by arm, with the twelve
harness defects that were paid for on the way and the four runs excluded
because the harness failed them rather than the agent.

### Running it again

```
python benches/agentic/runner/run.py --task t3-inspect --condition mcp --runs 4
python benches/agentic/runner/analyze.py results/bare/runs.jsonl results/mcp/runs.jsonl
```

About $0.40 a run for `t3-inspect` and $1.00 for `t1-counter`. Run
`selftest.py` and `selftest_drive.py` first; they cost nothing and they are the
only cheap place to find the next harness defect. Twelve have been paid for at
roughly a dollar each, and the free self-tests never spawn a model, which is
precisely how the last three got through.

The estimate that used to sit here said $5 for twelve runs. It was $11.98.

The three arms: `bare` is the prompt and the crate; `mcp` attaches an MCP
server so `initialize` puts the instructions from `src/agent/mcp.rs` in front of
the model — `examples/mcp_server.rs` for a writing task, and for `t3-inspect`
the application under inspection itself, so the tools point at *that* program
rather than at an empty catalogue; `warned` adds four sentences saying
Dewey is not iced, remembered signatures will not compile, and where to look.
`warned` states no part of the API on purpose — giving that away would measure
spec-following rather than discovery.

### What the sibling project already found

HawkTUI's `benchmarks/agentic/`, 184 recorded runs, is the closest prior art
and worth reading before spending:

- agents read the implementation in **100% of Hawk TUI runs** (16–22 reads
  each, first at tool call #1) against **6% of ratatui runs**. The habit tracks
  whether the model was trained on the framework, and Dewey is in Hawk's
  position.
- MCP tools raised consultation 4% → 42%, trigger prompts → 83%, and **score,
  cost and turns did not move**. Consultation is not the metric.

That is why `docs/agent-prompt.md` and the MCP `instructions` are described in
this repository as cheap and plausible rather than demonstrated.

---

## Where things are

```
scripts/check.sh              what CI runs, in one command
llms.txt                      the machine-readable index; a test keeps it true
docs/agent-prompt.md          paste-in fragment for clients with no MCP instructions
docs/agent-protocol.md        the protocol reference
examples/quickstart.rs        the sample in both the README and llms.txt
examples/mcp_server.rs        MCP server over the widget catalogue
src/drag.rs                   press + movement + release, read as a drag
benches/comparative/          frame-build cost against egui and iced
benches/scaffold/             what an agent must write, and observation_cost.rs
benches/agentic/              the model-in-the-loop harness, and its results
benches/agentic/subject/      an application an agent is asked about, not to write
```

Any application built with this crate can prove its first screen is operable
without a display:

```
cargo run --example showcase -- --dewey-validate
```

That flag found nine widgets in this repository's own examples advertising
actions with no handler, six of them in `showcase.rs`.

`CHANGELOG.md` carries the full list of what changed and why, including the
findings that were unflattering.
