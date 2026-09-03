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


def build_prompt(task_dir):
    """The task prompt with the contract spliced in, as the agent sees it."""
    prompt = (Path(task_dir) / "prompt.md").read_text(encoding="utf-8")
    contract = (TASKS / "contract.md").read_text(encoding="utf-8")
    return prompt.replace("{{CONTRACT}}", contract)


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
    ]
    if model:
        cmd += ["--model", model]
    if condition == "mcp":
        # The server is the point of this condition: it is what puts the
        # instructions in front of the model.
        cmd += ["--mcp-config", str(HERE / "mcp.json")]

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
    return events, {"wall_seconds": wall, "agent_exit": proc.returncode}


def summarise(events):
    """Turns, cost, and where the model looked, from the transcript."""
    turns = 0
    cost = 0.0
    source_reads = 0
    ontology_calls = 0
    tool_result_chars = 0
    source_result_chars = 0
    ontology_result_chars = 0

    for event in events:
        if event.get("type") == "assistant":
            turns += 1
        if "total_cost_usd" in event:
            cost = event["total_cost_usd"]

        for block in event.get("message", {}).get("content", []) or []:
            if block.get("type") != "tool_use":
                continue
            name = block.get("name", "")
            args = json.dumps(block.get("input", {}))
            # Reading the framework's own source, which is the behaviour the
            # whole question is about.
            if name in ("Read", "Grep", "Glob") and "deweygui" in args.lower():
                source_reads += 1
            elif name in ("Read", "Grep") and "/src/" in args and "dewey" in args:
                source_reads += 1
            if name.startswith("mcp__dewey"):
                ontology_calls += 1

    return {
        "turns": turns,
        "cost_usd": round(cost, 4),
        "source_reads": source_reads,
        "ontology_calls": ontology_calls,
        "tool_result_chars": tool_result_chars,
        "source_result_chars": source_result_chars,
        "ontology_result_chars": ontology_result_chars,
    }


def one_run(task, condition, model, keep):
    task_dir = TASKS / task
    prompt = build_prompt(task_dir)
    workdir = Path(tempfile.mkdtemp(prefix=f"dewey-{task}-"))
    try:
        events, meta = run_agent(prompt, workdir, condition, model)
        record = {"task": task, "condition": condition, **meta, **summarise(events)}

        built = subprocess.run(
            ["cargo", "build", "--release"],
            cwd=workdir,
            capture_output=True,
            text=True,
        )
        record["built"] = built.returncode == 0
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
            }
        )
        return record
    finally:
        if not keep:
            shutil.rmtree(workdir, ignore_errors=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--task", required=True)
    parser.add_argument("--condition", default="bare", choices=["bare", "mcp"])
    parser.add_argument("--runs", type=int, default=1)
    parser.add_argument("--model", default=None)
    parser.add_argument("--label", default=None)
    parser.add_argument("--keep", action="store_true", help="keep the work tree")
    args = parser.parse_args()

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
