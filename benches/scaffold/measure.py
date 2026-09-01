"""Scaffold-cost metrics: what an agent must produce, and how fast it finds out
whether the code compiles."""
import re, subprocess, time, os, sys

FILES = {
    "Dewey (plain)": "src/bin/counter_dewey_plain.rs",
    "Dewey (agent)": "src/bin/counter_dewey.rs",
    "egui 0.31": "src/bin/counter_egui.rs",
    "iced 0.13": "src/bin/counter_iced.rs",
}
BINS = {"Dewey (plain)": "counter_dewey_plain", "Dewey (agent)": "counter_dewey", "egui 0.31": "counter_egui", "iced 0.13": "counter_iced"}

def source_metrics(path):
    src = open(path, encoding="utf-8").read()
    lines = src.splitlines()
    code = [l for l in lines if l.strip() and not l.strip().startswith("//")]
    toks = re.findall(r"[A-Za-z_][A-Za-z0-9_]*|\d+|\S", src)
    return len(code), len(src), len(toks)

def check_latency(bin_name, path, rounds=3):
    """Time `cargo check` after touching the file — the agent's edit→error loop."""
    best = None
    for _ in range(rounds):
        os.utime(path, None)
        t = time.perf_counter()
        subprocess.run(["cargo", "check", "--release", "--bin", bin_name],
                       capture_output=True, check=False)
        e = time.perf_counter() - t
        best = e if best is None else min(best, e)
    return best

print(f"{'Framework':<15} {'code':>5} {'bytes':>6} {'~tok':>6} {'check':>8}")
rows = {}
for name, path in FILES.items():
    c, b, t = source_metrics(path)
    lat = check_latency(BINS[name], path)
    rows[name] = (c, b, t, lat)
    print(f"{name:<15} {c:>5} {b:>6} {t:>6} {lat:>7.2f}s")

base = rows["Dewey (plain)"]
print()
for name, (c, b, t, lat) in rows.items():
    print(f"{name:<15} code {c/base[0]:.2f}x   tokens {t/base[2]:.2f}x   check {lat/base[3]:.2f}x  (vs Dewey plain)")
