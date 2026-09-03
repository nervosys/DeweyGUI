Build a GUI program in Rust using the **dewey** crate.

The program is a single binary named `app`. Its Cargo.toml must depend on
Dewey by path:

```toml
[dependencies]
dewey = { package = "deweygui", path = "{{CRATE}}", default-features = false }
serde_json = "1"
```

## What it must display

Three widgets stacked vertically in a 240x120 window:

1. a `Label` with agent id `title`, reading `Counter`
2. a `Label` with agent id `count`, reading `Count: N` where `N` starts at `0`
3. a `Button` with agent id `inc`, reading `+`

## Behaviour

- Pressing the `inc` button increments the count by one.
- The `q` key quits.

{{CONTRACT}}

When you are done, this must work:

```
cargo run --release -- --headless 240x120 --script "click:inc,click:inc,key:q" --dump
```
