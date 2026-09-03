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
