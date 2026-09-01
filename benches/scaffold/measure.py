"""Scaffold-cost metrics: what an agent must produce, and how fast it finds out
whether the code compiles."""
import re, subprocess, time, os

APPS = {
    "counter (simple)": {
        "Dewey (plain)": "counter_dewey_plain",
        "Dewey (agent)": "counter_dewey",
        "egui 0.31": "counter_egui",
        "iced 0.13": "counter_iced",
    },
    "todomvc (complex)": {
        "Dewey (plain)": "todo_dewey_plain",
        "Dewey (agent)": "todo_dewey",
        "egui 0.31": "todo_egui",
        "iced 0.13": "todo_iced",
    },
}

def source_metrics(path):
    """Code lines, bytes, and tokens - all with comments stripped.

    Comments must not count: they are explanatory prose written for a human
    reader of this benchmark, and including them once made a strictly smaller
    program score higher than the one it was derived from.
    """
    src = open(path, encoding="utf-8").read()
    code = [l for l in src.splitlines() if l.strip() and not l.strip().startswith("//")]
    body = "
".join(code)
    toks = re.findall(r"[A-Za-z_][A-Za-z0-9_]*|\d+|\S", body)
    return len(code), len(body), len(toks)

def _one_check(bin_name, path):
    os.utime(path, None)
    t = time.perf_counter()
    subprocess.run(["cargo", "check", "--bin", bin_name], capture_output=True, check=False)
    return time.perf_counter() - t

def check_latencies(bins, rounds=4):
    """Time `cargo check` after touching each file - the agent's edit/error loop.

    Interleaved across frameworks and reported as the minimum: run
    sequentially, whichever bin goes first absorbs shared dependency work and
    looks slower, which made an earlier version of this script rank the same
    code 1.6x apart between runs.
    """
    best = {name: None for name in bins}
    for _ in range(rounds):
        for name, b in bins.items():
            e = _one_check(b, "src/bin/%s.rs" % b)
            if best[name] is None or e < best[name]:
                best[name] = e
    return best

for app, bins in APPS.items():
    print("\n=== %s ===" % app)
    print("%-15s %5s %6s %6s %8s" % ("Framework", "code", "bytes", "~tok", "check"))
    for name, b in bins.items():   # warm up so no bin absorbs first-run dep work
        _one_check(b, "src/bin/%s.rs" % b)
    lats = check_latencies(bins)
    rows = {}
    for name, b in bins.items():
        c, by, t = source_metrics("src/bin/%s.rs" % b)
        rows[name] = (c, by, t, lats[name])
        print("%-15s %5d %6d %6d %7.2fs" % (name, c, by, t, lats[name]))
    base = rows["Dewey (plain)"]
    print()
    for name, (c, by, t, lat) in rows.items():
        print("%-15s code %.2fx   tokens %.2fx   check %.2fx  (vs Dewey plain)"
              % (name, c / base[0], t / base[2], lat / base[3]))
