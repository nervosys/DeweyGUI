//! Agent Headless — programmatic agent that controls widgets via the protocol.
//!
//! This example simulates a full AI agent session:
//! 1. Connects to the app via HeadlessDriver
//! 2. Negotiates protocol capabilities
//! 3. Discovers the UI through ontology queries
//! 4. Performs actions on widgets (click, toggle, inject keys)
//! 5. Reads state changes and verifies results
//! 6. Executes batch actions atomically
//! 7. Subscribes to state-change events
//!
//! Run with: `cargo run --example agent_headless`

use dewey::agent::driver::HeadlessDriver;
use dewey::agent::protocol::*;
use dewey::ontology::SemanticRole;
use dewey::prelude::*;

// ── App model ───────────────────────────────────────────────────────

struct TodoApp {
    items: Vec<TodoItem>,
    /// The next id to hand out. Ids are never reused, so a name an agent read
    /// once never comes to mean something else.
    next_id: u32,
    filter: Filter,
}

struct TodoItem {
    /// Names this item for as long as it exists.
    ///
    /// Not its position: an agent that reads `todo_2`, then switches the
    /// filter or completes an earlier item, would find the same name meaning
    /// a different todo — and would be told its action succeeded. `validate`
    /// reports index-shaped ids as `positional_id` for exactly this reason.
    id: u32,
    text: String,
    done: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Filter {
    All,
    Active,
    Done,
}

impl TodoApp {
    fn new() -> Self {
        Self {
            items: vec![
                TodoItem {
                    id: 1,
                    text: "Write documentation".into(),
                    done: false,
                },
                TodoItem {
                    id: 2,
                    text: "Add tests".into(),
                    done: true,
                },
                TodoItem {
                    id: 3,
                    text: "Release v1.0".into(),
                    done: false,
                },
            ],
            next_id: 4,
            filter: Filter::All,
        }
    }

    fn visible_items(&self) -> Vec<&TodoItem> {
        self.items
            .iter()
            .filter(|i| match self.filter {
                Filter::All => true,
                Filter::Active => !i.done,
                Filter::Done => i.done,
            })
            .collect()
    }

    fn active_count(&self) -> usize {
        self.items.iter().filter(|i| !i.done).count()
    }

    fn done_count(&self) -> usize {
        self.items.iter().filter(|i| i.done).count()
    }
}

#[derive(Debug)]
#[allow(dead_code)]
enum Msg {
    /// Names the item, not its place in the list.
    ToggleItem(u32),
    AddItem(String),
    SetFilter(Filter),
    ClearDone,
}

impl Model for TodoApp {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) -> Command<Msg> {
        match msg {
            Msg::ToggleItem(id) => {
                if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
                    item.done = !item.done;
                }
            }
            Msg::AddItem(text) => {
                self.items.push(TodoItem {
                    id: self.next_id,
                    text,
                    done: false,
                });
                self.next_id += 1;
            }
            Msg::SetFilter(f) => self.filter = f,
            Msg::ClearDone => self.items.retain(|i| !i.done),
        }
        Command::None
    }

    fn view(&self, frame: &mut Frame<'_>) {
        let area = frame.area;

        let rows = Layout::new(
            Direction::Vertical,
            [
                Constraint::Length(30.0), // title
                Constraint::Length(25.0), // stats label
                Constraint::Length(25.0), // filter buttons
                Constraint::Fill(1.0),    // todo list
                Constraint::Length(25.0), // clear button
            ],
        )
        .split(area);

        // Title
        Label::new("Todo List")
            .agent_id("title")
            .render(rows[0], frame);

        // Stats
        Label::new(format!(
            "{} active, {} done, {} total",
            self.active_count(),
            self.done_count(),
            self.items.len()
        ))
        .agent_id("stats_label")
        .render(rows[1], frame);

        // Filter buttons
        let filter_cols = Layout::new(
            Direction::Horizontal,
            [
                Constraint::Percentage(33.3),
                Constraint::Percentage(33.3),
                Constraint::Percentage(33.3),
            ],
        )
        .split(rows[2]);

        Button::new("All")
            .agent_id("filter_all")
            .render(filter_cols[0], frame);
        Button::new("Active")
            .agent_id("filter_active")
            .render(filter_cols[1], frame);
        Button::new("Done")
            .agent_id("filter_done")
            .render(filter_cols[2], frame);

        // Todo items as checkboxes
        let visible = self.visible_items();
        let item_constraints: Vec<Constraint> = visible
            .iter()
            .map(|_| Constraint::Length(25.0))
            .chain(std::iter::once(Constraint::Fill(1.0)))
            .collect();
        let item_rows = Layout::new(Direction::Vertical, item_constraints).split(rows[3]);

        for (i, item) in visible.iter().enumerate() {
            // `on` rather than `agent_id`: the id alone makes the checkbox
            // visible to an agent, and toggling it would have done nothing.
            let id = item.id;
            Checkbox::new(&item.text, item.done)
                .on(format!("todo_{id}"), move |app: &mut TodoApp| {
                    if let Some(item) = app.items.iter_mut().find(|i| i.id == id) {
                        item.done = !item.done;
                    }
                })
                .render(item_rows[i], frame);
        }

        // Clear done button
        Button::new(format!("Clear Done ({})", self.done_count()))
            .agent_id("clear_done")
            .enabled(self.done_count() > 0)
            .render(rows[4], frame);
    }

    fn handle_event(&self, event: Event) -> Option<Msg> {
        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Char('a'),
                ..
            }) => Some(Msg::SetFilter(Filter::All)),
            Event::Key(KeyEvent {
                code: KeyCode::Char('f'),
                ..
            }) => Some(Msg::SetFilter(Filter::Active)),
            Event::Key(KeyEvent {
                code: KeyCode::Char('d'),
                ..
            }) => Some(Msg::SetFilter(Filter::Done)),
            _ => None,
        }
    }

    fn register_ontology(&self, registry: &mut OntologyRegistry) {
        registry.register_schema(WidgetSchema::new(
            "TodoApp",
            "A todo list application",
            SemanticRole::Container,
        ));
        // Register component widget types
        registry.register(&Button::new("_"));
        registry.register(&Label::new("_"));
        registry.register(&Checkbox::new("_", false));
    }

    fn title(&self) -> &str {
        "Todo App"
    }
}

// ── Agent simulation ────────────────────────────────────────────────

fn main() {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║      Dewey Agent Headless — Programmatic Control        ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    let mut driver = HeadlessDriver::new(TodoApp::new(), 800.0, 600.0);
    driver.init();
    driver.tick();

    // ── Phase 1: Discovery ──────────────────────────────────────────
    println!("═══ Phase 1: Discovery ══════════════════════════════════");
    println!();

    // Negotiate
    let resp = driver.process_request(&AgentRequest::Negotiate {
        client_version: 2,
        capabilities: vec!["batch_actions".into(), "state_diffs".into()],
    });
    assert!(resp.success, "Negotiate failed");
    println!(
        "  ✓ Protocol negotiated (v{})",
        resp.data.as_ref().unwrap()["protocol_version"]
    );

    // Discover what widget types are available
    let resp = driver.process_request(&AgentRequest::QueryOntology {
        query: None,
        role: None,
    });
    assert!(resp.success);
    let types = resp.data.as_ref().unwrap().as_array().unwrap();
    println!("  ✓ Discovered {} widget types", types.len());
    for t in types {
        println!(
            "    • {} ({})",
            t["name"].as_str().unwrap_or("?"),
            t["default_role"].as_str().unwrap_or("?")
        );
    }
    println!();

    // Find all actionable widgets
    let resp = driver.process_request(&AgentRequest::QueryOntology {
        query: None,
        role: Some("action".into()),
    });
    let action_types = resp.data.as_ref().unwrap().as_array().unwrap();
    println!("  ✓ Found {} action-role widget types", action_types.len());

    // Find all input widgets
    let resp = driver.process_request(&AgentRequest::QueryOntology {
        query: None,
        role: Some("input".into()),
    });
    let input_types = resp.data.as_ref().unwrap().as_array().unwrap();
    println!("  ✓ Found {} input-role widget types", input_types.len());
    println!();

    // ── Phase 2: Read initial state ─────────────────────────────────
    println!("═══ Phase 2: Read Initial State ═════════════════════════");
    println!();

    // Read the stats label
    let resp = driver.process_request(&AgentRequest::GetState {
        agent_id: "stats_label".into(),
    });
    if resp.success {
        if let Some(data) = &resp.data {
            println!("  stats_label: {}", data);
        }
    } else {
        println!("  stats_label: (widget state not available via GetState — using model)");
    }
    println!(
        "  Model state: {} items, {} active, {} done",
        driver.model().items.len(),
        driver.model().active_count(),
        driver.model().done_count(),
    );
    for (i, item) in driver.model().items.iter().enumerate() {
        println!(
            "    [{}] {} — {}",
            i,
            if item.done { "✓" } else { "○" },
            item.text
        );
    }
    println!();

    // ── Phase 3: Perform actions ────────────────────────────────────
    println!("═══ Phase 3: Perform Actions ════════════════════════════");
    println!();

    // Action: Toggle the first item (index 0: "Write documentation" — currently not done)
    println!("  → Injecting key 'a' to set filter to All...");
    let resp = driver.process_request(&AgentRequest::InjectEvent {
        event: InjectedEvent::Key {
            code: "a".into(),
            modifiers: vec![],
        },
    });
    assert!(resp.success);
    assert_eq!(driver.model().filter, Filter::All);
    println!("    ✓ Filter set to All");

    // Inject key 'f' to switch to Active filter
    println!("  → Injecting key 'f' to filter Active items...");
    let resp = driver.process_request(&AgentRequest::InjectEvent {
        event: InjectedEvent::Key {
            code: "f".into(),
            modifiers: vec![],
        },
    });
    assert!(resp.success);
    assert_eq!(driver.model().filter, Filter::Active);
    println!("    ✓ Filter set to Active");
    println!("    Active items: {}", driver.model().visible_items().len());

    // Switch back to All
    driver.process_request(&AgentRequest::InjectEvent {
        event: InjectedEvent::Key {
            code: "a".into(),
            modifiers: vec![],
        },
    });

    // Execute action on a button widget
    println!("  → ExecuteAction on 'clear_done' button...");
    let resp = driver.process_request(&AgentRequest::ExecuteAction {
        agent_id: "clear_done".into(),
        action: "click".into(),
        params: serde_json::json!({}),
    });
    println!("    Response: success={}", resp.success);
    println!();

    // ── Phase 4: Batch actions ──────────────────────────────────────
    println!("═══ Phase 4: Batch Actions ══════════════════════════════");
    println!();

    println!("  → Executing batch: click filter_all + click filter_done...");
    let resp = driver.process_request(&AgentRequest::BatchActions {
        actions: vec![
            BatchActionEntry {
                agent_id: "filter_all".into(),
                action: "click".into(),
                params: serde_json::json!({}),
            },
            BatchActionEntry {
                agent_id: "filter_done".into(),
                action: "click".into(),
                params: serde_json::json!({}),
            },
        ],
    });
    assert!(resp.success);
    if let Some(data) = &resp.data {
        println!("    ✓ Batch result: {}", data);
    }
    println!();

    // ── Phase 5: Subscribe to events ────────────────────────────────
    println!("═══ Phase 5: Event Subscription ═════════════════════════");
    println!();

    println!("  → Subscribing to state_changed events...");
    let resp = driver.process_request(&AgentRequest::Subscribe {
        events: vec!["state_changed".into()],
    });
    assert!(resp.success);
    println!("    ✓ Subscribed");

    // Trigger a state change
    println!("  → Injecting '+' key... (should cause no change — app doesn't handle '+')");
    driver.process_request(&AgentRequest::InjectEvent {
        event: InjectedEvent::Key {
            code: "d".into(),
            modifiers: vec![],
        },
    });
    println!("    Filter is now: {:?}", driver.model().filter);

    // Compute diffs
    let diffs = driver.compute_state_diffs();
    println!("    State diffs detected: {}", diffs.len());
    for diff in &diffs {
        if let AgentEvent::StateChanged { agent_id, state } = diff {
            println!("      Δ {agent_id}: {state}");
        }
    }

    // Unsubscribe
    let resp = driver.process_request(&AgentRequest::Unsubscribe {
        events: vec!["state_changed".into()],
    });
    assert!(resp.success);
    println!("    ✓ Unsubscribed");
    println!();

    // ── Phase 6: Screenshot (JSON tree) ─────────────────────────────
    println!("═══ Phase 6: Screenshot ═════════════════════════════════");
    println!();

    let resp = driver.process_request(&AgentRequest::Screenshot {
        format: "json".into(),
    });
    assert!(resp.success);
    if let Some(data) = &resp.data {
        println!("  Screenshot kind: {}", data["kind"]);
        let json_str = serde_json::to_string_pretty(data).unwrap();
        let lines: Vec<&str> = json_str.lines().collect();
        println!("  Data: {} lines of JSON", lines.len());
        for line in lines.iter().take(8) {
            println!("  {line}");
        }
        if lines.len() > 8 {
            println!("  ... ({} more)", lines.len() - 8);
        }
    }
    println!();

    // ── Phase 7: Widget-level ontology inspection ───────────────────
    println!("═══ Phase 7: Widget Ontology Inspection ═════════════════");
    println!();

    // Inspect Button schema actions
    let resp = driver.process_request(&AgentRequest::GetSchema {
        widget_type: "Button".into(),
    });
    if let Some(data) = &resp.data {
        println!("  Button:");
        println!("    Role: {}", data["default_role"]);
        if let Some(actions) = data["actions"].as_array() {
            for a in actions {
                println!("    Action: {} — {}", a["name"], a["description"]);
            }
        }
        if let Some(hint) = data["usage_hint"].as_str() {
            println!("    Usage: {hint}");
        }
    }

    // Inspect Checkbox schema
    let resp = driver.process_request(&AgentRequest::GetSchema {
        widget_type: "Checkbox".into(),
    });
    if let Some(data) = &resp.data {
        println!("  Checkbox:");
        println!("    Role: {}", data["default_role"]);
        if let Some(actions) = data["actions"].as_array() {
            for a in actions {
                println!(
                    "    Action: {} — {} (mutates: {})",
                    a["name"], a["description"], a["mutates"]
                );
            }
        }
    }
    println!();

    // ── Phase 8: Final state ────────────────────────────────────────
    println!("═══ Phase 8: Final State ════════════════════════════════");
    println!();
    println!("  Filter: {:?}", driver.model().filter);
    println!("  Items ({}):", driver.model().items.len());
    for (i, item) in driver.model().items.iter().enumerate() {
        println!(
            "    [{}] {} — {}",
            i,
            if item.done { "✓" } else { "○" },
            item.text
        );
    }
    println!("  App running: {}", driver.is_running());
    println!();

    // ── Quit ────────────────────────────────────────────────────────
    println!("  → Sending Quit...");
    let resp = driver.process_request(&AgentRequest::Quit);
    assert!(resp.success);
    assert!(!driver.is_running());
    println!("    ✓ Application terminated gracefully");
    println!();

    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║  Agent session complete!                                 ║");
    println!("║                                                         ║");
    println!("║  Demonstrated:                                          ║");
    println!("║    • Protocol negotiation                               ║");
    println!("║    • Ontology discovery (types, roles, schemas)          ║");
    println!("║    • Key injection for navigation                       ║");
    println!("║    • Widget action execution                            ║");
    println!("║    • Batch actions                                      ║");
    println!("║    • Event subscription + state diffs                   ║");
    println!("║    • JSON screenshot                                    ║");
    println!("║    • Graceful shutdown                                   ║");
    println!("╚══════════════════════════════════════════════════════════╝");
}
