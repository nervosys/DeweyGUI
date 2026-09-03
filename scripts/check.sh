#!/usr/bin/env bash
# Everything CI runs on a push, in the order that fails fastest.
#
# CONTRIBUTING has listed these commands for a while, and listing them is not
# the same as running them: this session pushed a red Test job once and a red
# Format job once, both of them things a local run would have caught in under
# a minute. One command is the difference.
#
#   scripts/check.sh          the fast gate: fmt, clippy, test, doc
#   scripts/check.sh --all    also the sibling crate and the benchmark
#                             workspaces, which live outside this workspace
#                             and which `cargo check` here never sees
set -euo pipefail

cd "$(dirname "$0")/.."
export RUSTFLAGS="${RUSTFLAGS:--Dwarnings}"

run() {
    echo "── $*"
    "$@"
}

run cargo fmt --check
run cargo clippy --all-targets -- -D warnings
run cargo test
# Accessibility is off by default and must not rot; it also pulls a different
# egui feature set, which is where the version skew lives.
run cargo test --features accesskit
export RUSTDOCFLAGS="${RUSTDOCFLAGS:--Dwarnings}"
run cargo doc --no-deps

if [ "${1:-}" = "--all" ]; then
    run cargo test --manifest-path agpu/Cargo.toml
    for bench in comparative scaffold; do
        run cargo check --all-targets --manifest-path "benches/$bench/Cargo.toml"
    done
    # Compiling a benchmark is not running one. These three assert something
    # before they time anything.
    run cargo run --release --bin allocs --manifest-path benches/comparative/Cargo.toml
    run cargo run --release --bin agent_loop --manifest-path benches/comparative/Cargo.toml
    run cargo run --release --bin agent_task --manifest-path benches/scaffold/Cargo.toml
fi

echo
echo "All checks passed."
