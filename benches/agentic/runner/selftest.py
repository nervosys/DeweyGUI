#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Prove the verifier can tell a working interface from one that looks working.

A scoring function nobody has scored is the same shape as everything else this
project has spent a week finding: it compiles, it produces a number, and the
number means nothing. So the harness is run against solutions whose answers are
known before it is run against a model.

    reference solutions   must score 1.000
    the broken solution   must score below 1.000, and fail `operable`
    a program that is not there  must be reported as a contract failure

Exits non-zero if any of that stops being true. Needs no model and no network.
"""
import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
TASKS = ROOT / "tasks"
REFERENCE = ROOT / "reference"

sys.path.insert(0, str(HERE))
from run import summarise  # noqa: E402
from verify import verify  # noqa: E402


def build():
    """Build the reference solutions, and say where the binaries went."""
    print("building the reference solutions ...", flush=True)
    proc = subprocess.run(
        [
            "cargo",
            "build",
            "--release",
            "--manifest-path",
            str(REFERENCE / "Cargo.toml"),
            "--message-format",
            "json-render-diagnostics",
        ],
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        sys.stderr.write(proc.stderr)
        raise SystemExit("selftest: the reference solutions do not build")

    binaries = {}
    for line in proc.stdout.splitlines():
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            continue
        if message.get("reason") == "compiler-artifact" and message.get("executable"):
            name = message["target"]["name"]
            binaries[name] = message["executable"]
    return binaries


def report(label, result):
    mark = "ok  " if result["ok"] else "FAIL"
    print(
        f"  {mark} {label:<34} score {result['score']:.3f}"
        f"  ({result['checks_passed']}/{result['checks_total']})"
    )
    if not result["ok"]:
        for check in result["checks"]:
            if not check["passed"]:
                print(f"         {check['id']}: {check['why']}")
    return result["ok"]


def main():
    binaries = build()
    print()

    ok = True

    # A reference solution answers every check. If one of these ever fails, the
    # framework changed under a solution the prompt implies is achievable, and
    # the benchmark would blame every future attempt for it.
    for task, binary in [("t1-counter", "t1_counter"), ("t2-todo", "t2_todo")]:
        if binary not in binaries:
            print(f"  FAIL {task}: `{binary}` was not built")
            ok = False
            continue
        result = verify(TASKS / task, binaries[binary])
        result["ok"] = result["score"] == 1.0 and not result["contract_failed"]
        ok &= report(f"reference {task}", result)

    # The broken solution renders correctly and cannot be operated. A verifier
    # that scores it 1.000 is reading pixels, not interfaces.
    if "t1_counter_broken" in binaries:
        result = verify(TASKS / "t1-counter", binaries["t1_counter_broken"])
        failed = {c["id"] for c in result["checks"] if not c["passed"]}
        # `addressable` is the check that has to fail. An interactive widget
        # rendered without an id never reaches the UI tree at all, so no
        # amount of reading the snapshot reveals an anonymous one — the first
        # version of this harness had a `no_anonymous_widgets` check that
        # could not fail, which is the shape it exists to catch. `validate`
        # reports `unaddressable_widget`; a frame cannot.
        result["ok"] = result["score"] < 1.0 and "addressable" in failed
        ok &= report("broken t1-counter is caught", result)
    else:
        print("  FAIL t1_counter_broken was not built")
        ok = False

    # The transcript reader, against a recording whose answers are known
    # before it runs. `run.py` needs a model and a paid API call; the part of
    # it that turns a transcript into numbers does not, and it is the part
    # that decides what the benchmark reports.
    fixture = HERE / "fixtures" / "transcript.jsonl"
    events = [
        json.loads(line)
        for line in fixture.read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]
    got = summarise(events)
    want = {
        # Six assistant messages: five ordinary ones and a malformed one
        # whose `message` is a bare string. A real transcript carries those
        # and the first paid run crashed on one.
        "turns": 6,
        "cost_usd": 0.2137,
        # A `Read` and a `Grep` inside Dewey's own src; the `Write` of the
        # agent's own file is work, not going around the ontology.
        "source_reads": 2,
        "ontology_calls": 1,
        # 100 + 25 + 50 + 2, so a miscounted result shows up as a wrong total.
        "tool_result_chars": 177,
        "source_result_chars": 150,
        "ontology_result_chars": 25,
    }
    wrong = {k: (got.get(k), v) for k, v in want.items() if got.get(k) != v}
    if wrong:
        print("  FAIL the transcript reader")
        for key, (g, w) in wrong.items():
            print(f"         {key}: got {g}, expected {w}")
        ok = False
    else:
        print("  ok   the transcript reader counts what it claims")

    # The runner pasted into the contract must be the one the reference
    # compiles. If they drift, every attempt is given code that does not build
    # while the reference quietly keeps working, and the benchmark blames the
    # model for it.
    lib = (REFERENCE / "src" / "lib.rs").read_text(encoding="utf-8")
    lib = lib.replace("\r\n", "\n")
    body = "\n".join(
        line for line in lib.split("\n") if not line.startswith("//!")
    ).strip()
    contract = (TASKS / "contract.md").read_text(encoding="utf-8")
    contract = contract.replace("\r\n", "\n")
    if body in contract:
        print("  ok   the contract ships the runner the reference compiles")
    else:
        print("  FAIL the contract's runner has drifted from reference/src/lib.rs")
        ok = False

    # A program that does not exist is a contract failure and not a score of
    # zero on the interface, because the two mean different things about an
    # attempt.
    result = verify(TASKS / "t1-counter", "definitely-not-a-program")
    result["ok"] = result["contract_failed"] and result["score"] == 0.0
    ok &= report("a missing program is a contract failure", result)

    print()
    if not ok:
        raise SystemExit("selftest: the verifier does not behave as claimed")
    print("the verifier scores what it claims to score")


if __name__ == "__main__":
    main()
