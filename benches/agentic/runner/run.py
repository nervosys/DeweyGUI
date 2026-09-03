#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Drive a real model at a task and record what it cost.

This is the half of the question no other benchmark in this repository can
answer. `benches/scaffold/src/bin/observation_cost.rs` prices the strategies a
model picks between; nothing observes a model picking one. Whether it asks the
application or reads the source is a fact about model behaviour, and the only
way to find it out is to run one.

    run.py --task t1-counter --condition bare --runs 3

Writes one JSON object per attempt to `results/<label>/runs.jsonl`, with the
fields `analyze.py` expects. Costs money and needs a model; `selftest.py` is
what runs in CI, and it needs neither.

## Conditions

- `bare`     — the prompt, the crate, nothing else. What an agent meeting
               Dewey for the first time gets.
- `mcp`      — the Dewey MCP server is attached, so `initialize` puts the
               instructions in `src/agent/mcp.rs` in front of the model.

The comparison of those two is the whole point, and the sibling project's
results say not to expect much: HawkTUI raised ontology consultation from 4%
to 42% with MCP tools and to 83% with trigger prompts, and score, cost and
turns did not move. Consultation is not the metric.
"""
import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
TASKS = ROOT / "tasks"
RESULTS = ROOT / "results"
CRATE = ROOT.parent.parent

sys.path.insert(0, str(HERE))
from verify import verify  # noqa: E402


# A behavioural nudge, not an answer. It does not state the trait shape —
# giving away the thing the model got wrong would measure spec-following
# rather than discovery. It says only that guessing will not work and where to
# look, which is the intervention worth testing after the first runs showed a
# model writing iced's signatures from memory without consulting anything.
WARNING = """
## Before you write any Dewey code

Dewey is not iced, not ratatui, and not egui. Signatures you remember from
those crates will not compile here, and the compiler errors will not tell you
which crate you are thinking of. Look at what this crate actually provides
before writing against it: `{{CRATE}}/examples/` holds working programs, and
the tools attached to this session describe every widget and every action it
accepts.
"""


def build_prompt(task_dir, condition="bare"):
    """The task prompt with the contract spliced in, as the agent sees it.

    `{{CRATE}}` becomes the absolute path to this checkout. The agent works in
    a scratch directory and cannot be expected to guess where the framework
    lives; without it every attempt fails at `cargo build` for a reason that
    has nothing to do with the task.
    """
    prompt = (Path(task_dir) / "prompt.md").read_text(encoding="utf-8")
    contract = (TASKS / "contract.md").read_text(encoding="utf-8")
    if condition == "warned":
        prompt = prompt + WARNING
    return prompt.replace("{{CONTRACT}}", contract).replace(
        "{{CRATE}}", CRATE.as_posix()
    )


def mcp_binary():
    """Build `examples/mcp_server.rs` and return the path to it."""
    proc = subprocess.run(
        [
            "cargo",
            "build",
            "--release",
            "--example",
            "mcp_server",
            "--message-format",
            "json-render-diagnostics",
        ],
        cwd=CRATE,
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        sys.stderr.write(proc.stderr)
        raise SystemExit("run.py: the MCP server example does not build")
    for line in proc.stdout.splitlines():
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            continue
        if message.get("executable") and message.get("target", {}).get(
            "name"
        ) == "mcp_server":
            return message["executable"]
    raise SystemExit("run.py: cargo did not say where it put the MCP server")


def run_agent(prompt, workdir, condition, model):
    """Run the agent once in `workdir`. Returns (transcript, meta)."""
    cmd = [
        "claude",
        "-p",
        prompt,
        "--output-format",
        "stream-json",
        "--verbose",
        "--permission-mode",
        "acceptEdits",
        # Only the servers this benchmark attaches. Without it a run inherits
        # whatever the operator has configured, and the first `warned` run
        # spent turns grepping an unrelated repository that happened to be on
        # the operator's allowed-directory list.
        "--strict-mcp-config",
    ]
    if model:
        cmd += ["--model", model]
    if condition in ("mcp", "warned"):
        # The server is the point of these conditions: it is what puts the
        # instructions in front of the model. The config is written here with
        # an absolute `cwd`, because the client resolves a relative one from
        # the agent's scratch directory — a checked-in `../../..` produced
        # `{"name": "dewey", "status": "failed"}` and a run that was silently
        # the `bare` condition wearing the `mcp` label.
        # Outside the work tree: written inside it, the file shows up in
        # the agent's own directory listing and the first `warned` run read
        # it, which is the benchmark leaking into what it measures.
        config = Path(tempfile.mkdtemp(prefix="dewey-mcp-")) / "mcp.json"
        config.write_text(
            json.dumps(
                {
                    "mcpServers": {
                        "dewey": {
                            # The built binary, not `cargo run`: cargo
                            # compiles on first use and the client gives up
                            # waiting for the handshake, which is the second
                            # way this arm silently became the other one.
                            "command": mcp_binary(),
                            "args": [],
                            "cwd": str(CRATE),
                        }
                    }
                }
            ),
            encoding="utf-8",
        )
        cmd += ["--mcp-config", str(config)]

    started = time.time()
    proc = subprocess.run(
        cmd,
        cwd=workdir,
        capture_output=True,
        text=True,
        stdin=subprocess.DEVNULL,
    )
    wall = time.time() - started

    events = []
    for line in proc.stdout.splitlines():
        try:
            events.append(json.loads(line))
        except json.JSONDecodeError:
            continue
    # Whether the server actually attached. A failed one turns this arm into
    # the other arm without saying so.
    servers = {}
    for event in events:
        for entry in event.get("mcp_servers") or []:
            servers[entry.get("name")] = entry.get("status")

    return events, {
        "wall_seconds": wall,
        "agent_exit": proc.returncode,
        "mcp_status": servers.get("dewey"),
    }


def classify(name, args):
    """What a tool call was looking for: `source`, `ontology`, or neither.

    Reading the framework's own source is the behaviour the whole question is
    about, so it is worth being precise about what counts. A `Read` of the
    application the agent is writing is work; a `Read` of Dewey's `src/` is the
    model going around the ontology.
    """
    lowered = args.lower()
    if name.startswith("mcp__dewey"):
        return "ontology"
    if name in ("Read", "Grep", "Glob"):
        looks_like_dewey = "dewey" in lowered
        # A `src` path segment, however the path is spelled. Matching `/src/`
        # missed `"path": "/repo/deweygui/src"` — a directory-wide grep, which
        # is the most source-reading a single call can be.
        segments = lowered.replace("\\", "/").split("/")
        in_the_crate = any(re.sub(r"\W", "", s) == "src" for s in segments)
        if looks_like_dewey and in_the_crate:
            return "source"
    return None


def summarise(events):
    """Turns, cost, and where the model looked, from the transcript.

    The token split is what the sibling project found mattered most: source
    reads put 10,664 tokens per run into HawkTUI's context against 1,989 for
    ontology answers, and source reading was 42% of everything its tools
    returned. Counting calls alone would miss that a source read is several
    times larger than an ontology answer.
    """
    turns = 0
    cost = 0.0
    counts = {"source": 0, "ontology": 0}
    chars = {"source": 0, "ontology": 0}
    total_result_chars = 0
    # Which call each result belongs to, since the result arrives in a later
    # event than the call that asked for it.
    kind_of = {}

    for event in events:
        if event.get("type") == "assistant":
            turns += 1
        if "total_cost_usd" in event:
            cost = event["total_cost_usd"]

        # A real transcript carries shapes the obvious reading does not
        # survive: `message` is sometimes a string, and so is `content`. The
        # first paid run of this crashed here and its transcript was lost,
        # which is why `one_run` now writes the transcript before parsing it.
        message = event.get("message")
        if not isinstance(message, dict):
            continue
        blocks = message.get("content")
        if isinstance(blocks, str) or blocks is None:
            continue
        for block in blocks:
            if not isinstance(block, dict):
                continue
            kind = block.get("type")
            if kind == "tool_use":
                what = classify(block.get("name", ""), json.dumps(block.get("input", {})))
                if what:
                    counts[what] += 1
                    kind_of[block.get("id")] = what
            elif kind == "tool_result":
                content = block.get("content")
                if isinstance(content, list):
                    size = sum(len(str(p.get("text", ""))) for p in content)
                else:
                    size = len(str(content or ""))
                total_result_chars += size
                what = kind_of.get(block.get("tool_use_id"))
                if what:
                    chars[what] += size

    return {
        "turns": turns,
        "cost_usd": round(cost, 4),
        "source_reads": counts["source"],
        "ontology_calls": counts["ontology"],
        "tool_result_chars": total_result_chars,
        "source_result_chars": chars["source"],
        "ontology_result_chars": chars["ontology"],
    }


def one_run(task, condition, model, keep):
    task_dir = TASKS / task
    prompt = build_prompt(task_dir, condition)
    workdir = Path(tempfile.mkdtemp(prefix=f"dewey-{task}-"))
    try:
        events, meta = run_agent(prompt, workdir, condition, model)

        # Before anything that can fail: an attempt costs money, and a reader
        # that throws must not be able to lose one. The first run of this
        # crashed on a transcript shape and there was nothing left to debug.
        transcripts = RESULTS / "transcripts"
        transcripts.mkdir(parents=True, exist_ok=True)
        stamp = time.strftime("%Y%m%d-%H%M%S")
        raw = transcripts / f"{task}-{condition}-{stamp}.jsonl"
        with raw.open("w", encoding="utf-8") as f:
            for event in events:
                f.write(json.dumps(event) + "\n")

        record = {
            "task": task,
            "condition": condition,
            "transcript": raw.name,
            **meta,
            **summarise(events),
        }

        # An `mcp` run whose server did not attach is a `bare` run wearing the
        # wrong label, and averaging it into the comparison would answer the
        # question with the arms swapped. Two paid runs were recorded that way
        # before this check existed.
        if condition in ("mcp", "warned") and meta.get("mcp_status") != "connected":
            raise SystemExit(
                f"run.py: the MCP server reported `{meta.get('mcp_status')}`, so "
                "this run is not the condition it claims to be. Nothing was "
                f"recorded; the transcript is at {raw}."
            )

        built = subprocess.run(
            [
                "cargo",
                "build",
                "--release",
                "--message-format",
                "json-render-diagnostics",
            ],
            cwd=workdir,
            capture_output=True,
            text=True,
        )
        record["built"] = built.returncode == 0
        # A failed build with no error recorded is a paid run that taught
        # nothing. The first one of these came back `built: false` and there
        # was no way to tell whose fault it was.
        if not record["built"]:
            record["build_error"] = (built.stderr or built.stdout)[-1500:]
            record["wrote"] = sorted(
                str(f.relative_to(workdir))
                for f in workdir.rglob("*")
                if f.is_file() and "target" not in f.parts
            )[:40]
        # Where cargo says it put the binary, not where it would go by
        # default. `CARGO_TARGET_DIR` is commonly set to a shared directory,
        # and guessing `workdir/target/release/app` reported `program not
        # found` for a run that had built perfectly well — a paid attempt
        # scored zero for the harness's mistake.
        binary = None
        for line in built.stdout.splitlines():
            try:
                message = json.loads(line)
            except json.JSONDecodeError:
                continue
            if message.get("executable"):
                binary = message["executable"]
        if binary is None:
            binary = workdir / "target" / "release" / (
                "app.exe" if os.name == "nt" else "app"
            )
        result = verify(task_dir, binary, cwd=workdir) if record["built"] else None
        record.update(
            {
                "score": result["score"] if result else 0.0,
                "checks_passed": result["checks_passed"] if result else 0,
                "checks_total": result["checks_total"] if result else 0,
                "contract_failed": result["contract_failed"] if result else True,
                # Why it scored what it scored. Without this a run that built
                # and scored zero says only that something went wrong, which
                # is what the first three attempts said.
                "verify_error": result["error"] if result else "did not build",
                "first_frame": result["first_frame"] if result else None,
                "failed_checks": (
                    [c["id"] for c in result["checks"] if not c["passed"]]
                    if result
                    else []
                ),
            }
        )
        return record
    finally:
        if not keep:
            shutil.rmtree(workdir, ignore_errors=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--task", required=True)
    parser.add_argument(
        "--condition", default="bare", choices=["bare", "mcp", "warned"]
    )
    parser.add_argument("--runs", type=int, default=1)
    parser.add_argument("--model", default=None)
    parser.add_argument("--label", default=None)
    parser.add_argument("--keep", action="store_true", help="keep the work tree")
    args = parser.parse_args()

    if args.condition in ("mcp", "warned"):
        print("checking the MCP server attaches before spending anything ...")
    if shutil.which("claude") is None:
        raise SystemExit(
            "run.py needs the `claude` CLI on PATH. This is the one part of the "
            "harness that costs money and cannot run in CI; `selftest.py` "
            "checks everything else."
        )

    label = args.label or args.condition
    out = RESULTS / label
    out.mkdir(parents=True, exist_ok=True)
    path = out / "runs.jsonl"

    with path.open("a", encoding="utf-8") as f:
        for i in range(args.runs):
            record = one_run(args.task, args.condition, args.model, args.keep)
            f.write(json.dumps(record) + "\n")
            f.flush()
            print(
                f"[{i + 1}/{args.runs}] {record['task']} {record['condition']}: "
                f"score {record['score']:.3f}  turns {record['turns']}  "
                f"${record['cost_usd']:.3f}  "
                f"source {record['source_reads']}  ontology {record['ontology_calls']}"
            )

    print(f"\nwrote {path}")


if __name__ == "__main__":
    main()
