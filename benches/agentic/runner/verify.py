#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Score one attempt at a task by what it rendered.

The verifier reads frames and never source code. It cannot see how a frame was
produced, so it cannot reward a particular way of writing one — the cost is
that ignoring the contract scores zero however good the interface is, and such
runs are reported as `contract_failed` rather than as behavioural failures.

Usage:
    verify.py <task-dir> <program> [--cwd DIR]

Prints a JSON result to stdout and exits 0 whether or not the attempt passed;
a non-zero exit means the verifier itself could not run.
"""
import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

FORM_FEED = "\x0c"

# Widget types that do nothing unless something can address them. A `Label`
# with no id is a caption; a `Button` with no id is a defect that renders
# perfectly.
INTERACTIVE = (
    "Button",
    "Checkbox",
    "Radio",
    "TextInput",
    "TextArea",
    "Slider",
    "Select",
    "List",
    "Tabs",
    "Table",
    "Toolbar",
    "Menu",
)


def run_attempt(program, task, cwd):
    """Run the program under the contract and return (frames, error)."""
    window = task["window"]
    script = ",".join(task["script"])
    cmd = [
        str(program),
        "--headless",
        f"{window['w']}x{window['h']}",
        "--script",
        script,
        "--dump",
    ]
    try:
        proc = subprocess.run(
            cmd,
            cwd=cwd,
            capture_output=True,
            text=True,
            timeout=60,
            stdin=subprocess.DEVNULL,
        )
    except FileNotFoundError:
        return [], "program not found"
    except subprocess.TimeoutExpired:
        return [], "timed out after 60s"

    if proc.returncode != 0:
        return [], f"exit {proc.returncode}: {proc.stderr.strip()[:200]}"

    text = proc.stdout.replace("\r\n", "\n")
    frames = [f.strip("\n") for f in text.split(FORM_FEED) if f.strip()]
    return frames, None


def widgets(frame):
    """Every widget line in a frame as (type, id_or_None)."""
    out = []
    for line in frame.split("\n"):
        stripped = line.strip()
        if not stripped:
            continue
        parts = stripped.split()
        kind = parts[0]
        ident = None
        for part in parts[1:]:
            if part.startswith("#"):
                ident = part[1:]
                break
        out.append((kind, ident))
    return out


def apply_check(check, frames):
    """Whether one check passes, and why not when it does not."""
    kind = check["kind"]

    if kind == "frame_count":
        got = len(frames)
        want = check["equals"]
        return got == want, f"{got} frames, expected {want}"

    if kind == "frames_differ":
        a, b = check["frames"]
        if max(a, b) >= len(frames):
            return False, f"only {len(frames)} frames"
        return frames[a] != frames[b], "frames are identical"

    index = check.get("frame", 0)
    if index >= len(frames):
        return False, f"no frame {index}; only {len(frames)}"
    frame = frames[index]

    if kind == "contains":
        pattern = check["pattern"]
        return bool(re.search(pattern, frame)), f"no match for /{pattern}/"

    if kind == "absent":
        pattern = check["pattern"]
        return not re.search(pattern, frame), f"unexpected match for /{pattern}/"

    if kind == "has_widget":
        want = check["agent_id"]
        found = any(i == want for _, i in widgets(frame))
        return found, f"no widget with id `{want}`"

    if kind == "no_anonymous_widgets":
        anon = [k for k, i in widgets(frame) if k in INTERACTIVE and i is None]
        return not anon, f"interactive widgets with no id: {sorted(set(anon))}"

    raise SystemExit(f"verify.py: unknown check kind `{kind}`")


def score(task, frames, error):
    """The result record for one attempt."""
    results = []
    contract_failed = error is not None
    for check in task["checks"]:
        if error is not None:
            passed, why = False, error
        else:
            passed, why = apply_check(check, frames)
        if check.get("contract") and not passed:
            contract_failed = True
        results.append(
            {
                "id": check["id"],
                "kind": check["kind"],
                "passed": passed,
                "why": None if passed else why,
            }
        )

    passed = sum(1 for r in results if r["passed"])
    return {
        "task": task["id"],
        "score": passed / len(results) if results else 0.0,
        "checks_passed": passed,
        "checks_total": len(results),
        "contract_failed": contract_failed,
        "frames": len(frames),
        "error": error,
        "checks": results,
    }


def verify(task_dir, program, cwd=None):
    task = json.loads((Path(task_dir) / "checks.json").read_text(encoding="utf-8"))
    frames, error = run_attempt(program, task, cwd)
    return score(task, frames, error)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("task_dir")
    parser.add_argument("program")
    parser.add_argument("--cwd", default=None)
    args = parser.parse_args()

    result = verify(args.task_dir, args.program, args.cwd)
    json.dump(result, sys.stdout, indent=2)
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
