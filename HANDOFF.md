# Handoff

**Date:** 3 September 2026 · **Range:** `65f67d0..0cebcd3`, 26 commits ·
**State:** green on CI, 313 tests passing across 11 binaries

Read this before picking the work up. It says what was wrong, what is fixed,
what is knowingly still broken, and the one thing that costs money to finish.

---

## The one pattern

Nearly every defect fixed here had a single shape: **something reported
success and did nothing.** It compiled, it was documented, it was on the
roadmap as complete, and no code path reached it.

The ones that mattered most, in the order a user would notice them:

| what | it was |
|---|---|
| **Clicking a button** | inert under the default backend. It converted a click to an `Event::Mouse`, handed it to `handle_event` and stopped — no `hit_test`, no `Handlers`. `Button::action`, `Checkbox::on`, `TextInput::on_input` and nine more did nothing under the backend `Program::run` uses. Worked headless, so all tests passed; worked under agpu, which is opt-in. |
| **Pressing Tab** | did nothing anywhere. `FocusManager` was complete and nothing called it. |
| **A modal dialog** | dimmed the screen and blocked no input; clicks landed on the button behind it. |
| **`Command::AgentAction`** | a `log::debug!` under the default backend — the same line, in the same position, that had made both network transports unable to act. |
| **The plugin system** | ran only under `agpu-backend`. `Program` had no `with_plugin` at all, and agpu dropped two of the four contributions plugins make. |
| **The AccessKit tree** | published every widget and marked none focused, so a screen reader could be read the interface but not walk it. |
| **`Event::DragDrop`** | cannot be delivered by any host. Documented as such rather than fixed. |

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
| `tests/backend_parity.rs` | a click, an `AgentAction`, a window option, an event kind or the focus ring reaching one host and not another |
| `tests/docs_conformance.rs` | a documented request that does not deserialise, a documented response field the server does not send, an `ignore`d doctest with no stated reason, a README quick start that has drifted from `examples/quickstart.rs`, and an `llms.txt` claim that is not true |
| `src/agent/mcp.rs` tests | a tool description that stops telling a model not to read the source, or promises atomicity |
| `scripts/check.sh` | nothing — it is what CI runs, in one command. `--all` adds the sibling crate and both benchmark workspaces. |

`scripts/check.sh` exists because this session pushed a red Test job once and a
red Format job once. Run it before pushing.

---

## Known limits, all recorded in ROADMAP.md

These are honest `[~]` entries, not oversights:

- **the profiler** is driven only by agpu, and nothing reads `last_frame()` or
  `history()` even there
- **`Arena`/`VecPool`/`InlineString`**, `RenderBatch`, `ThemeWatcher`,
  `OverlayStack` — working code nothing calls
- **tray and native dialogs** — types only, no platform backend
- **multi-window** — in-memory bookkeeping that opens no windows
- **the eframe pin** cannot move: wgpu-hal 30 needs `windows 0.62`, agpu pins
  wgpu 24 which needs `windows 0.58`

---

## The open question, and what it costs

**Does an agent actually use the ontology?** Every performance number this
project publishes assumes it does. An ontology nobody queries costs the same
and buys nothing.

What is measured (`benches/scaffold/src/bin/observation_cost.rs`, in CI):

- a full `get_tree` is ~2000 estimated tokens and this TodoMVC's whole egui
  source is 801 — **on an application that small, reading is cheaper than one
  observation**
- the ontology wins on targeted reads (120 tokens), on change-polling (29),
  and on the three of five questions source cannot answer at any price
- on an application three times the size, asking is ahead from the first
  observation

What is **not** measured: a model choosing. `benches/agentic/` is the harness
for that. It drives a real model, scores what it built by what it rendered,
and reads the transcript for turns, cost, source reads and ontology calls.

### Its state

Everything that does not cost money is checked and passing in CI —
`runner/selftest.py` scores the verifier, and `runner/selftest_pipeline.py`
builds what a perfect attempt would have written and scores it **1.000 through
the same code a real run uses**. The task is passable and the plumbing works.

**No valid model run exists.** Twenty attempts, about $10, all quarantined in
`results/*/runs.*.jsonl`. Seven harness defects consumed them; the last and
worst was that `--permission-mode acceptEdits` gave the agent no read access to
the framework, so every arm measured an agent that was *denied* rather than one
that chose not to look. `--add-dir` is now passed. `benches/agentic/README.md`
lists all seven with what each cost.

**Do not quote any number from those runs.** One observation survives and is
about the task, not the ontology: placing `src/contract.rs` in the work tree
instead of asking the agent to transcribe it took builds from 1-of-8 to 9-of-12.

### To finish it

```
python benches/agentic/runner/run.py --task t1-counter --condition bare   --runs 4
python benches/agentic/runner/run.py --task t1-counter --condition mcp    --runs 4
python benches/agentic/runner/run.py --task t1-counter --condition warned --runs 4
python benches/agentic/runner/analyze.py results/bare/runs.jsonl results/warned/runs.jsonl
```

Roughly $5. The account was at **94% of its seven-day rate limit** when this
stopped, which is why it stopped.

The three arms: `bare` is the prompt and the crate; `mcp` attaches
`examples/mcp_server.rs` so `initialize` puts the instructions from
`src/agent/mcp.rs` in front of the model; `warned` adds four sentences saying
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
examples/quickstart.rs        the README quick start, compiled by cargo
examples/mcp_server.rs        MCP server over the widget catalogue
benches/comparative/          frame-build cost against egui and iced
benches/scaffold/             what an agent must write, and observation_cost.rs
benches/agentic/              the model-in-the-loop harness
```

`CHANGELOG.md` carries the full list of what changed and why, including the
findings that were unflattering.
