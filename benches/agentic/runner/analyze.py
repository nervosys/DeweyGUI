#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Summarise recorded runs, with intervals rather than a single number.

    analyze.py results/bare/runs.jsonl [results/mcp/runs.jsonl ...]

Medians, because agent runs are long-tailed and a mean is dragged around by
one bad attempt. Bootstrap confidence intervals, because three runs of
anything can say whatever you want them to and the interval is what says
whether they did. A difference whose interval spans zero is reported as
spanning zero and not as a result.
"""
import argparse
import json
import random
import statistics
import sys
from pathlib import Path

RESAMPLES = 10_000
METRICS = ("score", "turns", "cost_usd", "source_reads", "ontology_calls")


def load(path):
    runs = []
    for line in Path(path).read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if line:
            runs.append(json.loads(line))
    return runs


def median_ci(values, confidence=0.95, seed=0):
    """The median, and a bootstrap interval around it."""
    if not values:
        return None, (None, None)
    if len(values) == 1:
        return values[0], (values[0], values[0])
    rng = random.Random(seed)
    medians = []
    n = len(values)
    for _ in range(RESAMPLES):
        sample = [values[rng.randrange(n)] for _ in range(n)]
        medians.append(statistics.median(sample))
    medians.sort()
    lo = medians[int((1 - confidence) / 2 * RESAMPLES)]
    hi = medians[int((1 + confidence) / 2 * RESAMPLES) - 1]
    return statistics.median(values), (lo, hi)


def describe(label, runs):
    print(f"\n{label}  n={len(runs)}")
    if not runs:
        return {}
    contract = sum(1 for r in runs if r.get("contract_failed"))
    built = sum(1 for r in runs if r.get("built"))
    print(f"  built {built}/{len(runs)}   contract failures {contract}")

    summary = {}
    for metric in METRICS:
        values = [r[metric] for r in runs if metric in r]
        if not values:
            continue
        med, (lo, hi) = median_ci(values)
        summary[metric] = values
        print(f"  {metric:<16} {med:>8.3f}   95% CI [{lo:.3f}, {hi:.3f}]")
    return summary


def compare(a_label, a, b_label, b, seed=1):
    """The difference of medians, with an interval, for each metric."""
    print(f"\n{b_label} minus {a_label}:")
    rng = random.Random(seed)
    for metric in METRICS:
        if metric not in a or metric not in b:
            continue
        xs, ys = a[metric], b[metric]
        if len(xs) < 2 or len(ys) < 2:
            print(f"  {metric:<16} too few runs to say anything")
            continue
        observed = statistics.median(ys) - statistics.median(xs)
        diffs = []
        for _ in range(RESAMPLES):
            rx = [xs[rng.randrange(len(xs))] for _ in range(len(xs))]
            ry = [ys[rng.randrange(len(ys))] for _ in range(len(ys))]
            diffs.append(statistics.median(ry) - statistics.median(rx))
        diffs.sort()
        lo = diffs[int(0.025 * RESAMPLES)]
        hi = diffs[int(0.975 * RESAMPLES) - 1]
        spans_zero = lo <= 0 <= hi
        note = "spans zero" if spans_zero else "excludes zero"
        print(f"  {metric:<16} {observed:>+8.3f}   95% CI [{lo:+.3f}, {hi:+.3f}]  {note}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("files", nargs="+")
    args = parser.parse_args()

    summaries = []
    for path in args.files:
        runs = load(path)
        summaries.append((path, describe(path, runs)))

    if len(summaries) == 2:
        (a_label, a), (b_label, b) = summaries
        compare(a_label, a, b_label, b)
    elif len(summaries) > 2:
        print("\nmore than two files: reporting each, comparing none")

    print()


if __name__ == "__main__":
    main()
