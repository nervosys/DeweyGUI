# Agentic benchmark

What it costs a model to build a working Dewey application, and whether it
consults the ontology or reads the source.

Every other benchmark in this repository prices a strategy.
`benches/scaffold/src/bin/observation_cost.rs` says a targeted read costs 120
tokens and a source file costs 801; it cannot say which one a model reaches
for. That is a fact about model behaviour, and the only way to find it out is
to run one.

## What it measures

| | |
|---|---|
| `score` | fraction of the task's checks the rendered interface passes |
| `contract_failed` | the attempt ignored the harness contract, so its score says nothing about the interface |
| `turns` | assistant turns |
| `cost_usd` | what the attempt cost |
| `source_reads` | tool calls that read Dewey's own source |
| `ontology_calls` | MCP calls to the Dewey server |

Two conditions. **`bare`** is the prompt and the crate: what an agent meeting
Dewey for the first time gets. **`mcp`** attaches `examples/mcp_server.rs`, so
`initialize` puts the instructions from `src/agent/mcp.rs` in front of the
model. The comparison of the two is the point.

## What it does not measure

Not a cross-framework comparison. HawkTUI can compare frameworks because TUIs
share a character grid, so one verifier scores ratatui and Hawk identically.
GUIs share no such surface: scoring an egui attempt would mean asking it to
emit a widget tree it has no notion of. The cross-framework question stays
where it was — the static token proxy in the README, which measures a
reference implementation's source and not a session.

Not statistical power, at these run counts. `analyze.py` reports bootstrap
intervals and says which differences span zero; three runs of anything span
zero.

## Running it

```
python runner/selftest.py                              # no model, no network
python runner/run.py --task t1-counter --condition bare --runs 5
python runner/run.py --task t1-counter --condition mcp  --runs 5
python runner/analyze.py results/bare/runs.jsonl results/mcp/runs.jsonl
```

`selftest.py` is what CI runs. It scores the reference solutions, which must
come out at 1.000, and a deliberately broken one whose button renders
perfectly and has no id, which must not. A scoring function nobody has scored
is the same shape as everything else this project spent a week finding: it
compiles, it produces a number, and the number means nothing.

Writing the checks found one of those immediately. The first version had a
`no_anonymous_widgets` check that could never fail, because an interactive
widget rendered without an id never reaches the UI tree at all — no amount of
reading a snapshot reveals one. `validate` reports `unaddressable_widget`; a
frame cannot.

## What to expect

The sibling project has already run this experiment. HawkTUI's numbers, from
184 recorded runs:

- agents read the implementation in **100% of Hawk TUI runs**, 16–22 reads per
  run, the first at tool call #1 — against **6% of ratatui runs**, 0.1 reads
  per run, first at call #8. The habit tracks whether the model was trained on
  the framework, and Dewey is in Hawk's position rather than ratatui's.
- MCP tools raised ontology consultation from **4% to 42%**, trigger prompts
  took it to **83%**, and score, cost and turns did not move: 1.000 in every
  arm, $0.78 / $0.79 / $0.78. One task got monotonically worse, 19 → 37 → 53
  turns.

So the `mcp` condition here is expected to move `ontology_calls` and not much
else. If it moves `cost_usd` or `turns` and the interval excludes zero, that
is a result worth having; if it does not, that is the honest answer and it
belongs in the ROADMAP next to the rest.

## Results — 8 September 2026

Twelve valid runs, four per arm, `t1-counter`, one model. **$11.98.** The first
valid runs this harness has produced; the twenty before them were quarantined
and none of their numbers were ever quoted.

| arm | n | built | score | turns | cost | source reads | **ontology calls** |
|---|---|---|---|---|---|---|---|
| `bare` | 4 | 4/4 | 1.000 | 41.2 | $0.97 | 1, 1, 7, 1 | **0, 0, 0, 0** |
| `mcp` | 4 | 4/4 | 1.000 | 43.8 | $0.99 | 1, 1, 1, 1 | **0, 0, 0, 0** |
| `warned` | 4 | 4/4 | 1.000 | 42.2 | $1.04 | 1, 1, 1, 5 | **0, 0, 0, 0** |

### The finding

**No run consulted the ontology. Not one, in any arm.**

In the `mcp` arm the server reported `connected` in all four runs, its tools
were listed, and `initialize` delivered the instructions telling the model the
application describes itself and that it need not read the source. The model
then used `Bash`, `PowerShell`, `Read` and `Write`, and **never invoked a single
`mcp__` tool.** Naming the tools in front of a model does not make it reach for
them.

Every run scored 1.000. Nothing was lost by not asking.

### What it read instead

Across all twelve runs, by file:

| reads | file |
|---|---|
| 12 | `examples/counter.rs` |
| 9 | `examples/agent_headless.rs` |
| 7 | `llms.txt` |
| 3 | `examples/quickstart.rs` |
| 3 | `Cargo.toml` |
| 2 | `src/agent/driver.rs` |
| 1 | `src/runtime/mod.rs` |

This is the part worth sitting with. The model did not grind through the crate
source — only three reads touched `src/` at all. It read the **examples** and
**`llms.txt`**: small, curated, static files that this project maintains for
exactly this purpose. Given a choice it made unprompted, it preferred a good
example to a live interface.

### What this does not show

`t1-counter` is a **code-writing** task. The agent writes a program from a
specification; there is no running application to observe, so `get_tree`,
`get_state`, `screenshot` and `validate` have nothing to point at. The only
ontology calls that could have helped are `query_ontology` and `get_schema` —
the catalogue, for writing against — which is what `examples/mcp_server.rs`
serves and what the model declined in favour of `examples/counter.rs`.

So this measures the writing case and says nothing about the driving case.
`benches/scaffold/observation_cost.rs` prices driving — a tree against a source
file, a targeted read against a screenshot — and **no run here exercised that at
all.** The two benchmarks do not measure the same thing, and the question the
handoff posed ("does an agent actually use the ontology?") is answered only for
half of it.

Also not shown: anything about other models, larger applications, or tasks where
the interface already exists and has to be inspected. n=4 per arm on one task
with one model. The score column is unanimous, which means the task was too easy
to separate the arms on quality — a harder task is what would make `score` say
anything.

### What it is consistent with

HawkTUI's 184 runs found agents read the implementation in 100% of Hawk TUI runs
against 6% of ratatui runs, and that MCP tools raised consultation 4% → 42% and
trigger prompts → 83% **while score, cost and turns did not move.** These twelve
runs agree with the last clause exactly — no arm differs on outcome — and
disagree with the middle one: tools and prompts moved consultation not at all,
from zero.

### The honest conclusion

The performance case for the ontology rests on an agent asking, and on this task
an agent given every encouragement did not ask once. Before quoting any
token-saving number from `observation_cost`, that assumption has to be earned on
a task where observing is the only way to answer.

**No such task exists here.** `t2-todo` is a writing task as well — build a
to-do list from a specification — so both tasks hand the agent a spec and ask
for a program, and neither ever puts a running application in front of it. An
earlier version of this paragraph said `t2-todo` was the one that would ask the
other half. That was written without reading it. Asking the driving question
needs a third task, where the agent is given an application it did not write and
the answers are only in the running state.

### `t3-inspect`, the task that asks it

Written and not yet run. The agent is handed a **running to-do application**
(`subject/`) and asked one question: how many items are urgent and not yet
done? It writes the number to `answer.txt` and is scored on that alone.

The list is built from a seed chosen when the program starts. The seed is never
given to the agent, and the source is identical on every run — so the answer
moves and the code does not. Reading the source cannot produce it. Asking the
program can, with a single `get_tree`.

The same three arms apply. `bare` gets the binary and the protocol described in
the prompt; `mcp` additionally attaches the subject itself as an MCP server, so
the tools point at *this* application rather than at an empty catalogue, which
is the difference that makes the arm mean something for a driving task.

`runner/selftest_drive.py` runs in CI and costs nothing. It checks that the
answer is reachable by asking, that it varies across 120 seeds while the source
stands still, that an agent which always guesses the commonest answer scores
only 27%, and that the prompt leaks neither the seed nor the answer. Nine
harness defects have been paid for at about a dollar each; this is where the
tenth is cheap.

#### What it can and cannot settle

It answers a narrower question than "does an agent use the ontology", and the
difference matters enough to state before anyone pays for it.

Because reading the source cannot produce the answer, an agent that wants to
finish **has to** observe. So this measures whether an agent *can* drive an
application it did not write, and what that costs in turns and tokens — not
whether it *would choose* to when reading was also an option. `t1-counter`
measured the choice and found the answer was no; this measures the capability,
which is a different thing wearing similar words.

The preference question may not be askable in the driving case at all. Driving
requires the interface; there is no second way to learn what a running program
currently holds. What the two arms still compare is *which* interface an agent
reaches for when both are present — raw JSON Lines on stdin in `bare`, typed
tools in `mcp` — and whether the typed one is worth what it costs to maintain.

That is worth knowing, and it is what `observation_cost` has been pricing all
along without anybody checking an agent could actually do it. It is not the
headline the first framing of this task implied, and the numbers should not be
reported as if it were.

#### Results — 8 September 2026

Twelve valid runs, four per arm. **$5.43**, plus **$1.74** on four runs lost to
harness permission gaps, which are described below because they are the more
useful half.

| arm | n | correct | turns | cost/run | ontology calls | source reads |
|---|---|---|---|---|---|---|
| `bare` | 4 | **4/4** | 29.0 | $0.58 | 3.8 | 0 |
| `mcp` | 4 | **4/4** | 19.8 | $0.37 | 8.2 | 0 |
| `warned` | 4 | **4/4** | 23.5 | $0.41 | 6.0 | 0 |

The twelve seeds gave answers of 0, 1, 2, 2, 3, 4, 5, 5, 5, 6, 6 and 7, so
nothing here was carried by guessing the mode.

**An agent can drive a Dewey application it did not write.** Every run got the
right number, and no run read the crate's source even once — there was nothing
there to read, which is the design. The best of them ran the program, took a
`get_tree`, checked it against a text `screenshot`, then used `get_performance`
to confirm `widget_count` matched what the tree contained before answering, and
said so.

`mcp` is the cheapest arm: fewest turns, lowest cost, most ontology calls per
run. Typed tools appear to be worth their maintenance for a driving task — the
opposite of what the same arm showed on `t1-counter`, where an agent writing
code declined the tools entirely. That is the comparison this task was built
for, and it is a real difference, but n=4 and one task: read it as a direction,
not a measurement.

#### What the four lost runs cost, and what they showed

They were harness defects, all of one kind: **the agent could not start the
program it was asked about.**

| defect | what happened |
|---|---|
| 10th | `--permission-mode acceptEdits` refuses to execute an arbitrary binary. Copying it into the work tree changed nothing. |
| 11th | `ontology_calls` counted only `mcp__` calls, so a run that sent 29 requests down a pipe scored `ontology 0`. |
| 12th | The allowlist named `Bash(subject.exe:*)` alone: one agent reached for PowerShell, another for the MCP tools, and both were denied. |

Every blocked run ended the same way — the agent stated exactly what was denied
and **refused to write a guessed number**:

> Nothing was written to `answer.txt` — I have no data to base a number on, and
> I won't guess.

That is the behaviour anybody would want, and it cost four runs. A benchmark
that scores an agent for a door the harness forgot to unlock is measuring the
harness, so those runs are excluded by seed and named here rather than averaged
in.

The eleventh is the one to remember. It would not have failed anything: it would
have reported `ontology 0` for runs whose transcripts show the agent doing
nothing but ask the ontology, and that number would have been quoted.


## First runs, 2026-09-03 — all discarded

Twenty paid attempts at `t1-counter`, about $10, **none valid**. Every one is
quarantined in `results/*/runs.*.jsonl` rather than deleted, because what they
recorded is a list of defects in this harness.

The last of them explains the rest. A transcript said it in as many words:

> I'm blocked on two things I need, and both require your approval:
> 1. **Reading the Dewey crate** at `C:/…/DeweyGUI`

`--permission-mode acceptEdits` permits writes in the work tree and **no reads
outside it**. The agent could not read the framework at all. So every arm
measured an agent that was denied access, not one that chose not to look — and
the earlier conclusion drawn from those runs, that the model prefers guessing
to consulting, is not supported by them. It could not consult. `--add-dir` is
now passed.

The other defects, in the order they cost a run:

| defect | runs lost | fix |
|---|---|---|
| the prompt said "depend by path" and named no path | 1 | the crate path is injected |
| the MCP `cwd` was relative; the client resolved it from the agent's scratch directory and reported `failed`, so an `mcp` run was `bare` mislabelled | 2 | absolute path; a run whose server is not `connected` is refused |
| `cargo run` was too slow for the MCP handshake | — | it points at the built binary |
| a transcript shape crashed the reader after payment | 1 | transcripts are written before parsing |
| the binary was sought at `workdir/target/release/app` and `CARGO_TARGET_DIR` had moved it | 1 | the path comes from cargo's output |
| the agent had to transcribe the 90-line contract runner, and two runs ended with a `Cargo.toml` naming a `src/main.rs` never written | 2 | the harness places `src/contract.rs`; the prompt fell from ~9,000 to ~2,950 characters |
| the crate was unreadable | 12 | `--add-dir` |

Placing the runner did move one number that is worth keeping: **9 of 12 runs
built**, against 1 of 8 before it. That is a fact about the task being
writable, not about the ontology.

Nothing else here should be quoted. There is no result yet.

### Before spending again

`selftest.py` and `selftest_pipeline.py` both pass, and the pipeline test
scores a perfect attempt 1.000 through the same code a real run uses, so the
next batch is the first that can measure anything. It costs roughly $5 for
four runs in each of three arms — and the account it draws on was at 94% of
its seven-day limit when these stopped.

## Layout

```
tasks/contract.md       injected verbatim into every prompt
tasks/t1-counter/       prompt.md and checks.json
tasks/t2-todo/
reference/              solutions with known answers, and one known-bad
runner/verify.py        scores frames, never source
runner/selftest.py      scores the verifier
runner/run.py           drives a model; costs money
runner/analyze.py       medians and bootstrap intervals
results/                recorded runs, not checked in
```
