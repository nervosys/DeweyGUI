#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Prove the inspection task is answerable, and answerable only by asking.

The twelve runs of 8 September found that no agent consulted the ontology, and
the honest caveat was that neither task ever put a running application in front
of one. `t3-inspect` is the task that does. Before it costs anything, three
things have to be true, and this checks all of them without a model:

    the answer is reachable   a single `get_tree` and some counting gets it
    the source cannot say     the same source, a different answer per seed
    guessing is a bad plan    the most common answer is far short of always

Nine harness defects have been paid for at roughly a dollar each. This is the
cheapest place to find the tenth.
"""
import json
import os
import subprocess
import sys
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
SUBJECT = ROOT / "subject"
TASK = ROOT / "tasks" / "t3-inspect"

sys.path.insert(0, str(HERE))
from run import subject_binary, subject_truth  # noqa: E402


def observe(binary, seed):
    """Answer the question the way an agent would: ask, then count."""
    env = dict(os.environ, DEWEY_SUBJECT_SEED=str(seed))
    proc = subprocess.run(
        [binary],
        input='{"id":"1","request":{"type":"get_tree"}}\n',
        capture_output=True,
        text=True,
        env=env,
    )
    reply = json.loads(proc.stdout.splitlines()[0])
    nodes = reply["data"]["root"]["children"]
    titles = [n.get("state", {}).get("text", "") for n in nodes]
    checked = [n.get("state", {}).get("checked") for n in nodes]
    # Rows are a checkbox followed by its label, after the count line.
    return sum(
        1
        for i in range(1, len(nodes) - 1)
        if checked[i] is False and str(titles[i + 1]).startswith("urgent")
    )


def main():
    print("building the application under inspection ...", flush=True)
    binary = subject_binary()
    ok = True

    # 1. Reachable. If a `get_tree` and some counting cannot produce the
    # answer, the task is unanswerable and every run would score zero while
    # looking like a finding about agents.
    seeds = [11, 42, 99, 12345, 7, 2024]
    wrong = [
        (s, subject_truth(binary, s), observe(binary, s))
        for s in seeds
        if subject_truth(binary, s) != observe(binary, s)
    ]
    if wrong:
        print("  FAIL the answer cannot be reached by asking:")
        for seed, truth, seen in wrong:
            print(f"         seed {seed}: truth {truth}, observed {seen}")
        ok = False
    else:
        print(f"  ok   answerable by one `get_tree` on {len(seeds)} seeds")

    # 2. The source cannot say. The whole design rests on the answer moving
    # while the code stands still; if every seed gave the same number, reading
    # the source once would be enough and the task would measure nothing.
    answers = Counter(subject_truth(binary, s) for s in range(1, 121))
    if len(answers) < 4:
        print(f"  FAIL the answer barely varies across seeds: {dict(answers)}")
        ok = False
    else:
        print(f"  ok   {len(answers)} distinct answers over 120 seeds")

    # 3. Guessing is a bad plan. An agent that never looks and always writes
    # the most common answer must do clearly worse than one that looks.
    common, hits = answers.most_common(1)[0]
    rate = hits / sum(answers.values())
    if rate > 0.4:
        print(f"  FAIL guessing `{common}` every time scores {rate:.0%}")
        ok = False
    else:
        print(f"  ok   guessing `{common}` every time scores only {rate:.0%}")

    # 4. The prompt must not give the game away. A seed or a truth in the text
    # would turn an inspection task into a reading task, which is the mistake
    # this whole file exists to avoid making twice.
    prompt = (TASK / "prompt.md").read_text(encoding="utf-8")
    leaked = [w for w in ("DEWEY_SUBJECT_SEED", "--truth", "seed=") if w in prompt]
    if leaked:
        print(f"  FAIL the prompt leaks {leaked}")
        ok = False
    else:
        print("  ok   the prompt names no seed and no answer")

    print()
    if not ok:
        raise SystemExit("selftest_drive: the inspection task is not sound")
    print("the inspection task can be answered, and only by looking")


if __name__ == "__main__":
    main()
