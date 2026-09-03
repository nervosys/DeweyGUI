# Harness contract

Every benchmark program implements this so that one verifier scores every
attempt identically. It is injected verbatim into the prompt for every task.

## Command line

```
<prog> --headless <W>x<H> --script "<STEP>,<STEP>,..." --dump
```

- `--headless WxH` — lay out against a window exactly `W` by `H` logical
  pixels. Never open a window. The program must run with stdin closed and
  stdout redirected to a file.
- `--script` — a comma-separated list of steps, applied in order. A step is
  either `key:<NAME>` or `click:<AGENT_ID>`.
- `--dump` — write frames to stdout as described below.

## Frame dump format

1. Render the initial frame and print it.
2. For each step: apply it, render, and print the resulting frame.
3. If a step causes the program to quit, exit **without** printing a frame for
   it, so the frame count proves the quit worked.

A frame is `HeadlessDriver::snapshot()`: one line per widget, in render order,
`<type> <id> <x>,<y> <w>x<h> <key>=<value> ...`, with properties sorted and
bounds rounded to whole pixels. Frames are separated by a single form feed
(`\x0c`) on its own line. Exit code 0 on success.

## Step names

| Step | Meaning |
|---|---|
| `key:Up` `key:Down` `key:Left` `key:Right` | arrow keys |
| `key:Enter` `key:Esc` `key:Tab` `key:Space` | named keys |
| `key:a` | that character |
| `click:add_btn` | a click on the widget with that agent id |

## The runner, which is already written for you

`src/contract.rs` is already in your working directory. Do not rewrite it and
do not modify it. Add `mod contract;` to your `main.rs` and call it:

```rust
mod contract;

fn main() -> std::io::Result<()> {
    contract::run_contract(App { /* your initial state */ })?;
    Ok(())
}
```

It needs `serde_json` in your `Cargo.toml` alongside Dewey.

## Why this exists

The verifier scores rendered widgets, never source code. It cannot see how a
frame was produced, so it cannot reward a particular style of writing one. The
cost is that ignoring the contract scores zero whatever the interface looks
like; those runs are reported separately as `contract_failed` rather than as
behavioural failures.
