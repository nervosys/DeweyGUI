//! Ontology Explorer — an agent that discovers and inspects the Dewey framework.
//!
//! This example demonstrates the ontology subsystem by:
//! 1. Building a rich app model with many widgets
//! 2. Running it headlessly via `HeadlessDriver`
//! 3. Querying the ontology registry: list types, search, filter by role
//! 4. Inspecting individual widget schemas, capabilities, and actions
//! 5. Exporting the full catalog as JSON
//!
//! Run with: `cargo run --example ontology_explorer`

use dewey::agent::driver::HeadlessDriver;
use dewey::agent::protocol::*;
use dewey::ontology::SemanticRole;
use dewey::prelude::*;
use std::cell::RefCell;

// ── App model with many widget types ────────────────────────────────

#[allow(dead_code)]
struct OntologyApp {
    counter: i32,
    checkbox_on: bool,
    slider_state: RefCell<dewey::widget::slider::SliderState>,
    select_state: RefCell<dewey::widget::select::SelectState>,
    table_state: RefCell<dewey::widget::table::TableState>,
}

impl OntologyApp {
    fn new() -> Self {
        Self {
            counter: 0,
            checkbox_on: true,
            slider_state: RefCell::new(dewey::widget::slider::SliderState::new(42.0)),
            select_state: RefCell::new(dewey::widget::select::SelectState::new()),
            table_state: RefCell::new(dewey::widget::table::TableState::new()),
        }
    }
}

#[derive(Debug)]
enum Msg {
    Increment,
    ToggleCheck,
}

impl Model for OntologyApp {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) -> Command<Msg> {
        match msg {
            Msg::Increment => self.counter += 1,
            Msg::ToggleCheck => self.checkbox_on = !self.checkbox_on,
        }
        Command::None
    }

    fn view(&self, frame: &mut Frame<'_>) {
        let area = frame.area;

        let rows = Layout::new(
            Direction::Vertical,
            [
                Constraint::Length(30.0),
                Constraint::Length(30.0),
                Constraint::Length(30.0),
                Constraint::Length(30.0),
                Constraint::Length(30.0),
                Constraint::Length(30.0),
                Constraint::Length(30.0),
                Constraint::Length(80.0),
                Constraint::Length(80.0),
                Constraint::Fill(1.0),
            ],
        )
        .split(area);

        // Register a variety of widgets with agent IDs for ontology inspection
        Label::new(format!("Counter: {}", self.counter))
            .agent_id("info_label")
            .render(rows[0], frame);

        Button::new("Increment")
            .agent_id("inc_btn")
            .render(rows[1], frame);

        Checkbox::new("Enable notifications", self.checkbox_on)
            .agent_id("notif_checkbox")
            .render(rows[2], frame);

        Radio::new("Priority: High", true)
            .agent_id("priority_radio")
            .render(rows[3], frame);

        ProgressBar::new(0.73)
            .agent_id("upload_progress")
            .render(rows[4], frame);

        Tooltip::new("Help", "Click for context-sensitive help")
            .agent_id("help_tooltip")
            .render(rows[5], frame);

        Slider::new(0.0, 100.0)
            .step(0.5)
            .label("Volume")
            .agent_id("volume_slider")
            .render(rows[6], frame, &mut self.slider_state.borrow_mut());

        // Table
        Table::new(
            vec!["Name".into(), "Value".into(), "Status".into()],
            vec![
                vec!["Alpha".into(), "42".into(), "Active".into()],
                vec!["Beta".into(), "17".into(), "Pending".into()],
                vec!["Gamma".into(), "99".into(), "Done".into()],
            ],
        )
        .agent_id("data_table")
        .render(rows[7], frame, &mut self.table_state.borrow_mut());

        // Tree
        Tree::new(TreeNode::branch(
            "System",
            vec![
                TreeNode::branch(
                    "Services",
                    vec![TreeNode::leaf("HTTP Server"), TreeNode::leaf("Database")],
                ),
                TreeNode::leaf("Configuration"),
            ],
        ))
        .agent_id("system_tree")
        .render(rows[8], frame);

        // Canvas
        Canvas::new()
            .agent_id("drawing_canvas")
            .background([20, 20, 40, 255])
            .filled_rect(10.0, 10.0, 60.0, 60.0, [100, 150, 255, 255])
            .filled_circle(50.0, 50.0, 20.0, [255, 200, 50, 255])
            .render(rows[9], frame);
    }

    fn handle_event(&self, event: Event) -> Option<Msg> {
        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Char('+'),
                ..
            }) => Some(Msg::Increment),
            Event::Key(KeyEvent {
                code: KeyCode::Char('c'),
                ..
            }) => Some(Msg::ToggleCheck),
            _ => None,
        }
    }

    fn register_ontology(&self, registry: &mut OntologyRegistry) {
        registry.register_schema(WidgetSchema::new(
            "OntologyExplorerApp",
            "Demo app showcasing ontology discovery",
            SemanticRole::Container,
        ));

        // Register all widget types used in this app for full catalog
        registry.register(&Button::new("_"));
        registry.register(&Label::new("_"));
        registry.register(&Checkbox::new("_", false));
        registry.register(&Radio::new("_", false));
        registry.register(&ProgressBar::new(0.0));
        registry.register(&Tooltip::new("_", "_"));
        registry.register(&Slider::new(0.0, 1.0));
        registry.register(&Table::new(vec![], vec![]));
        registry.register(&Tree::new(TreeNode::leaf("_")));
        registry.register(&Canvas::new());
    }

    fn title(&self) -> &str {
        "Ontology Explorer"
    }
}

// ── Agent simulation ────────────────────────────────────────────────

fn main() {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║         Dewey Ontology Explorer — Agent Demo            ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();

    // 1. Create a headless app
    let mut driver = HeadlessDriver::new(OntologyApp::new(), 800.0, 600.0);
    driver.init();
    driver.tick();

    println!("✓ HeadlessDriver initialized (800×600 virtual window)");
    println!();

    // ── Step 1: Negotiate protocol ──────────────────────────────────
    println!("── Step 1: Protocol Negotiation ──────────────────────────");
    let resp = driver.process_request(&AgentRequest::Negotiate {
        client_version: 2,
        capabilities: vec!["batch_actions".into(), "state_diffs".into()],
    });
    if let Some(data) = &resp.data {
        println!("  Protocol version: {}", data["protocol_version"]);
        println!("  Server caps:      {}", data["capabilities"]);
    }
    println!();

    // ── Step 2: Ping ────────────────────────────────────────────────
    println!("── Step 2: Ping ────────────────────────────────────────");
    let resp = driver.process_request(&AgentRequest::Ping);
    if let Some(data) = &resp.data {
        println!("  Framework: {}", data["framework"]);
        println!("  Status:    {}", data["status"]);
    }
    println!();

    // ── Step 3: Query full ontology ─────────────────────────────────
    println!("── Step 3: Query Ontology (all types) ────────────────────");
    let resp = driver.process_request(&AgentRequest::QueryOntology {
        query: None,
        role: None,
    });
    if let Some(data) = &resp.data {
        if let Some(arr) = data.as_array() {
            println!("  Discovered {} widget types:", arr.len());
            for item in arr {
                let name = item["name"].as_str().unwrap_or("?");
                let role = item["default_role"].as_str().unwrap_or("?");
                let desc = item["description"].as_str().unwrap_or("");
                println!("    • {:<25} [{}] {}", name, role, desc);
            }
        }
    }
    println!();

    // ── Step 4: Search by keyword ───────────────────────────────────
    println!("── Step 4: Search by keyword \"click\" ─────────────────────");
    let resp = driver.process_request(&AgentRequest::QueryOntology {
        query: Some("click".into()),
        role: None,
    });
    if let Some(data) = &resp.data {
        if let Some(arr) = data.as_array() {
            println!("  Found {} types matching \"click\":", arr.len());
            for item in arr {
                println!("    • {}", item["name"].as_str().unwrap_or("?"));
            }
        }
    }
    println!();

    // ── Step 5: Filter by role ──────────────────────────────────────
    println!("── Step 5: Filter by role \"input\" ────────────────────────");
    let resp = driver.process_request(&AgentRequest::QueryOntology {
        query: None,
        role: Some("input".into()),
    });
    if let Some(data) = &resp.data {
        if let Some(arr) = data.as_array() {
            println!("  Found {} Input widgets:", arr.len());
            for item in arr {
                let name = item["name"].as_str().unwrap_or("?");
                let hint = item["usage_hint"].as_str().unwrap_or("(no hint)");
                println!("    • {:<15} hint: {}", name, hint);
            }
        }
    }
    println!();

    // ── Step 6: Get specific schema ─────────────────────────────────
    println!("── Step 6: Inspect \"Button\" schema ──────────────────────");
    let resp = driver.process_request(&AgentRequest::GetSchema {
        widget_type: "Button".into(),
    });
    if let Some(data) = &resp.data {
        println!("  Schema: {}", serde_json::to_string_pretty(data).unwrap());
    }
    println!();

    // ── Step 7: Inspect a complex widget ────────────────────────────
    println!("── Step 7: Inspect \"Table\" schema ───────────────────────");
    let resp = driver.process_request(&AgentRequest::GetSchema {
        widget_type: "Table".into(),
    });
    if let Some(data) = &resp.data {
        if let Some(actions) = data["actions"].as_array() {
            println!("  Table actions ({}):", actions.len());
            for action in actions {
                let name = action["name"].as_str().unwrap_or("?");
                let desc = action["description"].as_str().unwrap_or("");
                let mutates = action["mutates"].as_bool().unwrap_or(false);
                println!(
                    "    • {:<20} {} {}",
                    name,
                    desc,
                    if mutates { "[mutates]" } else { "" }
                );
            }
        }
        if let Some(tags) = data["tags"].as_array() {
            let tag_strs: Vec<&str> = tags.iter().filter_map(|t| t.as_str()).collect();
            println!("  Tags: {}", tag_strs.join(", "));
        }
    }
    println!();

    // ── Step 8: Get UI tree ─────────────────────────────────────────
    println!("── Step 8: Get UI Tree ─────────────────────────────────");
    let resp = driver.process_request(&AgentRequest::GetTree { since: None });
    if let Some(data) = &resp.data {
        fn print_tree(node: &serde_json::Value, depth: usize) {
            let indent = "  ".repeat(depth + 1);
            let wtype = node["widget_type"].as_str().unwrap_or("?");
            let id = node["agent_id"].as_str().unwrap_or("(none)");
            let role = node["role"].as_str().unwrap_or("?");
            println!("{indent}├─ {wtype} id={id} role={role}");
            if let Some(children) = node["children"].as_array() {
                for child in children {
                    print_tree(child, depth + 1);
                }
            }
        }
        if let Some(root) = data.get("root") {
            println!("  Widget tree:");
            print_tree(root, 0);
        } else {
            println!("  (No UI tree available — widgets register during render)");
        }
    }
    println!();

    // ── Step 9: Direct registry inspection ──────────────────────────
    println!("── Step 9: Direct ontology registry queries ──────────────");
    let ontology = driver.ontology();

    // List all registered types
    let types = ontology.list_types();
    println!("  Registered types ({}): {}", types.len(), types.join(", "));

    // Find by role: Action
    let action_widgets = ontology.find_by_role(SemanticRole::Action);
    println!(
        "  Action role widgets: {:?}",
        action_widgets
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>()
    );

    // Find by role: Input
    let input_widgets = ontology.find_by_role(SemanticRole::Input);
    println!(
        "  Input role widgets:  {:?}",
        input_widgets
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>()
    );

    // Search by tag
    let range_widgets = ontology.search("range");
    println!(
        "  Widgets matching \"range\": {:?}",
        range_widgets
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>()
    );

    // Validate action params
    let valid = ontology.validate_action_params("Button", "click", &serde_json::json!({}));
    println!(
        "  Validate Button.click({{}}): {}",
        if valid.is_ok() {
            "✓ valid"
        } else {
            "✗ invalid"
        }
    );

    println!();

    // ── Step 10: Export full catalog ─────────────────────────────────
    println!("── Step 10: Export full catalog ───────────────────────────");
    let catalog = ontology.export_catalog();
    let catalog_str = serde_json::to_string_pretty(&catalog).unwrap();
    let lines: Vec<&str> = catalog_str.lines().collect();
    println!("  Full catalog: {} lines of JSON", lines.len());
    // Show first 10 lines as preview
    for line in lines.iter().take(10) {
        println!("  {line}");
    }
    if lines.len() > 10 {
        println!("  ... ({} more lines)", lines.len() - 10);
    }
    println!();

    // ── Summary ─────────────────────────────────────────────────────
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║  Ontology exploration complete!                         ║");
    println!("║                                                         ║");
    println!("║  An AI agent can:                                       ║");
    println!("║    • Negotiate protocol version and capabilities        ║");
    println!("║    • Discover all widget types via QueryOntology        ║");
    println!("║    • Search by keyword or filter by semantic role       ║");
    println!("║    • Inspect schemas, actions, properties, and tags     ║");
    println!("║    • Navigate the live UI tree                          ║");
    println!("║    • Validate action parameters before execution        ║");
    println!("║    • Export the full catalog as structured JSON          ║");
    println!("╚══════════════════════════════════════════════════════════╝");
}
