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

## The runner, which you copy verbatim

Create `src/contract.rs` with exactly this, and `mod contract;` in your
`main.rs`. It is given to you so that every attempt is scored on its
interface rather than on its argument parsing.

```rust
use dewey::agent::driver::HeadlessDriver;
use dewey::agent::protocol::{AgentRequest, InjectedEvent};
use dewey::runtime::Model;

/// Run the contract: lay out at `WxH`, apply each step, print each snapshot.
///
/// Returns the number of frames printed. A step that quits the application
/// ends the run without printing a frame for it, so the count proves the quit
/// worked.
pub fn run_contract<M: Model + 'static>(model: M) -> std::io::Result<usize> {
    let args: Vec<String> = std::env::args().collect();
    let value = |flag: &str| -> Option<String> {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };

    let (w, h) = value("--headless")
        .and_then(|s| {
            let (w, h) = s.split_once('x')?;
            Some((w.parse().ok()?, h.parse().ok()?))
        })
        .unwrap_or((240.0, 120.0));
    let script: Vec<String> = value("--script")
        .map(|s| {
            s.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    let mut driver = HeadlessDriver::new(model, w, h);
    driver.init();

    let mut frames = 0;
    print_frame(&mut driver, &mut frames);

    for step in script {
        let request = match step.split_once(':') {
            // A physical click, hit-tested, rather than the literal action
            // name "click". A `Checkbox` advertises `toggle` and refuses
            // `click` — correctly, since an agent that is told a call worked
            // has no reason to look again. Pressing a widget means whatever
            // that widget registered.
            Some(("click", id)) => {
                let Some(centre) = centre_of(&mut driver, id) else {
                    continue;
                };
                AgentRequest::InjectEvent {
                    event: InjectedEvent::MouseClick {
                        x: centre.0,
                        y: centre.1,
                        button: "left".into(),
                    },
                }
            }
            Some(("key", name)) => AgentRequest::InjectEvent {
                event: InjectedEvent::Key {
                    code: name.to_string(),
                    modifiers: Vec::new(),
                },
            },
            _ => continue,
        };
        driver.process_request(&request);
        if !driver.is_running() {
            break;
        }
        print_frame(&mut driver, &mut frames);
    }

    Ok(frames)
}

/// Where a widget is, read out of the snapshot the application just rendered.
///
/// The snapshot rather than the JSON tree: it is the same text the verifier
/// scores, so a widget the verifier can see is a widget this can click, and
/// there is no second format to fall out of step with.
fn centre_of<M: Model + 'static>(
    driver: &mut HeadlessDriver<M>,
    agent_id: &str,
) -> Option<(f32, f32)> {
    let needle = format!("#{agent_id} [");
    for line in driver.snapshot().lines() {
        let Some(at) = line.find(&needle) else { continue };
        let rest = &line[at + needle.len()..];
        let bounds = rest.split(']').next()?;
        // `x,y wxh`
        let (origin, size) = bounds.split_once(' ')?;
        let (x, y) = origin.split_once(',')?;
        let (w, h) = size.split_once('x')?;
        let x: f32 = x.trim().parse().ok()?;
        let y: f32 = y.trim().parse().ok()?;
        let w: f32 = w.trim().parse().ok()?;
        let h: f32 = h.trim().parse().ok()?;
        return Some((x + w / 2.0, y + h / 2.0));
    }
    None
}

fn print_frame<M: Model + 'static>(driver: &mut HeadlessDriver<M>, frames: &mut usize) {
    if *frames > 0 {
        println!("\u{c}");
    }
    print!("{}", driver.snapshot());
    *frames += 1;
}
```

Your `main` is then:

```rust
mod contract;

fn main() -> std::io::Result<()> {
    contract::run_contract(App { /* your initial state */ })?;
    Ok(())
}
```

## Why this exists

The verifier scores rendered widgets, never source code. It cannot see how a
frame was produced, so it cannot reward a particular style of writing one. The
cost is that ignoring the contract scores zero whatever the interface looks
like; those runs are reported separately as `contract_failed` rather than as
behavioural failures.
