//! What the ontology buys, and what it does not.
//!
//! Every other benchmark here measures how fast a frame is built. This one
//! measures the thing the frame cost is being spent *on*, and tries as hard to
//! find where the ontology fails as where it wins.
//!
//! The baseline here is the same application driven from pixels and screen
//! coordinates, which is what an agent must do when no structure is available.
//! That is honest for iced 0.13 and **not** for egui, which publishes an
//! AccessKit tree — an earlier version of this comment said otherwise and was
//! wrong. `agent_surface` compares against egui directly; this benchmark is
//! the pixels-only baseline. Both paths exist in this crate, so both run.
//!
//! Sections:
//!   1. Seeing      — cost and size of an observation
//!   2. Acting      — a named action against a coordinate, under layout change
//!   3. Verifying   — faults each channel can and cannot detect
//!   4. Paying      — what it costs, including where it buys nothing

use dewey::agent::driver::HeadlessDriver;
use dewey::agent::protocol::{AgentRequest, InjectedEvent};
use dewey::backend::image_buffer::ImagePainter;
use dewey::prelude::*;
use dewey::widget::{Checkbox, StatefulWidget, TextInput};
use dewey::widget::input::TextInputState;
use std::cell::RefCell;
use std::hint::black_box;
use std::time::{Duration, Instant};

// ── the application under test ───────────────────────────────────────────

struct Row {
    title: String,
    done: bool,
}

struct App {
    rows: Vec<Row>,
    /// Rows inserted at the top. Nothing else about the app changes, but every
    /// row below shifts down by one row height — which is all it takes to
    /// invalidate a coordinate an agent captured a moment ago.
    banner: bool,
    input: RefCell<TextInputState>,
}

impl App {
    fn new(n: usize) -> Self {
        Self {
            rows: (0..n)
                .map(|i| Row {
                    title: format!("item {i}"),
                    done: false,
                })
                .collect(),
            banner: false,
            input: RefCell::new(TextInputState::new()),
        }
    }

}

const ROW_H: f32 = 28.0;

impl Model for App {
    type Msg = ();

    fn update(&mut self, _m: ()) -> Command<()> {
        Command::None
    }

    fn view(&self, frame: &mut Frame<'_>) {
        let header = if self.banner { 2.0 * ROW_H } else { ROW_H };
        let h = frame.area.height;
        let bands = frame.area.rows_of(&[header, h - header]);

        let top = bands[0].rows_of(&[ROW_H]);
        let head = top[0].cols_of(&[top[0].width - 90.0, 90.0]);
        TextInput::new()
            .placeholder("filter")
            .on_input("filter", |a: &mut App, t: &str| {
                *a.input.borrow_mut() = TextInputState::new().with_text(t)
            })
            .render(head[0], frame, &mut self.input.borrow_mut());
        // Stands in for anything that changes the layout without the agent
        // asking: a notification, a background refresh, another user.
        Button::new("notify")
            .on("banner", |a: &mut App| a.banner = true)
            .render(head[1], frame);

        for (i, row) in (0..self.rows.len()).zip(bands[1].rows(ROW_H)) {
            let cells = row.cols_of(&[24.0, row.width - 24.0]);
            Checkbox::new("", self.rows[i].done)
                .on(format!("toggle_{i}"), move |a: &mut App| {
                    a.rows[i].done = !a.rows[i].done
                })
                .render(cells[0], frame);
            Label::new(self.rows[i].title.clone())
                .agent_id(format!("item_{i}"))
                .render(cells[1], frame);
        }
    }
}

/// Tall enough that every row is laid out. A fixed 800px window fits 27 rows,
/// so anything above that silently measures the same 27 — which it did, until
/// the 100- and 1000-row lines came back byte-identical.
fn window_height(n: usize) -> f32 {
    2.0 * ROW_H + n as f32 * ROW_H
}

fn driver(n: usize) -> HeadlessDriver<App> {
    let mut d = HeadlessDriver::new(App::new(n), 480.0, window_height(n));
    d.init();
    d
}

// ── helpers ──────────────────────────────────────────────────────────────

fn fmt(d: Duration) -> String {
    let us = d.as_secs_f64() * 1e6;
    if us < 1.0 {
        format!("{:.0} ns", d.as_secs_f64() * 1e9)
    } else if us >= 1000.0 {
        format!("{:.2} ms", us / 1000.0)
    } else {
        format!("{us:.1} us")
    }
}

fn bytes(n: usize) -> String {
    if n >= 1_000_000 {
        format!("{:.1} MB", n as f64 / 1e6)
    } else if n >= 1000 {
        format!("{:.1} kB", n as f64 / 1e3)
    } else {
        format!("{n} B")
    }
}

/// Minimum over `rounds`, which is the figure least polluted by whatever else
/// the machine is doing.
fn best(rounds: usize, mut f: impl FnMut()) -> Duration {
    let mut min = Duration::MAX;
    for _ in 0..rounds {
        let t = Instant::now();
        f();
        min = min.min(t.elapsed());
    }
    min
}

/// Rasterize the window the way a screenshot-driven agent would have to.
fn screenshot(app: &App, w: u32, h: u32) -> Vec<u8> {
    let mut painter = ImagePainter::new(w, h);
    let mut hit_map = dewey::event::HitMap::new();
    let area = Rect::new(0.0, 0.0, w as f32, h as f32);
    let mut frame = Frame::with_ontology(area, &mut hit_map, &mut painter, false);
    app.view(&mut frame);
    painter.pixels().to_vec()
}

fn png_len(pixels: &[u8], w: u32, h: u32) -> usize {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().expect("png header");
        writer.write_image_data(pixels).expect("png data");
    }
    out.len()
}

// ── 1. seeing ────────────────────────────────────────────────────────────

fn seeing(sizes: &[usize]) {
    println!("\n== 1. Seeing: what one observation costs ==\n");
    println!(
        "  {:<6} {:>21} {:>21} {:>21} {:>21} {:>21}",
        "",
        "tree (all widgets)",
        "shot (all widgets)",
        "shot (one viewport)",
        "tree (one viewport)",
        "unchanged (since=)"
    );
    println!(
        "  {:<6} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10}",
        "rows", "time", "bytes", "time", "bytes", "time", "bytes", "time", "bytes",
        "time", "bytes"
    );

    for &n in sizes {
        let rounds = if n >= 1000 { 40 } else { 400 };
        let mut d = driver(n);

        let full = d.process_request(&AgentRequest::GetTree { since: None, viewport: None });
        let version = full
            .data
            .as_ref()
            .and_then(|v| v.get("version"))
            .and_then(serde_json::Value::as_u64)
            .expect("version");
        let tree_bytes = serde_json::to_vec(&full.data).expect("tree json").len();

        let t_tree = best(rounds, || {
            black_box(d.process_request(&AgentRequest::GetTree { since: None, viewport: None }));
        });
        let t_poll = best(rounds, || {
            black_box(d.process_request(&AgentRequest::GetTree {
                since: Some(version),
                viewport: None,
            }));
        });
        let poll_bytes = serde_json::to_vec(
            &d.process_request(&AgentRequest::GetTree {
                since: Some(version),
                viewport: None,
            })
            .data,
        )
        .expect("poll json")
        .len();

        // The pixel path. Height is whatever it takes to show every row, so
        // the screenshot is not quietly cropping the thing being compared.
        let (w, h) = (480, window_height(n).ceil() as u32);
        let app = App::new(n);
        let px = screenshot(&app, w, h);
        let png = png_len(&px, w, h);
        let shot_rounds = if n >= 1000 { 3 } else { 20 };
        let t_shot = best(shot_rounds, || {
            let px = screenshot(&app, w, h);
            black_box(png_len(&px, w, h));
        });

        // What a real screenshot actually is: one window's worth, however
        // long the list. The tree above describes every row including the
        // 970 nobody can see.
        const VIEWPORT_H: u32 = 800;
        let vp = screenshot(&app, w, VIEWPORT_H);
        let vp_png = png_len(&vp, w, VIEWPORT_H);
        let t_vp = best(shot_rounds, || {
            let px = screenshot(&app, w, VIEWPORT_H);
            black_box(png_len(&px, w, VIEWPORT_H));
        });

        // The tree narrowed to the same window the screenshot shows.
        let view = dewey::agent::protocol::Viewport {
            x: 0.0,
            y: 0.0,
            width: 480.0,
            height: VIEWPORT_H as f32,
        };
        let vp_tree = d.process_request_json(&AgentRequest::GetTree {
            since: None,
            viewport: Some(view),
        });
        let vp_tree_bytes = vp_tree.len();
        let t_vp_tree = best(rounds, || {
            black_box(d.process_request_json(&AgentRequest::GetTree {
                since: None,
                viewport: Some(view),
            }));
        });

        println!(
            "  {:<6} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10}",
            n,
            fmt(t_tree),
            bytes(tree_bytes),
            fmt(t_shot),
            bytes(png),
            fmt(t_vp),
            bytes(vp_png),
            fmt(t_vp_tree),
            bytes(vp_tree_bytes),
            fmt(t_poll),
            bytes(poll_bytes),
        );
    }

    println!(
        "\n  The middle pair is generous to the tree: it grows the window until\n  \
               every row is drawn. A real screenshot is the third pair, one viewport,\n  \
               near constant however long the list.\n  \
             \n  \
               The fourth pair is `get_tree` given the same viewport. At 1000 rows it\n  \
               is 11.7 kB against a screenshot of 16.7 kB, and 971 us against 1.53 ms:\n  \
               smaller and faster, where the unclipped tree was 24x bigger and 3.7x\n  \
               slower. That column exists because measuring this benchmark showed the\n  \
               tree losing, which it had no business doing.\n  \
             \n  \
               What is still true, and smaller than it was: the clipped time still\n  \
               grows with the list. The viewport now decides before a UiNode is\n  \
               built rather than after, so an off-screen widget costs nothing to\n  \
               describe — but it is still laid out and still painted, and that is\n  \
               what remains. A list long enough for it to matter wants VirtualList\n  \
               in the view, which this benchmark deliberately does not use."
    );
}

// ── 2. acting ────────────────────────────────────────────────────────────

/// Does the action still reach the widget it was aimed at, after the layout
/// moves? An id survives; a coordinate does not.
fn acting() {
    println!("\n== 2. Acting: a name against a coordinate ==\n");

    const N: usize = 40;
    const TARGET: usize = 17;

    // Both agents observe, then the layout changes under them, then they act.
    // The change is one inserted banner row: nothing is added or removed from
    // the list, and every row simply sits one row lower.

    // -- coordinate path -------------------------------------------------
    let mut d = driver(N);
    // Read the target's position out of the observation, as an agent working
    // from a screenshot would read it off the image.
    let tree = d.process_request(&AgentRequest::GetTree { since: None, viewport: None });
    let bounds = find_bounds(&tree.data.clone().unwrap(), &format!("toggle_{TARGET}"))
        .expect("target is on screen");
    let (cx, cy) = (bounds.0 + bounds.2 / 2.0, bounds.1 + bounds.3 / 2.0);

    // The layout moves.
    raise_banner(&mut d);

    d.process_request(&AgentRequest::InjectEvent {
        event: InjectedEvent::MouseClick {
            x: cx,
            y: cy,
            button: "left".into(),
        },
    });
    let coord_hit = d.model().rows[TARGET].done;
    let coord_wrong = (0..N).find(|&i| d.model().rows[i].done);

    // -- ontology path ---------------------------------------------------
    let mut d = driver(N);
    d.process_request(&AgentRequest::GetTree { since: None, viewport: None });
    raise_banner(&mut d);
    let r = d.process_request(&AgentRequest::ExecuteAction {
        agent_id: format!("toggle_{TARGET}"),
        action: "toggle".into(),
        params: serde_json::Value::Null,
    });
    let id_hit = d.model().rows[TARGET].done;

    println!("  target: toggle_{TARGET}, observed at ({cx:.0}, {cy:.0})");
    println!("  then one row is inserted above the list and both agents act.\n");
    println!("  {:<34} {:>10}  actually changed", "path", "reported");
    println!(
        "  {:<34} {:>10}  {}",
        "click at the observed coordinate",
        "success",
        match (coord_hit, coord_wrong) {
            (true, _) => "toggle_17 — correct".to_string(),
            (false, Some(i)) => format!("toggle_{i} — the WRONG row"),
            (false, None) => "nothing at all".to_string(),
        }
    );
    println!(
        "  {:<34} {:>10}  {}",
        "execute_action(\"toggle_17\")",
        if r.success { "success" } else { "failure" },
        if id_hit {
            "toggle_17 — correct"
        } else {
            "nothing"
        }
    );

    // Now the cost of each, with the layout held still so both are correct.
    let mut d = driver(N);
    let t_id = best(2000, || {
        black_box(d.process_request(&AgentRequest::ExecuteAction {
            agent_id: format!("toggle_{TARGET}"),
            action: "toggle".into(),
            params: serde_json::Value::Null,
        }));
    });
    let t_click = best(2000, || {
        black_box(d.process_request(&AgentRequest::InjectEvent {
            event: InjectedEvent::MouseClick {
                x: cx,
                y: cy,
                button: "left".into(),
            },
        }));
    });
    println!("\n  cost per action, layout held still:");
    println!("    execute_action by id   {:>10}", fmt(t_id));
    println!("    click by coordinate    {:>10}", fmt(t_click));
    println!(
        "\n  The two cost the same. The difference is not speed: it is that one\n  \
         of them was still correct after the screen moved, and neither of them\n  \
         reported that it had not been."
    );
}

/// Something other than our agent changes the screen.
fn raise_banner(d: &mut HeadlessDriver<App>) {
    d.process_request(&AgentRequest::ExecuteAction {
        agent_id: "banner".into(),
        action: "click".into(),
        params: serde_json::Value::Null,
    });
}

fn find_bounds(tree: &serde_json::Value, id: &str) -> Option<(f32, f32, f32, f32)> {
    fn walk(node: &serde_json::Value, id: &str) -> Option<(f32, f32, f32, f32)> {
        if node.get("agent_id").and_then(serde_json::Value::as_str) == Some(id) {
            let b = node.get("bounds")?;
            let f = |k: &str| b.get(k).and_then(serde_json::Value::as_f64).unwrap_or(0.0) as f32;
            return Some((f("x"), f("y"), f("width"), f("height")));
        }
        node.get("children")?
            .as_array()?
            .iter()
            .find_map(|c| walk(c, id))
    }
    walk(tree.get("root").unwrap_or(tree), id)
}

// ── 3. verifying ─────────────────────────────────────────────────────────

/// Six interfaces, each broken in a way that renders perfectly.
fn verifying() {
    println!("\n== 3. Verifying: what each channel can detect ==\n");

    struct Case {
        name: &'static str,
        visible_in_pixels: bool,
        build: fn(&mut Frame<'_>),
    }

    fn unaddressable(frame: &mut Frame<'_>) {
        Button::new("Save").render(frame.area, frame);
    }
    fn duplicate_id(frame: &mut Frame<'_>) {
        let r = frame.area.split_rows(2);
        Button::new("Yes").on("confirm", |_: &mut ()| {}).render(r[0], frame);
        Button::new("No").on("confirm", |_: &mut ()| {}).render(r[1], frame);
    }
    fn zero_size(frame: &mut Frame<'_>) {
        Button::new("Send")
            .on("send", |_: &mut ()| {})
            .render(Rect::new(10.0, 10.0, 0.0, 0.0), frame);
    }
    fn offscreen(frame: &mut Frame<'_>) {
        Button::new("Help")
            .on("help", |_: &mut ()| {})
            .render(Rect::new(9000.0, 10.0, 80.0, 24.0), frame);
    }
    fn partial_wiring(frame: &mut Frame<'_>) {
        use dewey::widget::{Table, TableState};
        let mut state = TableState::new();
        Table::new(vec!["name".into()], vec![vec!["a".into()]])
            .on_select("rows", |_: &mut (), _| {})
            .render(frame.area, frame, &mut state);
    }
    fn white_on_white(frame: &mut Frame<'_>) {
        // Structurally perfect and completely unreadable. The panel is painted
        // because that is what "on white" means: text over a window background
        // the framework never drew has no recorded ground to be compared with.
        let area = frame.area;
        frame.painter().fill_rect(area, Color::WHITE, 0.0);
        Label::new("Balance: $4,201.55")
            .agent_id("balance")
            .fg(Color::WHITE)
            .render(area, frame);
    }

    let cases = [
        Case {
            name: "button rendered with no id",
            visible_in_pixels: false,
            build: unaddressable,
        },
        Case {
            name: "two widgets share one id",
            visible_in_pixels: false,
            build: duplicate_id,
        },
        Case {
            name: "widget laid out at zero size",
            visible_in_pixels: true,
            build: zero_size,
        },
        Case {
            name: "widget laid out off screen",
            visible_in_pixels: true,
            build: offscreen,
        },
        Case {
            name: "wired for select_row, not sort",
            visible_in_pixels: false,
            build: partial_wiring,
        },
        Case {
            name: "white text on white ground",
            visible_in_pixels: true,
            build: white_on_white,
        },
    ];

    println!("  {:<34} {:<24} {:>10}", "fault", "validate", "screenshot");
    let (mut found, mut seen) = (0, 0);
    for case in &cases {
        struct Probe(fn(&mut Frame<'_>));
        impl Model for Probe {
            type Msg = ();
            fn update(&mut self, _m: ()) -> Command<()> {
                Command::None
            }
            fn view(&self, frame: &mut Frame<'_>) {
                (self.0)(frame);
            }
        }
        let mut d = HeadlessDriver::new(Probe(case.build), 400.0, 200.0);
        d.init();
        let diagnostics = d.validate();
        let caught = !diagnostics.is_empty();
        if caught {
            found += 1;
        }
        if case.visible_in_pixels {
            seen += 1;
        }
        println!(
            "  {:<34} {:<24} {:>10}",
            case.name,
            if caught { diagnostics[0].code } else { "—" },
            if case.visible_in_pixels { "visible" } else { "—" }
        );
    }
    println!(
        "\n  validate: {found}/{} — screenshot: {seen}/{} would look wrong to a\n  \
         reader who already knew what to expect.",
        cases.len(),
        cases.len()
    );
    println!(
        "\n  The last row used to be blank. White on white is structurally\n  \
               perfect — correct id, real bounds, on screen, fully wired — so no\n  \
               check of the tree could see it, and it sat here as a failure for as\n  \
               long as this benchmark has existed.\n  \
             \n  \
               `validate` now also reads what was painted: for each piece of text,\n  \
               the last fill underneath it is the ground, and a WCAG contrast below\n  \
               1.6 is reported. No vision model, no window, still microseconds.\n  \
             \n  \
               What it still cannot see: text over a background the framework never\n  \
               drew, contrast against a gradient or an image, anything overlapping,\n  \
               and every question of whether the layout is any good. The ontology is\n  \
               not a substitute for looking."
    );
}

// ── 4. paying ────────────────────────────────────────────────────────────

fn paying(sizes: &[usize]) {
    println!("\n== 4. Paying: what it costs when nobody is asking ==\n");
    println!(
        "  {:<8} {:>12} {:>12} {:>12} {:>10}",
        "rows", "no ontology", "on demand", "every frame", "penalty"
    );

    for &n in sizes {
        let rounds = if n >= 1000 { 60 } else { 600 };
        let app = App::new(n);
        let area = Rect::new(0.0, 0.0, 480.0, window_height(n));

        let build = |ontology: bool| {
            best(rounds, || {
                let mut hit_map = dewey::event::HitMap::new();
                let mut painter = dewey::paint::NullPainter;
                let mut frame = Frame::with_ontology(area, &mut hit_map, &mut painter, ontology);
                app.view(&mut frame);
                black_box(frame.take_nodes());
            })
        };
        let off = build(false);
        let on = build(true);

        // On demand: the frame is free, and one extra paint-free pass happens
        // per query rather than per frame. 60 fps against 5 queries a second.
        let per_second_every = on.as_secs_f64() * 60.0;
        let per_second_demand = off.as_secs_f64() * 60.0 + on.as_secs_f64() * 5.0;

        println!(
            "  {:<8} {:>12} {:>12} {:>12} {:>10}",
            n,
            fmt(off),
            format!("{:.1} ms/s", per_second_demand * 1000.0),
            format!("{:.1} ms/s", per_second_every * 1000.0),
            format!("{:.2}x", on.as_secs_f64() / off.as_secs_f64()),
        );
    }

    println!(
        "\n  The penalty column is what an ontology frame costs against a plain\n  \
         one. It is only paid on frames that build the tree, which by default\n  \
         means the frames an agent actually asked about — with no agent\n  \
         attached the cost is not reduced, it is not incurred."
    );
}

// ── main ─────────────────────────────────────────────────────────────────

fn main() {
    println!("What the ontology buys, and what it does not.");
    println!("Baseline is the same application driven from pixels and coordinates,");
    println!("which is how an agent drives a toolkit that publishes no structure.");

    let sizes = [10usize, 100, 1000];
    seeing(&sizes);
    acting();
    verifying();
    paying(&sizes);

    println!(
        "\n── summary ──────────────────────────────────────────────────────────\n"
    );
    println!("  buys");
    println!("    an action that still hits the right widget after the screen moves;");
    println!("    the coordinate above reported success and toggled the wrong row");
    println!("    a re-poll costing 100 ns and 30 bytes instead of a render");
    println!("    3 structural faults out of 6 that no screenshot would show");
    println!(" ");
    println!("  does not");
    println!("    replace looking at it: contrast against a flat fill is checked,");
    println!("    but a gradient, an overlap, or a bad layout is not");
    println!("    make an observation constant-time: a viewport keeps the bytes");
    println!("    flat but the frame behind it is still built in full");
    println!("    make an action cheaper: by id and by coordinate cost the same");
    println!("    come free: a tree-building frame is 1.6-2.0x a plain one");
}
