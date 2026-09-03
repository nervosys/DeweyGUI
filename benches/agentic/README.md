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

## First runs, 2026-09-03

Seven paid attempts at `t1-counter`, about $4.60. **No run has scored above
zero**, and n=1–3 per arm settles nothing statistically. Two things are worth
recording anyway: what the model did, which was consistent across arms, and
that five of the seven were spoiled by defects in this harness rather than by
the model.

| arm | n | turns | cost | source reads | ontology calls | built |
|---|---|---|---|---|---|---|
| bare | 2 | 33, 48 | $0.66, $0.94 | 1, 1 | 0, 0 | no, no |
| mcp | 1 | 34 | $0.66 | 0 | 1 (`ping`) | yes |
| warned | 3 | 32, 29, 29 | $0.60, $0.47, $0.55 | 0, 0, 0 | 3, 2, 2 | no, yes, yes |

**The failure this was built to measure is not the failure that happened.**
The concern was that a model would read the source rather than ask the
ontology. In the `bare` and `mcp` arms it did neither — two source reads and
one `ping` across 115 turns — and wrote an API that does not exist:

    fn view(&self) -> View<Msg>              // bare
    fn view(&self) -> Element<Self::Message> // mcp — iced's signature verbatim
    Command::none()                          // iced's constructor

`Element<Self::Message>` and `Command::none()` are iced. The model recognised
the shape — an Elm-architecture Rust GUI crate — and confidently wrote the
framework it already knew. Guessing beat both reading and asking.

**The `warned` arm changed that, and tool availability alone did not.** Its
prompt adds four sentences: Dewey is not iced, ratatui or egui, signatures you
remember will not compile, look at `examples/` and the attached tools first. It
states no part of the API — giving away the thing the model got wrong would
measure spec-following rather than discovery. Under it the model globbed the
examples, called `query_ontology` and `get_schema`, made no source reads, and
built successfully twice out of three. The iced signatures stopped appearing.

That is the opposite of what HawkTUI's triggers arm found, where consultation
rose and outcomes did not move. It is also one run per arm at a task nobody
has yet passed, so it is a hypothesis with a result attached, not a finding.

### The harness was the bigger problem

Five defects, three of which spoiled a paid run each. All are fixed, and the
ones that could recur silently now cannot.

| defect | cost | fix |
|---|---|---|
| the prompt said to depend on Dewey "by path" and never said which path | 1 run | the crate path is injected |
| the MCP config's `cwd` was relative; the client resolved it from the agent's scratch directory and reported `failed`, so an `mcp` run was `bare` wearing the wrong label | 2 runs | absolute path, and a run whose server is not `connected` is refused rather than recorded |
| `cargo run` was too slow for the MCP handshake, failing the same way a second time | — | it points at the built binary |
| a transcript shape crashed the reader after the model had been paid for | 1 run | transcripts are written before anything parses them |
| the binary was looked for at `workdir/target/release/app`, and `CARGO_TARGET_DIR` had put it elsewhere, so a run that built fine scored `program not found` | 1 run | the path comes from cargo's own output |

Two threats to validity remain. The agent inherits the operator's Claude
configuration: one `warned` run spent turns grepping an unrelated repository
that happened to be on the operator's allowed-directory list, and
`--strict-mcp-config` only fixes half of that. And `t1-counter` has not been
passed by any arm, so nothing here distinguishes a model that cannot do the
task from a task specification that is wrong.

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
