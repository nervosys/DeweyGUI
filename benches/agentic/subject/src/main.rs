//! A running application an agent is asked about, rather than asked to write.
//!
//! Both existing tasks hand an agent a specification and score the program it
//! produces. Neither ever puts a running interface in front of it, so neither
//! can ask the question this project is built on: given an application it did
//! not write, does an agent inspect it or read its source?
//!
//! This program makes reading the source useless on purpose. The list it holds
//! is built from a seed passed in the environment, the seed is never given to
//! the agent, and the source is identical on every run. The number of items,
//! their labels and which are already done all change; the code does not. An
//! answer can only come from asking the program.
//!
//! Two ways to ask, one per benchmark arm:
//!
//! ```text
//! subject --serve      the JSON Lines agent protocol on stdin/stdout
//! subject --mcp        the same, wrapped as an MCP server
//! ```
//!
//! And one the harness uses and the agent is not told about:
//!
//! ```text
//! subject --truth      print the answer, for the checker
//! ```

use dewey::prelude::*;
use dewey::widget::Checkbox;

/// One row of the list.
struct Item {
    title: String,
    done: bool,
}

struct Board {
    items: Vec<Item>,
}

impl Board {
    /// The list this run holds, from a seed the agent never sees.
    ///
    /// Deliberately plain arithmetic rather than a dependency: the shape only
    /// has to vary, and a `rand` crate here would be one more thing to build
    /// before a paid run can start.
    fn from_seed(seed: u64) -> Self {
        let mut state = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        let mut next = || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 33) as u32
        };

        let count = 5 + (next() % 8) as usize; // 5..=12
        let subjects = [
            "renew the domain",
            "pay the invoice",
            "book the flight",
            "call the plumber",
            "file the tax return",
            "back up the laptop",
            "replace the smoke alarm",
            "return the library book",
        ];
        let items = (0..count)
            .map(|i| {
                let urgent = next() % 2 == 0;
                let done = next() % 3 == 0;
                let subject = subjects[(next() as usize) % subjects.len()];
                Item {
                    title: format!(
                        "{}: {subject} ({i})",
                        if urgent { "urgent" } else { "later" }
                    ),
                    done,
                }
            })
            .collect();
        Self { items }
    }

    /// What the task asks for: urgent items still outstanding.
    ///
    /// The checker uses this. The agent has to work it out by looking.
    fn answer(&self) -> usize {
        self.items
            .iter()
            .filter(|i| i.title.starts_with("urgent") && !i.done)
            .count()
    }
}

enum Msg {
    Toggle(usize),
}

impl Model for Board {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) -> Command<Msg> {
        match msg {
            Msg::Toggle(i) => {
                if let Some(item) = self.items.get_mut(i) {
                    item.done = !item.done;
                }
            }
        }
        Command::None
    }

    fn view(&self, frame: &mut Frame<'_>) {
        let mut y = frame.area.y;
        Label::new(format!("{} items", self.items.len()))
            .agent_id("count")
            .render(Rect::new(frame.area.x, y, frame.area.width, 24.0), frame);
        y += 24.0;

        for (i, item) in self.items.iter().enumerate() {
            let row = Rect::new(frame.area.x, y, frame.area.width, 24.0);
            // The checkbox carries the done flag and the label carries the
            // title, so both facts are in the tree an agent reads.
            Checkbox::new("", item.done)
                .action(
                    Box::leak(format!("toggle_{i}").into_boxed_str()) as &'static str,
                    Msg::Toggle(i),
                )
                .render(Rect::new(row.x, row.y, 24.0, row.height), frame);
            Label::new(&item.title)
                .agent_id(Box::leak(format!("item_{i}").into_boxed_str()) as &'static str)
                .render(
                    Rect::new(row.x + 28.0, row.y, row.width - 28.0, row.height),
                    frame,
                );
            y += 24.0;
        }
    }

    fn title(&self) -> &str {
        "Subject"
    }
}

fn main() -> std::io::Result<()> {
    let seed: u64 = std::env::var("DEWEY_SUBJECT_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let board = Board::from_seed(seed);
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Every flag named in the header, and nothing else. This used to test the
    // two it cared about and serve on anything left over, so `--serve` was
    // documented and never parsed — `--nonsense` started the server just the
    // same. That is the defect this whole benchmark exists to look for, in the
    // program the benchmark points at, so it is worth not shipping.
    let mode = match args.as_slice() {
        [] => "--serve",
        [one] if one == "--serve" || one == "--mcp" || one == "--truth" => one.as_str(),
        _ => {
            eprintln!("subject: expected --serve, --mcp or --truth; got {args:?}");
            std::process::exit(2);
        }
    };

    match mode {
        "--truth" => {
            println!("{}", board.answer());
            Ok(())
        }
        "--mcp" => dewey::agent::mcp::McpServer::new(board, 320.0, 400.0).run(),
        _ => {
            let mut driver = dewey::agent::driver::HeadlessDriver::new(board, 320.0, 400.0);
            driver.init();
            dewey::agent::rpc::serve_stdio(&mut driver)
        }
    }
}
