Build a GUI program in Rust using the **dewey** crate.

The program is a single binary named `app`. Its Cargo.toml must depend on
Dewey by path:

```toml
[dependencies]
dewey = { package = "deweygui", path = "{{CRATE}}", default-features = false }
serde_json = "1"
```

## What it must display

In a 320x240 window, a to-do list:

- a `Button` with agent id `add`, reading `Add`, which appends a new item
  titled `item N` where `N` is the number of items after appending
- one row per item, each with a `Checkbox` with agent id `toggle_<i>` and a
  `Label` with agent id `item_<i>`, where `<i>` is the item's index from zero
- a `Label` with agent id `remaining`, reading `N left`, counting the items
  that are not checked

Start with no items.

## Behaviour

- The `add` button appends an item.
- A `toggle_<i>` checkbox flips whether that item is done.
- The `q` key quits.

{{CONTRACT}}

When you are done, this must work:

```
cargo run --release -- --headless 320x240 --script "click:add,click:add,click:toggle_0,key:q" --dump
```
