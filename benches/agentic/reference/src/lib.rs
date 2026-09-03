//! The contract, implemented once so a solution only writes its interface.
//!
//! A benchmark that made every attempt reimplement argument parsing would be
//! measuring argument parsing. The prompt gives the agent this module.

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
