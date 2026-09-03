# Contributing to Dewey

Thank you for your interest in contributing to Dewey! This document provides guidelines
for contributing to the project.

## Getting Started

### Prerequisites

- Rust 1.85+ (2024 edition)
- A GPU-capable system for running GUI examples (headless tests work without one)

### Building

```bash
cargo build
```

### Running Tests

```bash
cargo test
```

### Running Clippy

```bash
cargo clippy --all-targets -- -D warnings
```

### Running Benchmarks

```bash
cargo bench
```

### Running Examples

```bash
# Headless (no GPU required)
cargo run --example ontology_explorer
cargo run --example agent_headless

# GUI (requires display)
cargo run --example hello
cargo run --example counter
cargo run --example showcase
cargo run --example canvas_drawing
cargo run --example agent_demo
```

## Development Workflow

1. Fork the repository and create a feature branch from `master`.
2. Make your changes with clear, atomic commits.
3. Run the checks:
   ```bash
   scripts/check.sh          # fmt, clippy, test, doc — what CI runs on a push
   scripts/check.sh --all    # and the sibling crate and benchmark workspaces
   ```
   `agpu/` and `benches/*` are separate workspaces, so the plain `cargo check`
   here never compiles them; both benchmark crates had silently stopped
   building before CI started covering them.
4. Open a pull request against `master`.

## Code Style

- Follow standard Rust formatting (`cargo fmt`).
- Add `#[must_use]` to pure-value constructors and builder methods.
- Every public type and function must have a rustdoc comment.
- Prefer `compact_str::CompactString` over `String` for widget identifiers.

## Adding a Widget

1. Create `src/widget/<name>.rs` implementing the widget struct.
2. Implement the `Widget` trait (`render`, `handle_event`, `layout_hint`).
3. Implement the `Discoverable` trait (`schema`, `capabilities`, `actions`, `agent_state`, `execute_action`).
4. Add `pub mod <name>;` to `src/widget/mod.rs` and re-export from the prelude.
5. Add unit tests in the widget file and integration tests in `tests/integration.rs`.
6. Update the widget count in `src/lib.rs` and `README.md`.

## Adding an Agent Protocol Request

1. Add the variant to `AgentRequest` in `src/agent/protocol.rs`.
2. Handle it in `HeadlessDriver::process_request` in `src/agent/driver.rs`.
3. Add integration tests in `tests/integration.rs`.
4. Document the request/response schema in `docs/agent-protocol.md`.

## Commit Messages

Use clear, imperative-mood commit messages:

```
Add ColorPicker widget with HSV selection
Fix focus ring wrapping on empty registry
Improve batch action error reporting
```

## Licensing

By contributing, you agree that your contributions will be licensed under the
AGPL-3.0-or-later license, consistent with the project's existing license.
