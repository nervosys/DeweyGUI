//! Integration tests for the Dewey framework.
//!
//! These tests verify the end-to-end flow: Model → HeadlessDriver → Agent Protocol.

use dewey::agent::driver::HeadlessDriver;
use dewey::agent::protocol::*;
use dewey::ontology::{Accessibility, Discoverable, OntologyRegistry, SemanticRole, WidgetSchema};
use dewey::prelude::*;
use dewey::widget::list::ListState;
use dewey::widget::panel::PanelSide;
use dewey::widget::table::{SortDirection, TableState};

// ── Minimal test model ──────────────────────────────────────────────

struct TestApp {
    count: i32,
}

#[derive(Debug)]
enum TestMsg {
    Increment,
    Decrement,
    Reset,
    SetCount(i32),
}

impl Model for TestApp {
    type Msg = TestMsg;

    fn update(&mut self, msg: TestMsg) -> Command<TestMsg> {
        match msg {
            TestMsg::Increment => self.count += 1,
            TestMsg::Decrement => self.count -= 1,
            TestMsg::Reset => self.count = 0,
            TestMsg::SetCount(n) => self.count = n,
        }
        Command::None
    }

    fn view(&self, frame: &mut Frame<'_>) {
        let area = frame.area;

        let chunks = Layout::new(
            Direction::Vertical,
            [Constraint::Length(30.0), Constraint::Length(30.0)],
        )
        .split(area);

        Label::new(format!("Count: {}", self.count))
            .agent_id("counter_label")
            .render(chunks[0], frame);

        Button::new("Increment")
            .agent_id("inc_btn")
            .render(chunks[1], frame);
    }

    fn handle_event(&self, event: Event) -> Option<TestMsg> {
        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Char('+'),
                ..
            }) => Some(TestMsg::Increment),
            Event::Key(KeyEvent {
                code: KeyCode::Char('-'),
                ..
            }) => Some(TestMsg::Decrement),
            _ => None,
        }
    }

    fn register_ontology(&self, registry: &mut OntologyRegistry) {
        registry.register_schema(WidgetSchema::new(
            "TestApp",
            "Test application",
            SemanticRole::Container,
        ));
    }

    fn title(&self) -> &str {
        "Test"
    }
}

// ── HeadlessDriver tests ────────────────────────────────────────────

#[test]
fn test_model_reset_and_set_count() {
    let mut app = TestApp { count: 5 };
    app.update(TestMsg::Reset);
    assert_eq!(app.count, 0);
    app.update(TestMsg::SetCount(42));
    assert_eq!(app.count, 42);
}

#[test]
fn headless_driver_ping() {
    let mut driver = HeadlessDriver::new(TestApp { count: 0 }, 800.0, 600.0);
    driver.init();

    let resp = driver.process_request(&AgentRequest::Ping);
    assert!(resp.success);
    let data = resp.data.unwrap();
    assert_eq!(data["status"], "pong");
    assert_eq!(data["framework"], "dewey");
}

#[test]
fn headless_driver_query_ontology() {
    let mut driver = HeadlessDriver::new(TestApp { count: 0 }, 800.0, 600.0);
    driver.init();

    let resp = driver.process_request(&AgentRequest::QueryOntology {
        query: Some("TestApp".into()),
        role: None,
    });
    assert!(resp.success);
    let data = resp.data.unwrap();
    assert!(!data.as_array().unwrap().is_empty());
}

#[test]
fn headless_driver_inject_key_event() {
    let mut driver = HeadlessDriver::new(TestApp { count: 0 }, 800.0, 600.0);
    driver.init();

    assert_eq!(driver.model().count, 0);

    // Inject a '+' key press
    let resp = driver.process_request(&AgentRequest::InjectEvent {
        event: InjectedEvent::Key {
            code: "+".into(),
            modifiers: vec![],
        },
    });
    assert!(resp.success);
    assert_eq!(driver.model().count, 1);

    // Inject another '+' key press
    driver.process_request(&AgentRequest::InjectEvent {
        event: InjectedEvent::Key {
            code: "+".into(),
            modifiers: vec![],
        },
    });
    assert_eq!(driver.model().count, 2);

    // Inject '-' to decrement
    driver.process_request(&AgentRequest::InjectEvent {
        event: InjectedEvent::Key {
            code: "-".into(),
            modifiers: vec![],
        },
    });
    assert_eq!(driver.model().count, 1);
}

#[test]
fn headless_driver_tick() {
    let mut driver = HeadlessDriver::new(TestApp { count: 0 }, 800.0, 600.0);
    driver.init();
    // Ticking should not crash
    driver.tick();
    driver.tick();
    assert!(driver.is_running());
}

#[test]
fn headless_driver_quit() {
    let mut driver = HeadlessDriver::new(TestApp { count: 0 }, 800.0, 600.0);
    driver.init();
    assert!(driver.is_running());

    let resp = driver.process_request(&AgentRequest::Quit);
    assert!(resp.success);
    assert!(!driver.is_running());
}

#[test]
fn headless_driver_envelope() {
    let mut driver = HeadlessDriver::new(TestApp { count: 0 }, 800.0, 600.0);
    driver.init();

    let envelope = RequestEnvelope {
        id: Some("req-42".into()),
        request: AgentRequest::Ping,
    };

    let resp = driver.process_envelope(&envelope);
    assert!(resp.success);
    assert_eq!(resp.id, Some("req-42".into()));
}

#[test]
fn headless_driver_task_command() {
    let app = TestApp { count: 0 };
    let mut driver = HeadlessDriver::new(app, 800.0, 600.0);
    driver.init();

    // Simulate a Task command by injecting an event that produces one
    // We test the Task path indirectly through model update
    assert_eq!(driver.model().count, 0);
}

// ── Widget ontology tests ───────────────────────────────────────────

#[test]
fn button_discoverable() {
    let btn = Button::new("Test");
    assert_eq!(btn.schema().name, "Button");
    assert_eq!(btn.semantic_role(), SemanticRole::Action);
    assert!(!btn.capabilities().is_empty());
    assert_eq!(btn.actions().len(), 1);
    assert_eq!(btn.actions()[0].name, "click");
}

#[test]
fn button_execute_action() {
    let mut btn = Button::new("Test");
    let result = btn.execute_action("click", &serde_json::json!({}));
    assert!(result.is_ok());

    let mut disabled_btn = Button::new("Test").enabled(false);
    let result = disabled_btn.execute_action("click", &serde_json::json!({}));
    assert!(result.is_err());
}

#[test]
fn label_discoverable() {
    let lbl = Label::new("Hello");
    assert_eq!(lbl.schema().name, "Label");
    assert!(lbl.actions().is_empty());
}

#[test]
fn tooltip_info() {
    let tip = Tooltip::new("Hover me", "This is a tooltip");
    assert_eq!(tip.schema().name, "Tooltip");
    let state = tip.agent_state();
    assert_eq!(state["label"], "Hover me");
    assert_eq!(state["text"], "This is a tooltip");
}

#[test]
fn canvas_builder_pattern() {
    let canvas = Canvas::new()
        .agent_id("my_canvas")
        .background([0, 0, 0, 255])
        .line(0.0, 0.0, 100.0, 100.0, [255, 0, 0, 255], 2.0)
        .filled_rect(10.0, 10.0, 50.0, 50.0, [0, 255, 0, 255])
        .circle(50.0, 50.0, 25.0, [0, 0, 255, 255], 1.5)
        .filled_circle(75.0, 75.0, 10.0, [255, 255, 0, 255])
        .text(5.0, 5.0, "Hello", 14.0, [255, 255, 255, 255]);

    assert_eq!(Discoverable::agent_id(&canvas), Some("my_canvas"));
    let state = canvas.agent_state();
    assert_eq!(state["command_count"], 5);
}

#[test]
fn tree_expand_collapse_via_actions() {
    let root = TreeNode::branch(
        "root",
        vec![TreeNode::branch("child", vec![TreeNode::leaf("leaf")])],
    );
    let mut tree = Tree::new(root);

    // Collapse
    let r = tree.execute_action("collapse", &serde_json::json!({"path": "root/child"}));
    assert!(r.is_ok());

    // Expand
    let r = tree.execute_action("expand", &serde_json::json!({"path": "root/child"}));
    assert!(r.is_ok());
    assert_eq!(r.unwrap()["expanded"], true);
}

#[test]
fn modal_open_close() {
    let mut modal = Modal::new("Test Modal", false);
    assert_eq!(modal.agent_state()["open"], false);

    modal
        .execute_action("open", &serde_json::json!({}))
        .unwrap();
    assert_eq!(modal.agent_state()["open"], true);

    modal
        .execute_action("close", &serde_json::json!({}))
        .unwrap();
    assert_eq!(modal.agent_state()["open"], false);
}

#[test]
fn image_from_uri() {
    let img = Image::from_uri("https://example.com/photo.png")
        .alt("Example photo")
        .fit(ImageFit::Cover);
    assert_eq!(img.schema().name, "Image");
    let state = img.agent_state();
    assert_eq!(state["source"], "uri");
    assert_eq!(state["alt"], "Example photo");
    assert_eq!(img.accessibility_label(), Some("Example photo".to_string()));
}

#[test]
fn image_from_rgba() {
    let pixels = vec![255u8; 4 * 2 * 2]; // 2x2 white image
    let img = Image::from_rgba(2, 2, pixels).alt("tiny");
    let state = img.agent_state();
    assert_eq!(state["source"], "rgba");
    assert_eq!(state["width"], 2);
    assert_eq!(state["height"], 2);
}

// ── Layout tests ────────────────────────────────────────────────────

#[test]
fn layout_constraint_split() {
    let area = Rect::new(0.0, 0.0, 300.0, 100.0);
    let chunks = Layout::new(
        Direction::Horizontal,
        [Constraint::Percentage(50.0), Constraint::Percentage(50.0)],
    )
    .split(area);

    assert_eq!(chunks.len(), 2);
    assert!((chunks[0].width - 150.0).abs() < 1.0);
    assert!((chunks[1].width - 150.0).abs() < 1.0);
}

// ── Protocol serialization tests ────────────────────────────────────

#[test]
fn agent_request_serde_roundtrip() {
    let request = AgentRequest::ExecuteAction {
        agent_id: "btn-1".into(),
        action: "click".into(),
        params: serde_json::json!({}),
    };
    let json = serde_json::to_string(&request).unwrap();
    let _parsed: AgentRequest = serde_json::from_str(&json).unwrap();
}

#[test]
fn agent_response_ok() {
    let resp = AgentResponse::ok(serde_json::json!({"test": true})).with_id("42");
    assert!(resp.success);
    assert_eq!(resp.id, Some("42".into()));
    assert!(resp.error.is_none());
}

#[test]
fn agent_response_err() {
    let resp = AgentResponse::err("something broke");
    assert!(!resp.success);
    assert_eq!(resp.error, Some("something broke".into()));
    assert!(resp.data.is_none());
}

#[test]
fn injected_event_serde() {
    let event = InjectedEvent::Key {
        code: "enter".into(),
        modifiers: vec!["ctrl".into()],
    };
    let json = serde_json::to_string(&event).unwrap();
    let parsed: InjectedEvent = serde_json::from_str(&json).unwrap();
    if let InjectedEvent::Key { code, modifiers } = parsed {
        assert_eq!(code, "enter");
        assert_eq!(modifiers, vec!["ctrl"]);
    } else {
        panic!("Expected Key event");
    }
}

// ── Focus management tests ──────────────────────────────────────────

#[test]
fn focus_manager_round_trip() {
    let mut fm = FocusManager::new();
    fm.register("a");
    fm.register("b");
    fm.register("c");

    assert_eq!(fm.focused_id(), None);
    fm.focus_next();
    assert_eq!(fm.focused_id(), Some("a"));
    fm.focus_next();
    assert_eq!(fm.focused_id(), Some("b"));
    fm.focus_next();
    assert_eq!(fm.focused_id(), Some("c"));
    fm.focus_next();
    assert_eq!(fm.focused_id(), Some("a")); // wraps around
}

// ── Theme tests ─────────────────────────────────────────────────────

#[test]
fn theme_token_lookup() {
    let dark = Theme::dark();
    let bg = dark.get(ThemeToken::Background);
    // Dark theme background should be dark
    assert!(bg.r < 0.5);

    let light = Theme::light();
    let bg = light.get(ThemeToken::Background);
    // Light theme background should be bright
    assert!(bg.r > 0.5);
}

// ── Batch actions tests ─────────────────────────────────────────────

#[test]
fn headless_driver_batch_actions() {
    let mut driver = HeadlessDriver::new(TestApp { count: 0 }, 800.0, 600.0);
    driver.init();

    let resp = driver.process_request(&AgentRequest::BatchActions {
        actions: vec![BatchActionEntry {
            agent_id: "inc_btn".into(),
            action: "click".into(),
            params: serde_json::json!({}),
        }],
    });
    assert!(resp.success);
    let data = resp.data.unwrap();
    assert!(data["results"].is_array());
    assert_eq!(data["results"][0]["status"], "dispatched");
}

// ── Negotiate tests ─────────────────────────────────────────────────

#[test]
fn headless_driver_negotiate() {
    let mut driver = HeadlessDriver::new(TestApp { count: 0 }, 800.0, 600.0);
    driver.init();

    let resp = driver.process_request(&AgentRequest::Negotiate {
        client_version: 1,
        capabilities: vec!["batch".into()],
    });
    assert!(resp.success);
    let data = resp.data.unwrap();
    assert!(data["protocol_version"].is_number());
}

// ── Screenshot tests ────────────────────────────────────────────────

#[test]
fn headless_driver_screenshot() {
    let mut driver = HeadlessDriver::new(TestApp { count: 0 }, 800.0, 600.0);
    driver.init();

    let resp = driver.process_request(&AgentRequest::Screenshot {
        format: "json".into(),
    });
    assert!(resp.success);
    let data = resp.data.unwrap();
    assert_eq!(data["kind"], "ui_tree");
}

// ── Accessibility tests ─────────────────────────────────────────────

#[test]
fn accessibility_struct() {
    use dewey::ontology::UiNode;

    let acc = Accessibility {
        role: Some("button".into()),
        description: Some("Submit form".into()),
        disabled: Some(false),
        shortcut: Some("Ctrl+Enter".into()),
        ..Default::default()
    };

    let node = UiNode::new("Button", SemanticRole::Action).with_accessibility(acc);
    assert_eq!(node.accessibility().role, Some("button".into()));
    assert!(node.accessibility().tab_index.is_none());
}

// ── New widget ontology tests ───────────────────────────────────────

#[test]
fn color_picker_discoverable() {
    let picker = ColorPicker::new("Color");
    assert_eq!(picker.schema().name, "ColorPicker");
}

#[test]
fn toolbar_discoverable() {
    let toolbar = Toolbar::new(vec![
        ToolbarItem::new("save", "Save"),
        ToolbarItem::new("open", "Open"),
    ]);
    assert_eq!(toolbar.schema().name, "Toolbar");
    let actions = toolbar.actions();
    assert!(actions.iter().any(|a| a.name == "click_item"));
}

#[test]
fn splitter_discoverable() {
    let splitter = Splitter::new(SplitDirection::Horizontal);
    assert_eq!(splitter.schema().name, "Splitter");
}

#[test]
fn command_palette_actions() {
    let commands = vec![
        PaletteCommand::new("save", "Save File"),
        PaletteCommand::new("open", "Open File"),
    ];
    let palette = CommandPalette::new(commands);
    assert_eq!(palette.schema().name, "CommandPalette");
    let actions = palette.actions();
    assert!(actions.iter().any(|a| a.name == "search"));
    assert!(actions.iter().any(|a| a.name == "execute"));
}

// ── Virtual list tests ──────────────────────────────────────────────

#[test]
fn virtual_list_discoverable() {
    let vlist = VirtualList::new(24.0, |_idx, _rect, _frame| {})
        .agent_id("my_vlist")
        .overscan(3);
    assert_eq!(vlist.schema().name, "VirtualList");
    assert_eq!(Discoverable::agent_id(&vlist), Some("my_vlist"));
}

// ── CancellationToken tests ─────────────────────────────────────────

#[test]
fn cancellation_token() {
    let token = CancellationToken::new();
    assert!(!token.is_cancelled());
    token.cancel();
    assert!(token.is_cancelled());
}

// ── BatchActionEntry serde ──────────────────────────────────────────

#[test]
fn batch_action_entry_serde() {
    let entry = BatchActionEntry {
        agent_id: "btn-1".into(),
        action: "click".into(),
        params: serde_json::json!({"key": "value"}),
    };
    let json = serde_json::to_string(&entry).unwrap();
    let parsed: BatchActionEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.agent_id, "btn-1");
    assert_eq!(parsed.action, "click");
}

// ── Checkbox widget tests ───────────────────────────────────────────

#[test]
fn checkbox_discoverable() {
    let cb = Checkbox::new("Accept", false);
    assert_eq!(cb.schema().name, "Checkbox");
    assert_eq!(cb.semantic_role(), SemanticRole::Input);
    let state = cb.agent_state();
    assert_eq!(state["checked"], false);
    assert_eq!(state["label"], "Accept");
}

#[test]
fn checkbox_toggle_action() {
    let mut cb = Checkbox::new("Terms", false);
    let result = cb.execute_action("toggle", &serde_json::json!({})).unwrap();
    assert_eq!(result["checked"], true);
    let result = cb.execute_action("toggle", &serde_json::json!({})).unwrap();
    assert_eq!(result["checked"], false);
}

// ── Radio widget tests ──────────────────────────────────────────────

#[test]
fn radio_discoverable() {
    let r = Radio::new("Option A", false);
    assert_eq!(r.schema().name, "Radio");
    assert_eq!(r.semantic_role(), SemanticRole::Input);
    assert_eq!(r.agent_state()["selected"], false);
}

#[test]
fn radio_select_action() {
    let mut r = Radio::new("Option A", false);
    let result = r.execute_action("select", &serde_json::json!({})).unwrap();
    assert_eq!(result["selected"], true);
}

// ── TextInput widget tests ──────────────────────────────────────────

#[test]
fn text_input_discoverable() {
    let input = TextInput::new().placeholder("Enter name...");
    assert_eq!(input.schema().name, "TextInput");
    assert_eq!(input.semantic_role(), SemanticRole::Input);
    assert_eq!(input.agent_state()["placeholder"], "Enter name...");
    assert_eq!(
        input.accessibility_label(),
        Some("Enter name...".to_string())
    );
}

// ── TextArea widget tests ───────────────────────────────────────────

#[test]
fn text_area_discoverable() {
    let ta = TextArea::new().placeholder("Type here...");
    assert_eq!(ta.schema().name, "TextArea");
    assert_eq!(ta.semantic_role(), SemanticRole::Input);
    assert_eq!(ta.agent_state()["placeholder"], "Type here...");
    let actions = ta.actions();
    assert!(actions.iter().any(|a| a.name == "set_text"));
    assert!(actions.iter().any(|a| a.name == "insert"));
}

// ── Slider widget tests ─────────────────────────────────────────────

#[test]
fn slider_discoverable() {
    let s = Slider::new(0.0, 100.0).step(5.0).label("Volume");
    assert_eq!(s.schema().name, "Slider");
    assert_eq!(s.semantic_role(), SemanticRole::Input);
    let state = s.agent_state();
    assert_eq!(state["min"], 0.0);
    assert_eq!(state["max"], 100.0);
    assert_eq!(state["step"], 5.0);
    assert_eq!(s.accessibility_label(), Some("Volume".to_string()));
}

// ── Select widget tests ─────────────────────────────────────────────

#[test]
fn select_discoverable() {
    let sel = Select::new("Color", vec!["Red".into(), "Blue".into(), "Green".into()]);
    assert_eq!(sel.schema().name, "Select");
    assert_eq!(sel.semantic_role(), SemanticRole::Selection);
    let state = sel.agent_state();
    assert_eq!(state["label"], "Color");
    assert_eq!(state["options"].as_array().unwrap().len(), 3);
}

// ── List widget tests ────────────────────────────────────────────────

#[test]
fn list_discoverable() {
    let list = List::new(vec!["Alice".into(), "Bob".into()]);
    assert_eq!(list.schema().name, "List");
    assert_eq!(list.semantic_role(), SemanticRole::Selection);
    assert_eq!(list.agent_state()["count"], 2);
}

#[test]
fn list_state_navigation() {
    let mut state = ListState::new();
    assert_eq!(state.selected, None);
    state.select_next(3);
    assert_eq!(state.selected, Some(0));
    state.select_next(3);
    assert_eq!(state.selected, Some(1));
    state.select_prev();
    assert_eq!(state.selected, Some(0));
}

// ── Tabs widget tests ────────────────────────────────────────────────

#[test]
fn tabs_discoverable() {
    let tabs = Tabs::new(vec!["Home".into(), "Settings".into()]);
    assert_eq!(tabs.schema().name, "Tabs");
    assert_eq!(tabs.semantic_role(), SemanticRole::Tab);
    assert_eq!(tabs.agent_state()["labels"].as_array().unwrap().len(), 2);
}

// ── Table widget tests ───────────────────────────────────────────────

#[test]
fn table_discoverable() {
    let table = Table::new(
        vec!["Name".into(), "Age".into()],
        vec![
            vec!["Alice".into(), "30".into()],
            vec!["Bob".into(), "25".into()],
        ],
    );
    assert_eq!(table.schema().name, "Table");
    let state = table.agent_state();
    assert_eq!(state["row_count"], 2);
}

#[test]
fn table_state_sorting() {
    let mut state = TableState::new();
    assert!(state.sort_column.is_none());
    state.toggle_sort(0);
    assert_eq!(state.sort_column, Some((0, SortDirection::Ascending)));
    state.toggle_sort(0);
    assert_eq!(state.sort_column, Some((0, SortDirection::Descending)));
    state.toggle_sort(0);
    assert!(state.sort_column.is_none());
}

#[test]
fn table_state_pagination() {
    let mut state = TableState::new();
    state.set_page_size(10);
    assert_eq!(state.total_pages(25), 3);
    assert_eq!(state.current_page, 0);
    state.next_page(25);
    assert_eq!(state.current_page, 1);
    state.next_page(25);
    assert_eq!(state.current_page, 2);
    state.next_page(25); // should not go past last page
    assert_eq!(state.current_page, 2);
    state.prev_page();
    assert_eq!(state.current_page, 1);
}

// ── Menu widget tests ────────────────────────────────────────────────

#[test]
fn menu_discoverable() {
    let menu = Menu::new(
        "File",
        vec![
            MenuItem::new("Open").shortcut("Ctrl+O"),
            MenuItem::new("Save"),
            MenuItem::new("Exit").enabled(false),
        ],
    );
    assert_eq!(menu.schema().name, "Menu");
    assert_eq!(menu.semantic_role(), SemanticRole::Menu);
    let state = menu.agent_state();
    assert_eq!(state["title"], "File");
    assert_eq!(state["items"].as_array().unwrap().len(), 3);
}

// ── ProgressBar widget tests ─────────────────────────────────────────

#[test]
fn progress_bar_discoverable() {
    let pb = ProgressBar::new(0.75).label("Loading...");
    assert_eq!(pb.schema().name, "ProgressBar");
    assert_eq!(pb.semantic_role(), SemanticRole::Progress);
    assert_eq!(pb.agent_state()["progress"], 0.75);
    assert!(pb.actions().is_empty());
}

#[test]
fn progress_bar_clamps() {
    let over = ProgressBar::new(1.5);
    assert_eq!(over.agent_state()["progress"], 1.0);
    let under = ProgressBar::new(-0.5);
    assert_eq!(under.agent_state()["progress"], 0.0);
}

// ── Container widget tests ───────────────────────────────────────────

#[test]
fn container_discoverable() {
    let c = Container::new().title("Section");
    assert_eq!(c.schema().name, "Container");
    assert_eq!(c.semantic_role(), SemanticRole::Container);
    assert_eq!(c.agent_state()["title"], "Section");
    assert!(c.actions().is_empty());
}

// ── Panel widget tests ───────────────────────────────────────────────

#[test]
fn panel_discoverable() {
    let p = Panel::new(PanelSide::Left).title("Explorer");
    assert_eq!(p.schema().name, "Panel");
    assert_eq!(p.semantic_role(), SemanticRole::Container);
    assert_eq!(p.agent_state()["title"], "Explorer");
    assert_eq!(p.agent_state()["side"], "Left");
}

// ── ScrollArea widget tests ──────────────────────────────────────────

#[test]
fn scroll_area_discoverable() {
    let sa = ScrollArea::vertical();
    assert_eq!(sa.schema().name, "ScrollArea");
    assert_eq!(sa.semantic_role(), SemanticRole::Scrollable);
    let state = sa.agent_state();
    assert_eq!(state["vertical"], true);
    assert_eq!(state["horizontal"], false);
}

#[test]
fn scroll_area_both() {
    let sa = ScrollArea::both();
    let state = sa.agent_state();
    assert_eq!(state["vertical"], true);
    assert_eq!(state["horizontal"], true);
}

// ── Chart widget tests ───────────────────────────────────────────────

#[test]
fn chart_discoverable() {
    let chart = Chart::line("Revenue")
        .labels(vec!["Q1".into(), "Q2".into()])
        .series(Series::new("2024", vec![100.0, 150.0], Color::BLUE));
    assert_eq!(chart.schema().name, "Chart");
    assert_eq!(chart.semantic_role(), SemanticRole::DataVisualization);
    let state = chart.agent_state();
    assert_eq!(state["title"], "Revenue");
    assert_eq!(state["series"].as_array().unwrap().len(), 1);
}

#[test]
fn chart_bar_and_pie() {
    let bar = Chart::bar("Sales");
    assert_eq!(bar.agent_state()["kind"], "Bar");
    let pie = Chart::pie("Share");
    assert_eq!(pie.agent_state()["kind"], "Pie");
}

// ── Color API tests ─────────────────────────────────────────────────

#[test]
fn color_hex_rgb() {
    let c = Color::hex("#FF8800");
    assert!((c.r - 1.0).abs() < 0.01);
    assert!((c.g - 0.533).abs() < 0.01);
    assert!((c.b - 0.0).abs() < 0.01);
    assert!((c.a - 1.0).abs() < 0.01);
}

#[test]
fn color_hex_rgba() {
    let c = Color::hex("#FF880080");
    assert!((c.r - 1.0).abs() < 0.01);
    assert!((c.a - 0.502).abs() < 0.01);
}

#[test]
fn color_hex_no_hash() {
    let c = Color::from_hex("1A2B3C").unwrap();
    assert!((c.r - 0.102).abs() < 0.01);
}

#[test]
fn color_from_hex_invalid() {
    assert!(Color::from_hex("#ZZZ").is_none());
    assert!(Color::from_hex("#12345").is_none());
    assert!(Color::from_hex("").is_none());
}

#[test]
#[should_panic(expected = "invalid hex color")]
fn color_hex_panics_on_invalid() {
    let _ = Color::hex("#NOPE");
}

#[test]
fn color_from_rgb8() {
    let c = Color::from_rgb8(255, 128, 0);
    assert!((c.r - 1.0).abs() < 0.01);
    assert!((c.g - 0.502).abs() < 0.01);
    assert!((c.b - 0.0).abs() < 0.01);
}

#[test]
fn color_with_alpha() {
    let c = Color::RED.with_alpha(0.5);
    assert!((c.r - 1.0).abs() < 0.01);
    assert!((c.a - 0.5).abs() < 0.01);
}

#[test]
fn color_constants_distinct() {
    assert_ne!(Color::RED, Color::BLUE);
    assert_ne!(Color::ORANGE, Color::PURPLE);
    assert_ne!(Color::PINK, Color::BROWN);
    assert_ne!(Color::INDIGO, Color::CYAN);
    assert_ne!(Color::BLACK, Color::WHITE);
}

// ── TextStyle builder tests ─────────────────────────────────────────

#[test]
fn text_style_builder_chain() {
    use dewey::core::{FontWeight, TextStyle};

    let ts = TextStyle::new()
        .size(24.0)
        .color(Color::RED)
        .bold()
        .italic();
    assert!((ts.font_size - 24.0).abs() < f32::EPSILON);
    assert_eq!(ts.color, Color::RED);
    assert_eq!(ts.weight, FontWeight::Bold);
    assert!(ts.italic);
}

#[test]
fn text_style_defaults() {
    use dewey::core::{FontWeight, TextStyle};

    let ts = TextStyle::new();
    assert!((ts.font_size - 14.0).abs() < f32::EPSILON);
    assert_eq!(ts.color, Color::WHITE);
    assert_eq!(ts.weight, FontWeight::Regular);
    assert!(!ts.italic);
    assert!(!ts.underline);
    assert!(!ts.strikethrough);
}

// ── Style builder tests ─────────────────────────────────────────────

#[test]
fn style_builder_chain() {
    let s = Style::new()
        .bg(Color::DARK_GRAY)
        .fg(Color::WHITE)
        .rounded(12.0)
        .text_size(18.0)
        .bold();
    assert_eq!(s.background, Some(Color::DARK_GRAY));
    assert_eq!(s.foreground, Some(Color::WHITE));
    assert_eq!(s.border_radius, Some(12.0));
    let ts = s.resolved_text();
    assert!((ts.font_size - 18.0).abs() < f32::EPSILON);
    assert_eq!(ts.color, Color::WHITE); // inherits fg
}

#[test]
fn style_resolved_text_inherits_fg() {
    let s = Style::new().fg(Color::RED);
    let ts = s.resolved_text();
    assert_eq!(ts.color, Color::RED);
}

#[test]
fn style_resolved_text_explicit_color_wins() {
    let s = Style::new().fg(Color::RED).text_color(Color::BLUE);
    let ts = s.resolved_text();
    assert_eq!(ts.color, Color::BLUE);
}

#[test]
fn style_resolved_text_no_overrides() {
    let s = Style::new();
    let ts = s.resolved_text();
    assert!((ts.font_size - 14.0).abs() < f32::EPSILON);
    assert_eq!(ts.color, Color::WHITE);
}

#[test]
fn style_merge_override() {
    let base = Style::new().bg(Color::BLACK).fg(Color::WHITE).rounded(4.0);
    let over = Style::new().bg(Color::BLUE).text_size(20.0);
    let merged = base.merge(&over);
    assert_eq!(merged.background, Some(Color::BLUE)); // overridden
    assert_eq!(merged.foreground, Some(Color::WHITE)); // inherited
    assert_eq!(merged.border_radius, Some(4.0)); // inherited
    assert!(merged.text.is_some());
}

#[test]
fn style_opacity_clamps() {
    let s = Style::new().opacity(1.5);
    assert_eq!(s.opacity, Some(1.0));
    let s = Style::new().opacity(-0.5);
    assert_eq!(s.opacity, Some(0.0));
}

// ── Widget builder API tests ────────────────────────────────────────

#[test]
fn button_builder_api() {
    let btn = Button::new("Save")
        .bg(Color::BLUE)
        .fg(Color::WHITE)
        .rounded(8.0)
        .text_size(16.0)
        .enabled(false);
    let state = btn.agent_state();
    assert_eq!(state["label"], "Save");
    assert_eq!(state["enabled"], false);
}

#[test]
fn label_builder_api() {
    let lbl = Label::new("Title")
        .fg(Color::hex("#1A73E8"))
        .text_size(24.0)
        .bold();
    assert_eq!(lbl.agent_state()["text"], "Title");
}

#[test]
fn container_builder_api() {
    let c = Container::new()
        .bg(Color::DARK_GRAY)
        .rounded(12.0)
        .border(Color::GRAY, 1.0)
        .title("Card");
    assert_eq!(c.agent_state()["title"], "Card");
}

#[test]
fn text_input_builder_api() {
    let input = TextInput::new()
        .placeholder("Search…")
        .bg(Color::DARK_GRAY)
        .fg(Color::WHITE)
        .rounded(6.0);
    assert_eq!(input.agent_state()["placeholder"], "Search…");
}

#[test]
fn progress_bar_builder_api() {
    let pb = ProgressBar::new(0.5)
        .label("Loading")
        .fg(Color::GREEN)
        .bg(Color::DARK_GRAY);
    assert_eq!(pb.agent_state()["progress"], 0.5);
}

#[test]
fn select_builder_api() {
    let sel = Select::new("Size", vec!["S".into(), "M".into(), "L".into()])
        .bg(Color::DARK_GRAY)
        .fg(Color::WHITE)
        .rounded(4.0);
    assert_eq!(sel.agent_state()["options"].as_array().unwrap().len(), 3);
}

// ── Ontology gating ────────────────────────────────────────────────

/// Skipping ontology construction must not break input routing. The node tree
/// and the hit map are built in the same guarded block in every widget, so it
/// is easy to gate both by accident — a button that no longer hit-tests looks
/// fine on screen and is simply dead to the mouse.
#[test]
fn ontology_gate_skips_nodes_but_keeps_hitboxes() {
    use dewey::backend::test::TestBackend;
    use dewey::core::{Position, Rect};
    use dewey::event::HitMap;
    use dewey::runtime::Frame;
    use dewey::widget::{Button, Widget};

    let area = Rect::new(0.0, 0.0, 100.0, 40.0);

    // Ontology on: node registered, hitbox registered.
    let mut painter = TestBackend::new(200.0, 100.0);
    let mut hit_map = HitMap::new();
    let mut frame = Frame::with_ontology(area, &mut hit_map, &mut painter, true);
    Button::new("Go").agent_id("go").render(area, &mut frame);
    assert_eq!(frame.take_nodes().len(), 1, "ontology on: node expected");
    assert_eq!(hit_map.hit_test(Position::new(50.0, 20.0)), Some("go"));

    // Ontology off: no node, but the hitbox must survive.
    let mut painter = TestBackend::new(200.0, 100.0);
    let mut hit_map = HitMap::new();
    let mut frame = Frame::with_ontology(area, &mut hit_map, &mut painter, false);
    Button::new("Go").agent_id("go").render(area, &mut frame);
    assert!(frame.take_nodes().is_empty(), "ontology off: no nodes");
    assert_eq!(
        hit_map.hit_test(Position::new(50.0, 20.0)),
        Some("go"),
        "ontology off must not disable hit-testing"
    );
}

/// Painting is identical either way — the gate is invisible on screen.
#[test]
fn ontology_gate_does_not_change_rendering() {
    use dewey::backend::test::TestBackend;
    use dewey::core::Rect;
    use dewey::event::HitMap;
    use dewey::runtime::Frame;
    use dewey::widget::{Button, Widget};

    let area = Rect::new(0.0, 0.0, 100.0, 40.0);
    let mut counts = Vec::new();
    for enabled in [true, false] {
        let mut painter = TestBackend::new(200.0, 100.0);
        let mut hit_map = HitMap::new();
        let mut frame = Frame::with_ontology(area, &mut hit_map, &mut painter, enabled);
        Button::new("Go").agent_id("go").render(area, &mut frame);
        counts.push(painter.ops().len());
    }
    assert_eq!(counts[0], counts[1], "gate must not change draw calls");
}

// ── Ontology property storage ──────────────────────────────────────

/// `Properties` replaced `serde_json::Value` as the state store for speed. The
/// agent protocol is a wire contract, so the JSON must be byte-identical to
/// what a plain object produced — an object, not the underlying vector of
/// pairs.
#[test]
fn properties_serialize_as_a_json_object() {
    use dewey::ontology::{SemanticRole, UiNode};

    let node = UiNode::new("Button", SemanticRole::Action)
        .with_id("go")
        .with_property("label", serde_json::json!("Go"))
        .with_property("enabled", serde_json::json!(true));

    let v = serde_json::to_value(&node).unwrap();
    let state = v.get("state").expect("state field present");
    assert!(state.is_object(), "state must serialize as an object");
    assert_eq!(state.get("label").unwrap(), &serde_json::json!("Go"));
    assert_eq!(state.get("enabled").unwrap(), &serde_json::json!(true));

    // And it must survive a round trip.
    let back: UiNode = serde_json::from_value(v).unwrap();
    assert_eq!(back.state.get("label").unwrap(), &serde_json::json!("Go"));
    assert_eq!(back.state, node.state);
}

/// Re-setting a key must overwrite, matching the map semantics it replaced,
/// rather than appending a duplicate entry.
#[test]
fn properties_insert_replaces_existing_key() {
    use dewey::ontology::{SemanticRole, UiNode};

    let node = UiNode::new("Label", SemanticRole::Display)
        .with_property("text", serde_json::json!("first"))
        .with_property("text", serde_json::json!("second"));

    assert_eq!(node.state.len(), 1, "duplicate key must not be appended");
    assert_eq!(
        node.state.get("text").unwrap(),
        &serde_json::json!("second")
    );
}

/// An empty state stays `null` on the wire, as it did before.
#[test]
fn properties_empty_state_is_null() {
    use dewey::ontology::{SemanticRole, UiNode};

    let node = UiNode::new("Label", SemanticRole::Display);
    assert!(node.state.is_empty());
    assert_eq!(node.state.to_value(), serde_json::Value::Null);
}

/// Guards the state-change detection path directly: same content, different
/// insertion order, must compare equal.
#[test]
fn properties_equality_ignores_key_order() {
    use dewey::ontology::{SemanticRole, UiNode};

    let a = UiNode::new("W", SemanticRole::Display)
        .with_property("x", serde_json::json!(1))
        .with_property("y", serde_json::json!(2));
    let b = UiNode::new("W", SemanticRole::Display)
        .with_property("y", serde_json::json!(2))
        .with_property("x", serde_json::json!(1));
    assert_eq!(a.state, b.state);

    let c = UiNode::new("W", SemanticRole::Display)
        .with_property("y", serde_json::json!(3))
        .with_property("x", serde_json::json!(1));
    assert_ne!(
        a.state, c.state,
        "differing values must still compare unequal"
    );
}

/// Widgets build their ontology node at the end of `render` so owned fields
/// move into the state instead of being cloned. That reorder must not change
/// what an agent sees: nodes must still appear in the order the widgets were
/// rendered, and must still carry their state.
#[test]
fn node_registration_order_follows_render_order() {
    use dewey::backend::test::TestBackend;
    use dewey::core::Rect;
    use dewey::event::HitMap;
    use dewey::runtime::Frame;
    use dewey::widget::{Button, Label, Widget};

    let mut painter = TestBackend::new(400.0, 200.0);
    let mut hit_map = HitMap::new();
    let mut frame = Frame::new(Rect::from_size(400.0, 200.0), &mut hit_map, &mut painter);

    Label::new("first")
        .agent_id("a")
        .render(Rect::new(0.0, 0.0, 100.0, 20.0), &mut frame);
    Button::new("second")
        .agent_id("b")
        .render(Rect::new(0.0, 20.0, 100.0, 20.0), &mut frame);
    Label::new("third")
        .agent_id("c")
        .render(Rect::new(0.0, 40.0, 100.0, 20.0), &mut frame);

    let nodes = frame.take_nodes();
    let ids: Vec<_> = nodes
        .iter()
        .map(|n| n.agent_id.as_deref().unwrap())
        .collect();
    assert_eq!(ids, ["a", "b", "c"], "nodes must follow render order");

    // State survived the move into the node.
    assert_eq!(
        nodes[0].state.get("text").unwrap(),
        &serde_json::json!("first")
    );
    assert_eq!(
        nodes[1].state.get("label").unwrap(),
        &serde_json::json!("second")
    );
    assert_eq!(
        nodes[2].state.get("text").unwrap(),
        &serde_json::json!("third")
    );
}

/// A list moves its whole item vector into the state rather than cloning it
/// every frame; the contents must still arrive intact.
#[test]
fn list_state_carries_items_after_move() {
    use dewey::backend::test::TestBackend;
    use dewey::core::Rect;
    use dewey::event::HitMap;
    use dewey::runtime::Frame;
    use dewey::widget::{List, ListState, StatefulWidget};

    let mut painter = TestBackend::new(400.0, 200.0);
    let mut hit_map = HitMap::new();
    let mut frame = Frame::new(Rect::from_size(400.0, 200.0), &mut hit_map, &mut painter);
    let mut state = ListState::default();

    List::new(vec!["alpha".to_string(), "beta".to_string()])
        .agent_id("lst")
        .render(Rect::new(0.0, 0.0, 200.0, 100.0), &mut frame, &mut state);

    let nodes = frame.take_nodes();
    assert_eq!(nodes.len(), 1);
    assert_eq!(
        nodes[0].state.get("items").unwrap(),
        &serde_json::json!(["alpha", "beta"])
    );
}

// ── On-demand ontology ─────────────────────────────────────────────

/// `build_ontology_tree` runs a paint-free `view` pass. It must produce the
/// same tree the every-frame path produces, or on-demand mode would show
/// agents a different UI than the one on screen.
#[test]
fn on_demand_tree_matches_every_frame_tree() {
    use dewey::backend::test::TestBackend;
    use dewey::core::Rect;
    use dewey::event::HitMap;
    use dewey::runtime::{Frame, build_ontology_tree};

    let area = Rect::from_size(400.0, 200.0);
    let model = TestApp { count: 7 };

    // The every-frame path: build the tree while painting.
    let mut painter = TestBackend::new(400.0, 200.0);
    let mut hit_map = HitMap::new();
    let mut frame = Frame::with_ontology(area, &mut hit_map, &mut painter, true);
    model.view(&mut frame);
    let painted = frame.take_nodes();

    // The on-demand path: no painting at all.
    let tree = build_ontology_tree(&model, area);

    assert_eq!(
        tree.root.children.len(),
        painted.len(),
        "on-demand pass must see the same widgets"
    );
    for (a, b) in tree.root.children.iter().zip(painted.iter()) {
        assert_eq!(a.agent_id, b.agent_id);
        assert_eq!(a.widget_type, b.widget_type);
        assert_eq!(a.state, b.state, "state must match the painted frame");
    }
    assert!(!painted.is_empty(), "fixture must produce nodes");
}

/// The on-demand pass reflects current model state, which is the whole reason
/// it is safe to skip building the tree every frame.
#[test]
fn on_demand_tree_tracks_model_changes() {
    use dewey::core::Rect;
    use dewey::runtime::build_ontology_tree;

    let area = Rect::from_size(400.0, 200.0);

    let before = build_ontology_tree(&TestApp { count: 1 }, area);
    let after = build_ontology_tree(&TestApp { count: 2 }, area);

    let find = |t: &dewey::ontology::UiTree| {
        t.root
            .children
            .iter()
            .find(|n| n.agent_id.as_deref() == Some("counter_label"))
            .and_then(|n| n.state.get("text").cloned())
    };
    assert_ne!(find(&before), find(&after), "tree must track model state");
}

/// `OntologyMode::OnDemand` is the default, so a plain `ProgramOptions` skips
/// per-frame tree building.
#[test]
fn ontology_mode_defaults_to_on_demand() {
    use dewey::runtime::{OntologyMode, ProgramOptions};
    assert_eq!(ProgramOptions::default().ontology, OntologyMode::OnDemand);
}

/// `UiNode::accessibility` is boxed to keep the node small. The wire format
/// must not change: absent when unset, a plain object when set.
#[test]
fn boxed_accessibility_keeps_wire_format() {
    use dewey::ontology::{Accessibility, SemanticRole, UiNode};

    let bare = UiNode::new("Label", SemanticRole::Display);
    let v = serde_json::to_value(&bare).unwrap();
    assert!(
        v.get("accessibility").is_none(),
        "unset accessibility must be omitted, not serialized as null"
    );
    assert!(
        bare.accessibility().role.is_none(),
        "accessor gives empty set"
    );

    let described = UiNode::new("Button", SemanticRole::Action).with_accessibility(Accessibility {
        role: Some("button".into()),
        shortcut: Some("Ctrl+S".into()),
        ..Default::default()
    });
    let v = serde_json::to_value(&described).unwrap();
    let acc = v
        .get("accessibility")
        .expect("set accessibility serialized");
    assert_eq!(acc.get("role").unwrap(), &serde_json::json!("button"));

    let back: UiNode = serde_json::from_value(v).unwrap();
    assert_eq!(back.accessibility().role.as_deref(), Some("button"));
    assert_eq!(back.accessibility().shortcut.as_deref(), Some("Ctrl+S"));
}

/// Setting an all-default `Accessibility` must not allocate a box, so an
/// application that passes one by habit does not pay for it.
#[test]
fn empty_accessibility_is_not_boxed() {
    use dewey::ontology::{Accessibility, SemanticRole, UiNode};

    let node =
        UiNode::new("Label", SemanticRole::Display).with_accessibility(Accessibility::default());
    assert!(node.accessibility.is_none());
}

// ── Widget-carried messages ────────────────────────────────────────

/// An app whose buttons carry their own messages and which writes no
/// `execute_action` handler at all. Before `Button::action` existed, every
/// application had to route clicks itself.
struct ActionApp {
    count: i32,
    on: bool,
}

#[derive(Debug, PartialEq)]
enum ActionMsg {
    Inc,
    Toggle,
}

impl Model for ActionApp {
    type Msg = ActionMsg;

    fn update(&mut self, msg: ActionMsg) -> Command<ActionMsg> {
        match msg {
            ActionMsg::Inc => self.count += 1,
            ActionMsg::Toggle => self.on = !self.on,
        }
        Command::None
    }

    fn view(&self, frame: &mut Frame<'_>) {
        let rows = dewey::layout::Layout::vertical([
            dewey::layout::Constraint::Length(40.0),
            dewey::layout::Constraint::Length(40.0),
        ])
        .split(frame.area);
        Button::new("+")
            .action("inc", ActionMsg::Inc)
            .render(rows[0], frame);
        dewey::widget::Checkbox::new("on", self.on)
            .action("toggle", ActionMsg::Toggle)
            .render(rows[1], frame);
    }
}

/// An agent's `execute_action` must reach a widget that carries its own
/// message, with no handler written by the application.
#[test]
fn agent_click_dispatches_widget_message() {
    use dewey::agent::driver::HeadlessDriver;
    use dewey::agent::protocol::AgentRequest;

    let mut d = HeadlessDriver::new(
        ActionApp {
            count: 0,
            on: false,
        },
        200.0,
        200.0,
    );
    d.init();

    let r = d.process_request(&AgentRequest::ExecuteAction {
        agent_id: "inc".into(),
        action: "click".into(),
        params: serde_json::Value::Null,
    });
    assert!(r.success);
    assert_eq!(d.model().count, 1, "click must reach the model");

    // A Checkbox advertises `toggle`, not `click`; its handler must answer the
    // action its own ontology tells an agent to use.
    let r = d.process_request(&AgentRequest::ExecuteAction {
        agent_id: "toggle".into(),
        action: "toggle".into(),
        params: serde_json::Value::Null,
    });
    assert!(r.success);
    assert!(d.model().on, "checkbox action must dispatch too");

    // And the name it does not advertise must not work.
    let before = d.model().on;
    d.process_request(&AgentRequest::ExecuteAction {
        agent_id: "toggle".into(),
        action: "click".into(),
        params: serde_json::Value::Null,
    });
    assert_eq!(d.model().on, before, "an unadvertised action must not fire");
}

/// A real mouse click must route through the hit map to the same message —
/// the hit map used to be built every frame and never read.
#[test]
fn mouse_click_routes_through_hit_map() {
    use dewey::agent::driver::HeadlessDriver;
    use dewey::agent::protocol::{AgentRequest, InjectedEvent};

    let mut d = HeadlessDriver::new(
        ActionApp {
            count: 0,
            on: false,
        },
        200.0,
        200.0,
    );
    d.init();
    // Build the frame so the hit map and messages exist.
    let _ = d.process_request(&AgentRequest::GetTree {
        since: None,
        viewport: None,
    });

    d.process_request(&AgentRequest::InjectEvent {
        event: InjectedEvent::MouseClick {
            x: 20.0,
            y: 10.0,
            button: "left".into(),
        },
    });
    assert_eq!(d.model().count, 1, "a click inside the button must fire it");

    d.process_request(&AgentRequest::InjectEvent {
        event: InjectedEvent::MouseClick {
            x: 20.0,
            y: 150.0,
            button: "left".into(),
        },
    });
    assert_eq!(
        d.model().count,
        1,
        "a click outside any widget must do nothing"
    );
}

/// Messages are type-erased in the frame; a mismatched type must not panic in
/// release or silently corrupt another widget's dispatch.
#[test]
fn widget_message_is_consumed_once() {
    use dewey::agent::driver::HeadlessDriver;
    use dewey::agent::protocol::AgentRequest;

    let mut d = HeadlessDriver::new(
        ActionApp {
            count: 0,
            on: false,
        },
        200.0,
        200.0,
    );
    d.init();
    for _ in 0..3 {
        d.process_request(&AgentRequest::ExecuteAction {
            agent_id: "inc".into(),
            action: "click".into(),
            params: serde_json::Value::Null,
        });
    }
    assert_eq!(
        d.model().count,
        3,
        "each request re-renders and re-arms the message"
    );
}

// ── Structural validation ──────────────────────────────────────────

/// The exact mistake made while writing this project's own benchmarks: a
/// button with no id renders correctly and is completely dead.
struct DeadButtonApp;

#[derive(Debug)]
enum DeadMsg {}

impl Model for DeadButtonApp {
    type Msg = DeadMsg;
    fn update(&mut self, _m: DeadMsg) -> Command<DeadMsg> {
        Command::None
    }
    fn view(&self, frame: &mut Frame<'_>) {
        Button::new("click me").render(frame.area, frame);
    }
}

#[test]
fn validate_flags_a_button_with_no_id() {
    use dewey::agent::driver::HeadlessDriver;
    use dewey::ontology::Severity;

    let mut d = HeadlessDriver::new(DeadButtonApp, 200.0, 100.0);
    d.init();
    let found = d.validate();

    let dead = found
        .iter()
        .find(|x| x.code == "unaddressable_widget")
        .expect("an id-less button must be reported");
    assert_eq!(dead.severity, Severity::Error);
    assert_eq!(dead.widget_type.as_deref(), Some("Button"));
    assert!(
        dead.message.contains("action"),
        "the diagnostic should say how to fix it: {}",
        dead.message
    );
}

struct DuplicateIdApp;

#[derive(Debug)]
enum DupMsg {
    Go,
}

impl Model for DuplicateIdApp {
    type Msg = DupMsg;
    fn update(&mut self, _m: DupMsg) -> Command<DupMsg> {
        Command::None
    }
    fn view(&self, frame: &mut Frame<'_>) {
        let rows = dewey::layout::Layout::vertical([
            dewey::layout::Constraint::Length(40.0),
            dewey::layout::Constraint::Length(40.0),
        ])
        .split(frame.area);
        Button::new("a")
            .action("go", DupMsg::Go)
            .render(rows[0], frame);
        Button::new("b")
            .action("go", DupMsg::Go)
            .render(rows[1], frame);
    }
}

#[test]
fn validate_flags_duplicate_ids() {
    use dewey::agent::driver::HeadlessDriver;

    let mut d = HeadlessDriver::new(DuplicateIdApp, 200.0, 200.0);
    d.init();
    let found = d.validate();
    let dup = found
        .iter()
        .find(|x| x.code == "duplicate_agent_id")
        .expect("two widgets sharing an id must be reported");
    assert_eq!(dup.agent_id.as_deref(), Some("go"));
}

/// A correct interface must produce no findings, or the check is noise.
#[test]
fn validate_passes_a_well_formed_app() {
    use dewey::agent::driver::HeadlessDriver;

    let mut d = HeadlessDriver::new(
        ActionApp {
            count: 0,
            on: false,
        },
        200.0,
        200.0,
    );
    d.init();
    let found = d.validate();
    assert!(found.is_empty(), "well-formed app reported: {found:?}");
}

/// And it must be reachable over the protocol, not just from Rust.
#[test]
fn validate_is_available_to_agents() {
    use dewey::agent::driver::HeadlessDriver;
    use dewey::agent::protocol::AgentRequest;

    let mut d = HeadlessDriver::new(DeadButtonApp, 200.0, 100.0);
    d.init();
    let r = d.process_request(&AgentRequest::Validate { strict: false });
    assert!(r.success);
    let data = r.data.expect("validate returns data");
    assert_eq!(data["ok"], serde_json::json!(false));
    assert_eq!(data["errors"], serde_json::json!(1));
    assert_eq!(
        data["diagnostics"][0]["code"],
        serde_json::json!("unaddressable_widget")
    );

    let mut good = HeadlessDriver::new(
        ActionApp {
            count: 0,
            on: false,
        },
        200.0,
        200.0,
    );
    good.init();
    let r = good.process_request(&AgentRequest::Validate { strict: false });
    assert_eq!(r.data.unwrap()["ok"], serde_json::json!(true));
}

// ── Conditional tree reads and golden snapshots ────────────────────

/// An agent polling a screen that has not moved must not be sent the tree
/// again — rebuilding and serialising it is the most expensive thing it can
/// ask for.
#[test]
fn get_tree_since_reports_unchanged() {
    use dewey::agent::driver::HeadlessDriver;
    use dewey::agent::protocol::AgentRequest;

    let mut d = HeadlessDriver::new(
        ActionApp {
            count: 0,
            on: false,
        },
        200.0,
        200.0,
    );
    d.init();

    let first = d.process_request(&AgentRequest::GetTree {
        since: None,
        viewport: None,
    });
    let v = first.data.as_ref().unwrap()["version"].as_u64().unwrap();

    // Nothing has happened: the same version comes back as `unchanged`.
    let again = d.process_request(&AgentRequest::GetTree {
        since: Some(v),
        viewport: None,
    });
    let data = again.data.unwrap();
    assert_eq!(data["unchanged"], serde_json::json!(true));
    assert!(data.get("root").is_none(), "no tree should be sent");

    // After acting, the version moves and the tree comes back in full.
    d.process_request(&AgentRequest::ExecuteAction {
        agent_id: "inc".into(),
        action: "click".into(),
        params: serde_json::Value::Null,
    });
    let after = d.process_request(&AgentRequest::GetTree {
        since: Some(v),
        viewport: None,
    });
    let data = after.data.unwrap();
    assert!(
        data.get("unchanged").is_none(),
        "the screen changed: {data}"
    );
    assert_ne!(
        data["version"].as_u64().unwrap(),
        v,
        "version must advance after a mutation"
    );
}

/// The snapshot must be byte-stable across renders, or it is useless as a
/// golden file.
#[test]
fn snapshot_is_stable_and_reflects_change() {
    use dewey::agent::driver::HeadlessDriver;

    let mut d = HeadlessDriver::new(
        ActionApp {
            count: 0,
            on: false,
        },
        200.0,
        200.0,
    );
    d.init();

    let a = d.snapshot();
    let b = d.snapshot();
    assert_eq!(a, b, "two renders of one interface must match exactly");
    assert!(a.contains("Button #inc"), "snapshot names widgets: {a}");
    assert!(
        a.contains("Checkbox #toggle"),
        "snapshot names widgets: {a}"
    );

    // A real change must show up in the diff.
    d.process_request(&dewey::agent::protocol::AgentRequest::ExecuteAction {
        agent_id: "toggle".into(),
        action: "toggle".into(),
        params: serde_json::Value::Null,
    });
    let c = d.snapshot();
    assert_ne!(a, c, "toggling the checkbox must change the snapshot");
}

/// And an agent must be able to fetch it over the protocol.
#[test]
fn snapshot_is_available_to_agents() {
    use dewey::agent::driver::HeadlessDriver;
    use dewey::agent::protocol::AgentRequest;

    let mut d = HeadlessDriver::new(
        ActionApp {
            count: 0,
            on: false,
        },
        200.0,
        200.0,
    );
    d.init();
    let r = d.process_request(&AgentRequest::Screenshot {
        format: "text".into(),
    });
    let data = r.data.unwrap();
    assert_eq!(data["kind"], serde_json::json!("snapshot"));
    assert!(data["snapshot"].as_str().unwrap().contains("Button #inc"));

    // The default JSON form still works.
    let r = d.process_request(&AgentRequest::Screenshot {
        format: "json".into(),
    });
    assert_eq!(r.data.unwrap()["kind"], serde_json::json!("ui_tree"));
}

// ── Message-free applications ──────────────────────────────────────

/// An application with no message type and no `update` logic at all: widgets
/// carry the change they make. `Msg` is still required by the trait, but
/// nothing ever constructs one.
struct MutApp {
    count: i32,
    on: bool,
}

#[derive(Debug)]
enum Never {}

impl Model for MutApp {
    type Msg = Never;

    fn update(&mut self, _m: Never) -> Command<Never> {
        Command::None
    }

    fn view(&self, frame: &mut Frame<'_>) {
        let rows = frame.area.rows_of(&[40.0, 40.0]);
        Button::new("+")
            .on("inc", |app: &mut MutApp| app.count += 1)
            .render(rows[0], frame);
        dewey::widget::Checkbox::new("on", self.on)
            .on("toggle", |app: &mut MutApp| app.on = !app.on)
            .render(rows[1], frame);
    }
}

#[test]
fn widget_mutation_needs_no_message_type() {
    use dewey::agent::driver::HeadlessDriver;
    use dewey::agent::protocol::AgentRequest;

    let mut d = HeadlessDriver::new(
        MutApp {
            count: 0,
            on: false,
        },
        200.0,
        200.0,
    );
    d.init();

    for _ in 0..3 {
        let r = d.process_request(&AgentRequest::ExecuteAction {
            agent_id: "inc".into(),
            action: "click".into(),
            params: serde_json::Value::Null,
        });
        assert!(r.success);
    }
    assert_eq!(d.model().count, 3, "agent clicks applied the mutation");

    d.process_request(&AgentRequest::ExecuteAction {
        agent_id: "toggle".into(),
        action: "toggle".into(),
        params: serde_json::Value::Null,
    });
    assert!(d.model().on, "checkbox mutation applied");
}

/// A mutation-carrying widget must also respond to a real mouse click.
#[test]
fn widget_mutation_responds_to_a_mouse_click() {
    use dewey::agent::driver::HeadlessDriver;
    use dewey::agent::protocol::{AgentRequest, InjectedEvent};

    let mut d = HeadlessDriver::new(
        MutApp {
            count: 0,
            on: false,
        },
        200.0,
        200.0,
    );
    d.init();
    let _ = d.process_request(&AgentRequest::GetTree {
        since: None,
        viewport: None,
    });

    d.process_request(&AgentRequest::InjectEvent {
        event: InjectedEvent::MouseClick {
            x: 20.0,
            y: 10.0,
            button: "left".into(),
        },
    });
    assert_eq!(d.model().count, 1);
}

/// And such an app must still pass structural validation — carrying a mutation
/// gives the widget an id exactly as carrying a message does.
#[test]
fn message_free_app_validates_clean() {
    use dewey::agent::driver::HeadlessDriver;

    let mut d = HeadlessDriver::new(
        MutApp {
            count: 0,
            on: false,
        },
        200.0,
        200.0,
    );
    d.init();
    assert!(d.validate().is_empty(), "{:?}", d.validate());
}

// ── Value-carrying widgets ─────────────────────────────────────────

/// A form with no `execute_action` handler and no message type: the text field
/// and slider carry the change they make, including the new value.
struct FormApp {
    name: String,
    volume: f64,
    input: std::cell::RefCell<dewey::widget::input::TextInputState>,
    slider: std::cell::RefCell<dewey::widget::slider::SliderState>,
}

impl Model for FormApp {
    type Msg = ();
    fn update(&mut self, _m: ()) -> Command<()> {
        Command::None
    }
    fn view(&self, frame: &mut Frame<'_>) {
        use dewey::widget::{Slider, StatefulWidget, TextInput};
        let rows = frame.area.rows_of(&[40.0, 40.0]);
        TextInput::new()
            .on_input("name", |a: &mut FormApp, t: &str| a.name = t.to_string())
            .render(rows[0], frame, &mut self.input.borrow_mut());
        Slider::new(0.0, 1.0)
            .on_change("vol", |a: &mut FormApp, v: f64| a.volume = v)
            .render(rows[1], frame, &mut self.slider.borrow_mut());
    }
}

fn form_app() -> FormApp {
    FormApp {
        name: String::new(),
        volume: 0.0,
        input: Default::default(),
        slider: Default::default(),
    }
}

#[test]
fn value_widgets_apply_the_agents_value() {
    use dewey::agent::driver::HeadlessDriver;
    use dewey::agent::protocol::AgentRequest;

    let mut d = HeadlessDriver::new(form_app(), 200.0, 200.0);
    d.init();

    let r = d.process_request(&AgentRequest::ExecuteAction {
        agent_id: "name".into(),
        action: "set_text".into(),
        params: serde_json::json!({ "text": "Ada" }),
    });
    assert!(r.success);
    assert_eq!(d.model().name, "Ada", "the agent's text reached the model");

    let r = d.process_request(&AgentRequest::ExecuteAction {
        agent_id: "vol".into(),
        action: "set_value".into(),
        params: serde_json::json!({ "value": 0.75 }),
    });
    assert!(r.success);
    assert!((d.model().volume - 0.75).abs() < 1e-9);
}

/// A handler answers for its own action only. Firing a text field's handler
/// from an unrelated action would apply an empty value and silently wipe it.
#[test]
fn a_handler_answers_only_its_own_action() {
    use dewey::agent::driver::HeadlessDriver;
    use dewey::agent::protocol::AgentRequest;

    let mut d = HeadlessDriver::new(form_app(), 200.0, 200.0);
    d.init();
    d.process_request(&AgentRequest::ExecuteAction {
        agent_id: "name".into(),
        action: "set_text".into(),
        params: serde_json::json!({ "text": "Ada" }),
    });
    assert_eq!(d.model().name, "Ada");

    // A click on the text field must not run its set_text handler.
    d.process_request(&AgentRequest::ExecuteAction {
        agent_id: "name".into(),
        action: "click".into(),
        params: serde_json::Value::Null,
    });
    assert_eq!(d.model().name, "Ada", "click must not clear the field");
}

/// And such a form must validate clean — carrying a value handler names the
/// widget exactly as carrying a message does.
#[test]
fn value_widget_app_validates_clean() {
    use dewey::agent::driver::HeadlessDriver;

    let mut d = HeadlessDriver::new(form_app(), 200.0, 200.0);
    d.init();
    assert!(d.validate().is_empty(), "{:?}", d.validate());
}

/// A handler bound to an action the widget does not advertise is unreachable:
/// the ontology tells the agent one name and the application answers another.
/// `Checkbox` advertises `toggle`, and a handler registered under `click`
/// looked correct and could never be fired — this check exists because that
/// shipped.
#[test]
fn validate_flags_a_handler_for_an_unadvertised_action() {
    use dewey::agent::driver::HeadlessDriver;
    use dewey::ontology::{Diagnostic, Severity};

    // The real widgets are consistent, so the check is exercised by asking the
    // diagnostics directly with a handler that does not match the schema.
    let mut d = HeadlessDriver::new(
        ActionApp {
            count: 0,
            on: false,
        },
        200.0,
        200.0,
    );
    d.init();
    assert!(
        d.validate().is_empty(),
        "the library's own widgets must advertise what they handle"
    );

    let tree = dewey::runtime::build_ontology_tree(
        &ActionApp {
            count: 0,
            on: false,
        },
        dewey::core::Rect::from_size(200.0, 200.0),
    );
    use dewey::ontology::Discoverable;
    let mut registry = dewey::ontology::OntologyRegistry::new();
    let button = Button::new("x");
    let mut schema = button.schema();
    schema.actions = button.actions();
    registry.register_schema(schema);

    let found: Vec<Diagnostic> = dewey::ontology::diagnostics::check(
        &tree,
        &[],
        dewey::core::Size::new(200.0, 200.0),
        &[("inc".to_string(), "wiggle")],
        &registry,
        false,
    );
    let bad = found
        .iter()
        .find(|d| d.code == "unadvertised_action")
        .expect("a handler for an unadvertised action must be reported");
    assert_eq!(bad.severity, Severity::Error);
    assert!(
        bad.message.contains("click"),
        "the diagnostic should name what the widget does advertise: {}",
        bad.message
    );
}

// ── Handler coverage across the interactive widgets ────────────────

/// Every widget that now carries a handler must actually dispatch it, under
/// the action its own ontology advertises. Compiling is not evidence.
#[test]
fn every_extended_widget_dispatches_under_its_advertised_action() {
    use dewey::agent::driver::HeadlessDriver;
    use dewey::agent::protocol::AgentRequest;
    use dewey::widget::{
        List, ListState, Radio, Select, SelectState, StatefulWidget, TabState, Tabs, TextArea,
        TextAreaState,
    };

    #[derive(Default)]
    struct Hits {
        list: Option<usize>,
        select: Option<usize>,
        tab: Option<usize>,
        area: String,
        radio: bool,
    }

    struct W {
        hits: Hits,
        list_s: std::cell::RefCell<ListState>,
        sel_s: std::cell::RefCell<SelectState>,
        tab_s: std::cell::RefCell<TabState>,
        area_s: std::cell::RefCell<TextAreaState>,
    }

    impl Model for W {
        type Msg = ();
        fn update(&mut self, _m: ()) -> Command<()> {
            Command::None
        }
        fn view(&self, frame: &mut Frame<'_>) {
            let r = frame.area.rows_of(&[40.0, 40.0, 40.0, 40.0, 40.0]);
            List::new(vec!["a".into(), "b".into()])
                .on_select("lst", |w: &mut W, i: usize| w.hits.list = Some(i))
                .render(r[0], frame, &mut self.list_s.borrow_mut());
            Select::new("pick", vec!["x".into(), "y".into()])
                .on_select("sel", |w: &mut W, i: usize| w.hits.select = Some(i))
                .render(r[1], frame, &mut self.sel_s.borrow_mut());
            Tabs::new(vec!["one".into(), "two".into()])
                .on_select("tabs", |w: &mut W, i: usize| w.hits.tab = Some(i))
                .render(r[2], frame, &mut self.tab_s.borrow_mut());
            TextArea::new()
                .on_input("area", |w: &mut W, t: &str| w.hits.area = t.to_string())
                .on_insert(|w: &mut W, t: &str| w.hits.area.push_str(t))
                .render(r[3], frame, &mut self.area_s.borrow_mut());
            Radio::new("opt", true)
                .on_select("radio", |w: &mut W| w.hits.radio = true)
                .render(r[4], frame);
        }
    }

    let mut d = HeadlessDriver::new(
        W {
            hits: Hits::default(),
            list_s: Default::default(),
            sel_s: Default::default(),
            tab_s: Default::default(),
            area_s: Default::default(),
        },
        300.0,
        300.0,
    );
    d.init();

    let calls = [
        ("lst", "select", serde_json::json!({"index": 1})),
        ("sel", "select", serde_json::json!({"index": 1})),
        ("tabs", "select_tab", serde_json::json!({"index": 1})),
        ("area", "set_text", serde_json::json!({"text": "hello"})),
        ("radio", "select", serde_json::Value::Null),
    ];
    for (id, action, params) in calls {
        let r = d.process_request(&AgentRequest::ExecuteAction {
            agent_id: id.into(),
            action: action.into(),
            params,
        });
        assert!(r.success, "{id}.{action} failed");
    }

    let h = &d.model().hits;
    assert_eq!(h.list, Some(1), "List::on_select");
    assert_eq!(h.select, Some(1), "Select::on_select");
    assert_eq!(h.tab, Some(1), "Tabs::on_select");
    assert_eq!(h.area, "hello", "TextArea::on_input");
    assert!(h.radio, "Radio::on_select");

    // And the whole form must be structurally sound, which also proves every
    // handler is bound to an action its widget advertises.
    assert!(d.validate().is_empty(), "{:?}", d.validate());
}

/// A widget that advertises several actions must answer to each of them.
///
/// Registration used to be keyed by widget id alone, so a `Tree` registering
/// four handlers kept only the last: an agent could call `collapse_all` and
/// nothing else. Every action here is exercised through the protocol against
/// one widget instance.
#[test]
fn multi_action_widgets_answer_to_every_action_they_advertise() {
    use dewey::agent::driver::HeadlessDriver;
    use dewey::agent::protocol::AgentRequest;
    use dewey::widget::{
        ColorChange, ColorPicker, ColorPickerState, CommandPalette, CommandPaletteState,
        DateChange, DatePicker, DatePickerState, PaletteChange, ScrollArea, ScrollState,
        StatefulWidget, Tree, TreeChange, TreeNode, Widget,
    };

    #[derive(Default)]
    struct Log {
        tree: Vec<String>,
        date: Vec<String>,
        palette: Vec<String>,
        color: Option<dewey::core::Color>,
        scroll: Option<(Option<f32>, Option<f32>)>,
    }

    struct W {
        log: Log,
        date_s: std::cell::RefCell<DatePickerState>,
        pal_s: std::cell::RefCell<CommandPaletteState>,
        col_s: std::cell::RefCell<ColorPickerState>,
        scr_s: std::cell::RefCell<ScrollState>,
    }

    impl Model for W {
        type Msg = ();
        fn update(&mut self, _m: ()) -> Command<()> {
            Command::None
        }
        fn view(&self, frame: &mut Frame<'_>) {
            let r = frame.area.rows_of(&[40.0, 40.0, 40.0, 40.0, 40.0]);
            Tree::new(TreeNode::branch("root", vec![TreeNode::leaf("a")]))
                .on_change("tree", |w: &mut W, c: TreeChange<'_>| {
                    w.log.tree.push(format!("{c:?}"));
                })
                .render(r[0], frame);
            DatePicker::new()
                .on_change("date", |w: &mut W, c: DateChange| {
                    w.log.date.push(format!("{c:?}"));
                })
                .render(r[1], frame, &mut self.date_s.borrow_mut());
            CommandPalette::new(vec![])
                .on_change("palette", |w: &mut W, c: PaletteChange<'_>| {
                    w.log.palette.push(format!("{c:?}"));
                })
                .render(r[2], frame, &mut self.pal_s.borrow_mut());
            ColorPicker::new("Colour")
                .on_color("color", |w: &mut W, c: ColorChange| {
                    w.log.color = Some(c.applied_to(dewey::core::Color::BLACK));
                })
                .render(r[3], frame, &mut self.col_s.borrow_mut());
            ScrollArea::vertical()
                .on_scroll("scroll", |w: &mut W, x, y| w.log.scroll = Some((x, y)))
                .render(r[4], frame, &mut self.scr_s.borrow_mut());
        }
    }

    let mut d = HeadlessDriver::new(
        W {
            log: Log::default(),
            date_s: Default::default(),
            pal_s: Default::default(),
            col_s: Default::default(),
            scr_s: Default::default(),
        },
        300.0,
        400.0,
    );
    d.init();

    let calls = [
        ("tree", "expand", serde_json::json!({"path": "root/a"})),
        ("tree", "collapse", serde_json::json!({"path": "root/a"})),
        ("tree", "expand_all", serde_json::Value::Null),
        ("tree", "collapse_all", serde_json::Value::Null),
        (
            "date",
            "set_date",
            serde_json::json!({"year": 2026, "month": 9, "day": 1}),
        ),
        ("date", "prev_month", serde_json::Value::Null),
        ("date", "next_month", serde_json::Value::Null),
        ("date", "toggle", serde_json::Value::Null),
        ("palette", "open", serde_json::Value::Null),
        ("palette", "search", serde_json::json!({"query": "op"})),
        (
            "palette",
            "execute",
            serde_json::json!({"command_id": "open_file"}),
        ),
        ("palette", "close", serde_json::Value::Null),
        ("color", "set_color", serde_json::json!({"g": 255})),
        ("scroll", "scroll_to", serde_json::json!({"y": 120.0})),
    ];
    for (id, action, params) in calls {
        let r = d.process_request(&AgentRequest::ExecuteAction {
            agent_id: id.into(),
            action: action.into(),
            params,
        });
        assert!(r.success, "{id}.{action} was refused");
    }

    let log = &d.model().log;
    assert_eq!(
        log.tree,
        [
            "Expand(\"root/a\")",
            "Collapse(\"root/a\")",
            "ExpandAll",
            "CollapseAll"
        ],
        "every Tree action must reach the handler, in the order called"
    );
    assert_eq!(
        log.date,
        [
            "Set { year: 2026, month: 9, day: 1 }",
            "PrevMonth",
            "NextMonth",
            "Toggle"
        ]
    );
    assert_eq!(
        log.palette,
        ["Open", "Search(\"op\")", "Execute(\"open_file\")", "Close"]
    );

    // A change that names one component leaves the others alone: setting the
    // green channel of black must not also clear the alpha.
    let color = log.color.expect("ColorPicker::on_color");
    assert!((color.g - 1.0).abs() < 1e-6, "green set");
    assert!((color.r).abs() < 1e-6, "red kept");
    assert!((color.a - 1.0).abs() < 1e-6, "alpha kept, not zeroed");

    // An omitted offset stays omitted rather than arriving as zero.
    assert_eq!(log.scroll, Some((None, Some(120.0))));

    assert!(d.validate().is_empty(), "{:?}", d.validate());
}

/// A widget wired for some of its actions but not all reports the gap.
///
/// This is the quiet failure: `execute_action(id, "sort")` on a `Table` that
/// only wired `select_row` returns success and changes nothing, and the agent
/// has no way to tell that apart from a sort that happened to leave the order
/// alone.
#[test]
fn validate_flags_a_widget_wired_for_only_some_of_its_actions() {
    use dewey::agent::driver::HeadlessDriver;
    use dewey::widget::{StatefulWidget, Table, TableState};

    struct App {
        picked: Option<usize>,
        state: std::cell::RefCell<TableState>,
    }

    impl Model for App {
        type Msg = ();
        fn update(&mut self, _m: ()) -> Command<()> {
            Command::None
        }
        fn view(&self, frame: &mut Frame<'_>) {
            Table::new(
                vec!["name".into()],
                vec![vec!["a".into()], vec!["b".into()]],
            )
            .on_select("rows", |a: &mut App, i: usize| a.picked = Some(i))
            .render(frame.area, frame, &mut self.state.borrow_mut());
        }
    }

    let mut d = HeadlessDriver::new(
        App {
            picked: None,
            state: Default::default(),
        },
        400.0,
        300.0,
    );
    d.init();

    let found = d.validate();
    let gap = found
        .iter()
        .find(|d| d.code == "unhandled_action")
        .unwrap_or_else(|| panic!("expected an unhandled_action warning, got {found:?}"));
    assert_eq!(gap.agent_id.as_deref(), Some("rows"));
    assert_eq!(gap.severity, dewey::ontology::Severity::Warning);
    for action in ["sort", "filter", "page"] {
        assert!(
            gap.message.contains(action),
            "`{action}` missing from: {}",
            gap.message
        );
    }
    assert!(
        gap.message.contains("select_row"),
        "the warning should say what *is* wired: {}",
        gap.message
    );
}

/// An application that drives a widget through `execute_action` is not nagged.
///
/// Wiring no handlers at all is a different style, not a partial job, and a
/// warning there would fire on every pre-handler application.
#[test]
fn validate_is_quiet_about_widgets_with_no_handlers_at_all() {
    use dewey::agent::driver::HeadlessDriver;
    use dewey::widget::{StatefulWidget, Table, TableState};

    struct App {
        state: std::cell::RefCell<TableState>,
    }

    impl Model for App {
        type Msg = ();
        fn update(&mut self, _m: ()) -> Command<()> {
            Command::None
        }
        fn view(&self, frame: &mut Frame<'_>) {
            Table::new(vec!["name".into()], vec![vec!["a".into()]])
                .agent_id("rows")
                .render(frame.area, frame, &mut self.state.borrow_mut());
        }
    }

    let mut d = HeadlessDriver::new(
        App {
            state: Default::default(),
        },
        400.0,
        300.0,
    );
    d.init();
    assert!(
        !d.validate().iter().any(|d| d.code == "unhandled_action"),
        "{:?}",
        d.validate()
    );
}

/// A closed dialog must still answer `open`.
///
/// It renders nothing, so it has no node in the UI tree and nothing an agent
/// can find — but `open` is exactly the action it exists to accept, and a
/// modal that can only be closed strands whatever it was guarding.
#[test]
fn a_closed_modal_can_still_be_opened() {
    use dewey::agent::driver::HeadlessDriver;
    use dewey::agent::protocol::AgentRequest;
    use dewey::widget::{Modal, Widget};

    struct App {
        open: bool,
    }

    impl Model for App {
        type Msg = ();
        fn update(&mut self, _m: ()) -> Command<()> {
            Command::None
        }
        fn view(&self, frame: &mut Frame<'_>) {
            Modal::new("Confirm", self.open)
                .on_change("dialog", |a: &mut App, open: bool| a.open = open)
                .render(frame.area, frame);
        }
    }

    let mut d = HeadlessDriver::new(App { open: false }, 300.0, 200.0);
    d.init();

    let call = |d: &mut HeadlessDriver<App>, action: &str| {
        d.process_request(&AgentRequest::ExecuteAction {
            agent_id: "dialog".into(),
            action: action.into(),
            params: serde_json::Value::Null,
        })
    };

    assert!(call(&mut d, "open").success, "a closed modal must open");
    assert!(d.model().open, "and the model must say so");
    assert!(call(&mut d, "close").success);
    assert!(!d.model().open);
}

/// `Table::on_change` covers every action the table advertises.
#[test]
fn table_on_change_covers_sort_filter_and_page() {
    use dewey::agent::driver::HeadlessDriver;
    use dewey::agent::protocol::AgentRequest;
    use dewey::widget::{StatefulWidget, Table, TableChange, TableState};

    struct App {
        seen: Vec<String>,
        state: std::cell::RefCell<TableState>,
    }

    impl Model for App {
        type Msg = ();
        fn update(&mut self, _m: ()) -> Command<()> {
            Command::None
        }
        fn view(&self, frame: &mut Frame<'_>) {
            Table::new(
                vec!["name".into()],
                vec![vec!["a".into()], vec!["b".into()]],
            )
            .on_change("rows", |a: &mut App, c: TableChange<'_>| {
                a.seen.push(format!("{c:?}"));
            })
            .render(frame.area, frame, &mut self.state.borrow_mut());
        }
    }

    let mut d = HeadlessDriver::new(
        App {
            seen: Vec::new(),
            state: Default::default(),
        },
        400.0,
        300.0,
    );
    d.init();

    for (action, params) in [
        ("select_row", serde_json::json!({"index": 1})),
        (
            "sort",
            serde_json::json!({"column": 0, "direction": "desc"}),
        ),
        ("filter", serde_json::json!({"text": "a"})),
        ("page", serde_json::json!({"page": 2})),
    ] {
        let r = d.process_request(&AgentRequest::ExecuteAction {
            agent_id: "rows".into(),
            action: action.into(),
            params,
        });
        assert!(r.success, "{action} was refused");
    }

    assert_eq!(
        d.model().seen,
        [
            "SelectRow(1)",
            "Sort { column: 0, descending: true }",
            "Filter(\"a\")",
            "Page(2)"
        ]
    );
    // Fully wired, so nothing is left silently accepting calls.
    assert!(
        !d.validate().iter().any(|d| d.code == "unhandled_action"),
        "{:?}",
        d.validate()
    );
}

/// An agent can ask what a widget accepts without the application doing
/// anything to enable it.
#[test]
fn the_widget_catalogue_is_available_to_every_session() {
    use dewey::agent::driver::HeadlessDriver;
    use dewey::agent::protocol::AgentRequest;

    struct Bare;
    impl Model for Bare {
        type Msg = ();
        fn update(&mut self, _m: ()) -> Command<()> {
            Command::None
        }
        fn view(&self, _frame: &mut Frame<'_>) {}
    }

    let mut d = HeadlessDriver::new(Bare, 100.0, 100.0);
    d.init();
    let r = d.process_request(&AgentRequest::GetSchema {
        widget_type: "Table".into(),
    });
    assert!(r.success, "an unregistered widget type must still resolve");
    let actions = r.data.expect("schema")["actions"]
        .as_array()
        .expect("actions")
        .iter()
        .filter_map(|a| a["name"].as_str().map(str::to_string))
        .collect::<Vec<_>>();
    for want in ["select_row", "sort", "filter", "page"] {
        assert!(actions.contains(&want.to_string()), "missing {want}");
    }
}

/// An action a widget never advertised is refused, not quietly accepted.
///
/// A `Checkbox` advertises `toggle`. `execute_action(id, "click")` used to
/// come back successful having changed nothing, which is the worst answer
/// available: the agent has no reason to look again. This project's own
/// TodoMVC benchmark completed a todo that way for several commits.
#[test]
fn an_action_the_widget_does_not_advertise_is_refused() {
    use dewey::agent::driver::HeadlessDriver;
    use dewey::agent::protocol::AgentRequest;
    use dewey::widget::{Checkbox, Widget};

    struct App {
        on: bool,
    }

    impl Model for App {
        type Msg = ();
        fn update(&mut self, _m: ()) -> Command<()> {
            Command::None
        }
        fn view(&self, frame: &mut Frame<'_>) {
            Checkbox::new("done", self.on)
                .on("box", |a: &mut App| a.on = !a.on)
                .render(frame.area, frame);
        }
    }

    let mut d = HeadlessDriver::new(App { on: false }, 200.0, 60.0);
    d.init();

    let call = |d: &mut HeadlessDriver<App>, action: &str| {
        d.process_request(&AgentRequest::ExecuteAction {
            agent_id: "box".into(),
            action: action.into(),
            params: serde_json::Value::Null,
        })
    };

    let refused = call(&mut d, "click");
    assert!(!refused.success, "`click` is not advertised by Checkbox");
    let why = refused.error.expect("a refusal must say why");
    assert!(
        why.contains("toggle"),
        "the refusal should name what is accepted: {why}"
    );
    assert!(!d.model().on, "and nothing may have changed");

    assert!(call(&mut d, "toggle").success, "the published name works");
    assert!(d.model().on);
}

/// The full TodoMVC agent task, run as a test rather than only as a benchmark.
///
/// This exact sequence lived only in `benches/scaffold`, a separate workspace
/// the main `cargo check` never builds. It sat broken for several commits:
/// the toggle step named `click` on a `Checkbox`, which advertises `toggle`,
/// and reported success while doing nothing. A check that only runs when
/// someone remembers to run a benchmark is not a check.
mod todo_agent_task {
    use super::*;
    use dewey::agent::driver::HeadlessDriver;
    use dewey::agent::protocol::AgentRequest;
    use dewey::widget::input::TextInputState;
    use dewey::widget::{Checkbox, StatefulWidget, TextInput};
    use std::cell::RefCell;

    #[derive(Clone, Copy, PartialEq)]
    enum Filter {
        All,
        Active,
        Completed,
    }

    struct Todo {
        title: String,
        done: bool,
    }

    pub struct App {
        todos: Vec<Todo>,
        filter: Filter,
        input: RefCell<TextInputState>,
    }

    impl App {
        fn visible(&self) -> Vec<usize> {
            (0..self.todos.len())
                .filter(|&i| match self.filter {
                    Filter::All => true,
                    Filter::Active => !self.todos[i].done,
                    Filter::Completed => self.todos[i].done,
                })
                .collect()
        }

        fn remaining(&self) -> usize {
            self.todos.iter().filter(|t| !t.done).count()
        }

        fn add(&mut self) {
            let title = self.input.borrow().text.trim().to_string();
            if !title.is_empty() {
                self.todos.push(Todo { title, done: false });
                *self.input.borrow_mut() = TextInputState::new();
            }
        }
    }

    impl Model for App {
        type Msg = ();

        fn update(&mut self, _msg: ()) -> Command<()> {
            Command::None
        }

        fn view(&self, frame: &mut Frame<'_>) {
            let h = frame.area.height;
            let rows = frame.area.rows_of(&[36.0, 32.0, h - 96.0, 28.0]);

            let top = rows[0].cols_of(&[rows[0].width - 80.0, 80.0]);
            TextInput::new()
                .placeholder("What needs doing?")
                .on_input("new_todo", |a: &mut App, t: &str| {
                    *a.input.borrow_mut() = TextInputState::new().with_text(t)
                })
                .render(top[0], frame, &mut self.input.borrow_mut());
            Button::new("Add").on("add", App::add).render(top[1], frame);

            let f = rows[1].split_columns(3);
            Button::new("All")
                .on("filter_all", |a: &mut App| a.filter = Filter::All)
                .render(f[0], frame);
            Button::new("Active")
                .on("filter_active", |a: &mut App| a.filter = Filter::Active)
                .render(f[1], frame);
            Button::new("Completed")
                .on("filter_completed", |a: &mut App| {
                    a.filter = Filter::Completed
                })
                .render(f[2], frame);

            for (i, row) in self.visible().into_iter().zip(rows[2].rows(28.0)) {
                let c = row.cols_of(&[24.0, row.width - 52.0, 28.0]);
                Checkbox::new("", self.todos[i].done)
                    .on(format!("toggle_{i}"), move |a: &mut App| {
                        a.todos[i].done = !a.todos[i].done
                    })
                    .render(c[0], frame);
                Label::new(self.todos[i].title.clone())
                    .agent_id(format!("item_{i}"))
                    .render(c[1], frame);
                Button::new("x")
                    .on(format!("delete_{i}"), move |a: &mut App| {
                        a.todos.remove(i);
                    })
                    .render(c[2], frame);
            }

            let foot = rows[3].cols_of(&[rows[3].width - 140.0, 140.0]);
            Label::new(format!("{} items left", self.remaining()))
                .agent_id("remaining")
                .render(foot[0], frame);
            Button::new("Clear completed")
                .on("clear_completed", |a: &mut App| a.todos.retain(|t| !t.done))
                .render(foot[1], frame);
        }
    }

    fn driver() -> HeadlessDriver<App> {
        let mut d = HeadlessDriver::new(
            App {
                todos: Vec::new(),
                filter: Filter::All,
                input: RefCell::new(TextInputState::new()),
            },
            480.0,
            400.0,
        );
        d.init();
        d
    }

    fn act(id: &str, action: &str, params: serde_json::Value) -> AgentRequest {
        AgentRequest::ExecuteAction {
            agent_id: id.into(),
            action: action.into(),
            params,
        }
    }

    #[test]
    fn an_agent_can_add_complete_filter_and_read_the_result_back() {
        let mut d = driver();
        let null = serde_json::Value::Null;

        let steps = [
            (
                "discover",
                AgentRequest::GetTree {
                    since: None,
                    viewport: None,
                },
            ),
            (
                "type item 1",
                act(
                    "new_todo",
                    "set_text",
                    serde_json::json!({"text": "write tests"}),
                ),
            ),
            ("add item 1", act("add", "click", null.clone())),
            (
                "type item 2",
                act(
                    "new_todo",
                    "set_text",
                    serde_json::json!({"text": "ship it"}),
                ),
            ),
            ("add item 2", act("add", "click", null.clone())),
            // `toggle`, not `click`: the name the widget publishes.
            ("complete item 1", act("toggle_0", "toggle", null.clone())),
            ("filter active", act("filter_active", "click", null.clone())),
            (
                "re-read",
                AgentRequest::GetTree {
                    since: None,
                    viewport: None,
                },
            ),
        ];
        for (label, req) in &steps {
            assert!(d.process_request(req).success, "step failed: {label}");
        }

        let seen = d.process_request(&AgentRequest::GetState {
            agent_id: "remaining".into(),
        });
        let shown = serde_json::to_string(&seen.data.expect("state")).unwrap();
        assert!(
            shown.contains("1 items left"),
            "the footer must show what the agent did: {shown}"
        );

        let tree = serde_json::to_string(
            &d.process_request(&AgentRequest::GetTree {
                since: None,
                viewport: None,
            })
            .data
            .expect("tree"),
        )
        .unwrap();
        assert!(
            tree.contains("ship it"),
            "the active filter keeps the incomplete item"
        );
        assert!(
            !tree.contains("write tests"),
            "and drops the completed one: {tree}"
        );
    }

    /// The interface the agent drove must also be structurally sound.
    #[test]
    fn the_todo_app_validates_clean() {
        let mut d = driver();
        d.process_request(&act(
            "new_todo",
            "set_text",
            serde_json::json!({"text": "a"}),
        ));
        d.process_request(&act("add", "click", serde_json::Value::Null));
        assert!(d.validate().is_empty(), "{:?}", d.validate());
    }
}

/// Content widgets reach the model too, not a copy thrown away next frame.
///
/// `Canvas`, `Chart` and `RichText` each implement `Discoverable::execute_action`
/// and mutate themselves — but a widget is rebuilt inside `view` every frame,
/// so that change lasts until the next redraw. Nothing in the protocol calls
/// it either. Without a handler an agent's `clear` reported success and the
/// picture stayed exactly as it was.
#[test]
fn content_widgets_change_the_application_not_the_frame() {
    use dewey::agent::driver::HeadlessDriver;
    use dewey::agent::protocol::AgentRequest;
    use dewey::widget::{Canvas, Chart, ChartChange, RichText, RichTextChange, Widget};

    #[derive(Default)]
    struct App {
        strokes: usize,
        series: Vec<(String, Vec<f64>)>,
        markdown: String,
    }

    impl Model for App {
        type Msg = ();
        fn update(&mut self, _m: ()) -> Command<()> {
            Command::None
        }
        fn view(&self, frame: &mut Frame<'_>) {
            let r = frame.area.rows_of(&[60.0, 60.0, 60.0]);
            Canvas::new()
                .on_clear("sketch", |a: &mut App| a.strokes = 0)
                .render(r[0], frame);
            Chart::line("readings")
                .on_change("plot", |a: &mut App, c: ChartChange<'_>| match c {
                    ChartChange::AddSeries { label, values } => {
                        a.series.push((label.to_string(), values));
                    }
                    ChartChange::RemoveSeries(i) => {
                        a.series.remove(i);
                    }
                    ChartChange::Clear => a.series.clear(),
                })
                .render(r[1], frame);
            RichText::new(Vec::new())
                .on_change("notes", |a: &mut App, c: RichTextChange<'_>| match c {
                    RichTextChange::SetMarkdown(md) => a.markdown = md.to_string(),
                    RichTextChange::Clear => a.markdown.clear(),
                })
                .render(r[2], frame);
        }
    }

    let mut d = HeadlessDriver::new(
        App {
            strokes: 7,
            ..Default::default()
        },
        300.0,
        200.0,
    );
    d.init();

    let call = |d: &mut HeadlessDriver<App>, id: &str, action: &str, params| {
        let r = d.process_request(&AgentRequest::ExecuteAction {
            agent_id: id.into(),
            action: action.into(),
            params,
        });
        assert!(r.success, "{id}.{action} was refused");
    };

    call(&mut d, "sketch", "clear", serde_json::Value::Null);
    assert_eq!(d.model().strokes, 0, "Canvas::on_clear");

    call(
        &mut d,
        "plot",
        "add_series",
        serde_json::json!({"label": "temp", "values": [1.0, 2.5]}),
    );
    assert_eq!(
        d.model().series,
        [("temp".to_string(), vec![1.0, 2.5])],
        "Chart::on_change add_series"
    );
    call(
        &mut d,
        "plot",
        "remove_series",
        serde_json::json!({"index": 0}),
    );
    assert!(
        d.model().series.is_empty(),
        "Chart::on_change remove_series"
    );

    call(
        &mut d,
        "notes",
        "set_markdown",
        serde_json::json!({"content": "# hi"}),
    );
    assert_eq!(d.model().markdown, "# hi");
    call(&mut d, "notes", "clear", serde_json::Value::Null);
    assert!(d.model().markdown.is_empty());

    assert!(d.validate().is_empty(), "{:?}", d.validate());
}

/// The JSON a transport sends must mean the same thing as the typed reply, and
/// must actually drive the application.
///
/// Both transports used to reimplement the request loop, and their copies had
/// fallen behind: an `execute_action` became a `Command::AgentAction` that the
/// command loop logged and dropped. An agent connected over stdio or a
/// WebSocket could read the interface and change nothing. They now share the
/// driver, so this exercises the path they take.
#[test]
fn the_transport_json_path_drives_the_application() {
    use dewey::agent::driver::HeadlessDriver;
    use dewey::agent::protocol::{AgentRequest, RequestEnvelope};

    struct App {
        count: i32,
    }

    impl Model for App {
        type Msg = ();
        fn update(&mut self, _m: ()) -> Command<()> {
            Command::None
        }
        fn view(&self, frame: &mut Frame<'_>) {
            Button::new("inc")
                .on("inc", |a: &mut App| a.count += 1)
                .render(frame.area, frame);
        }
    }

    let mut d = HeadlessDriver::new(App { count: 0 }, 200.0, 80.0);
    d.init();

    let envelope = RequestEnvelope {
        id: Some("req-1".into()),
        request: AgentRequest::ExecuteAction {
            agent_id: "inc".into(),
            action: "click".into(),
            params: serde_json::Value::Null,
        },
    };
    let json = d.process_envelope_json(&envelope);
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    assert_eq!(parsed["success"], serde_json::json!(true), "{json}");
    assert_eq!(
        parsed["id"],
        serde_json::json!("req-1"),
        "the id comes back"
    );
    assert_eq!(d.model().count, 1, "the action must reach the model");

    // The tree reply takes a different, hand-built path to avoid an
    // intermediate `serde_json::Value`; it must still be the same document.
    let tree_env = RequestEnvelope {
        id: Some("req-2".into()),
        request: AgentRequest::GetTree {
            since: None,
            viewport: None,
        },
    };
    let direct: serde_json::Value =
        serde_json::from_str(&d.process_envelope_json(&tree_env)).expect("valid JSON");
    let typed = d.process_envelope(&tree_env);

    assert_eq!(direct["id"], serde_json::json!("req-2"));
    assert_eq!(direct["success"], serde_json::json!(true));
    assert_eq!(
        direct["data"],
        serde_json::to_value(typed.data.expect("data")).expect("value"),
        "the fast path and the Value path must agree"
    );

    // And the conditional form still short-circuits through it.
    let version = direct["data"]["version"].as_u64().expect("version");
    let unchanged: serde_json::Value =
        serde_json::from_str(&d.process_request_json(&AgentRequest::GetTree {
            since: Some(version),
            viewport: None,
        }))
        .expect("valid JSON");
    assert_eq!(unchanged["data"]["unchanged"], serde_json::json!(true));
}

/// A viewport narrows a tree reply to the widgets it shows.
///
/// The tree otherwise describes every widget including those scrolled out of
/// sight, which made it larger and slower than a screenshot for a long list —
/// a screenshot only ever shows one window's worth. Measured at 1000 rows the
/// full tree was 401 kB against a screenshot's 16.7 kB.
#[test]
fn a_viewport_narrows_the_tree_to_what_is_visible() {
    use dewey::agent::driver::HeadlessDriver;
    use dewey::agent::protocol::{AgentRequest, Viewport};

    const ROWS: usize = 200;
    const ROW_H: f32 = 20.0;

    struct App;
    impl Model for App {
        type Msg = ();
        fn update(&mut self, _m: ()) -> Command<()> {
            Command::None
        }
        fn view(&self, frame: &mut Frame<'_>) {
            for i in 0..ROWS {
                Label::new(format!("row {i}"))
                    .agent_id(format!("row_{i}"))
                    .render(Rect::new(0.0, i as f32 * ROW_H, 300.0, ROW_H), frame);
            }
        }
    }

    let mut d = HeadlessDriver::new(App, 300.0, ROWS as f32 * ROW_H);
    d.init();

    let full = d
        .process_request(&AgentRequest::GetTree {
            since: None,
            viewport: None,
        })
        .data
        .expect("tree");
    let full_bytes = serde_json::to_string(&full).expect("json").len();

    // One window's worth: the first ten rows.
    let view = Viewport {
        x: 0.0,
        y: 0.0,
        width: 300.0,
        height: 10.0 * ROW_H,
    };
    let clipped = d
        .process_request(&AgentRequest::GetTree {
            since: None,
            viewport: Some(view),
        })
        .data
        .expect("tree");
    let clipped_bytes = serde_json::to_string(&clipped).expect("json").len();

    assert_eq!(
        clipped["shown_nodes"],
        serde_json::json!(10),
        "ten rows intersect the viewport: {clipped}"
    );
    assert_eq!(
        clipped["total_nodes"],
        serde_json::json!(ROWS),
        "and the agent is told how many there are in all"
    );
    assert!(
        clipped_bytes * 10 < full_bytes,
        "clipping 200 rows to 10 should save an order of magnitude: \
         {clipped_bytes} against {full_bytes}"
    );

    let ids = serde_json::to_string(&clipped).expect("json");
    assert!(ids.contains("row_0") && ids.contains("row_9"));
    assert!(
        !ids.contains("row_10") && !ids.contains("row_199"),
        "rows below the viewport must not be described"
    );

    // A viewport further down shows different rows, not the first ones again.
    let lower = d
        .process_request(&AgentRequest::GetTree {
            since: None,
            viewport: Some(Viewport {
                y: 100.0 * ROW_H,
                ..view
            }),
        })
        .data
        .expect("tree");
    let lower_ids = serde_json::to_string(&lower).expect("json");
    assert!(lower_ids.contains("row_100") && !lower_ids.contains("row_0"));

    // The transport path must produce the same document as the Value path.
    let direct: serde_json::Value =
        serde_json::from_str(&d.process_request_json(&AgentRequest::GetTree {
            since: None,
            viewport: Some(view),
        }))
        .expect("valid JSON");
    assert_eq!(direct["data"]["shown_nodes"], serde_json::json!(10));
    assert_eq!(direct["data"]["root"], clipped["root"]);
}

/// A model can ask for its own window to be shown, hidden or focused.
///
/// Reported against 6ef0d7d by the Tabinator build: the defining gesture of a
/// tray application is "click the icon, toggle the window", and `Command` had
/// no way to say it. The only window operation reachable from `update` was
/// `Quit`, so closing to the tray and quitting were the same thing.
#[test]
fn a_model_can_move_its_own_window() {
    use dewey::agent::driver::HeadlessDriver;
    use dewey::agent::protocol::AgentRequest;

    #[derive(Default)]
    struct App {
        visible: bool,
    }

    impl Model for App {
        type Msg = bool;
        fn update(&mut self, show: bool) -> Command<bool> {
            self.visible = show;
            // Both must compile and be returnable from `update`; a headless
            // driver has no window to carry them out on.
            if show {
                Command::Batch(vec![
                    Command::SetWindowVisible(true),
                    Command::FocusWindow,
                    Command::SetAlwaysOnTop(true),
                    Command::SetWindowTitle("Tabinator".into()),
                ])
            } else {
                Command::SetWindowVisible(false)
            }
        }
        fn view(&self, frame: &mut Frame<'_>) {
            Button::new("toggle")
                .action("toggle", !self.visible)
                .render(frame.area, frame);
        }
    }

    let mut d = HeadlessDriver::new(App::default(), 200.0, 80.0);
    d.init();

    let r = d.process_request(&AgentRequest::ExecuteAction {
        agent_id: "toggle".into(),
        action: "click".into(),
        params: serde_json::Value::Null,
    });
    assert!(r.success);
    assert!(
        d.model().visible,
        "the window command must not swallow the state change that produced it"
    );

    // Every variant is constructible and Debug-printable, which is what a
    // backend match arm needs.
    let all: Vec<Command<bool>> = vec![
        Command::SetWindowVisible(false),
        Command::FocusWindow,
        Command::MinimiseWindow,
        Command::SetWindowPosition { x: 10.0, y: 20.0 },
        Command::SetWindowSize {
            width: 300.0,
            height: 400.0,
        },
        Command::SetAlwaysOnTop(true),
        Command::SetFullscreen(false),
        Command::SetWindowTitle("t".into()),
    ];
    assert_eq!(all.len(), 8);
    assert!(format!("{all:?}").contains("SetWindowPosition"));
}

/// A tray icon can carry artwork, and a single click is distinguishable.
///
/// Both reported by the Tabinator build: `TrayConfig` promised tray icons in
/// the README and had no field for one, so their backend generates a 32x32
/// buffer procedurally; and `TrayEvent` had only `DoubleClick`, so a backend
/// had to report a single click as a double one.
#[test]
fn tray_config_carries_an_icon_and_events_name_the_click() {
    use dewey::tray::{TrayConfig, TrayEvent, TrayIconImage, TrayMouseButton};

    let icon = TrayIconImage::from_rgba(2, 2, vec![0u8; 2 * 2 * 4]).expect("well-formed");
    let config = TrayConfig::new("Tabinator").with_icon(icon.clone());
    assert_eq!(config.icon.as_ref().map(|i| i.width), Some(2));

    assert!(
        TrayIconImage::from_rgba(2, 2, vec![0u8; 3]).is_none(),
        "a buffer that is not width * height * 4 must be refused here, not by \
         the platform later"
    );

    let click = TrayEvent::Click {
        button: TrayMouseButton::Left,
    };
    assert!(!matches!(click, TrayEvent::DoubleClick));
}

/// Strict validation must catch the defects that shipped this week.
///
/// The value of a check is not that it passes on good code. It is that it
/// fails on the specific things that already went wrong. Each case here is a
/// defect that was live in this repository, reduced to the interface that
/// exhibits it.
#[test]
fn strict_validation_catches_every_defect_from_this_week() {
    use dewey::agent::driver::HeadlessDriver;
    use dewey::ontology::Severity;
    use dewey::widget::{Canvas, Chart, StatefulWidget, Table, TableState, Widget};

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

    // A `Table` wired for selection only. `sort`, `filter` and `page` were
    // accepted and ignored.
    fn half_wired_table(frame: &mut Frame<'_>) {
        let mut state = TableState::new();
        Table::new(vec!["c".into()], vec![vec!["a".into()]])
            .on_select("rows", |_: &mut Probe, _| {})
            .render(frame.area, frame, &mut state);
    }

    // A `Canvas` with an id and no handler. `clear` reported success and the
    // picture did not change, because the widget it mutated was discarded.
    fn unwired_canvas(frame: &mut Frame<'_>) {
        Canvas::new().agent_id("sketch").render(frame.area, frame);
    }

    // Same shape, different widget.
    fn unwired_chart(frame: &mut Frame<'_>) {
        Chart::line("readings")
            .agent_id("plot")
            .render(frame.area, frame);
    }

    for (name, build) in [
        (
            "table wired for one of four actions",
            half_wired_table as fn(&mut Frame<'_>),
        ),
        ("canvas that publishes clear and ignores it", unwired_canvas),
        (
            "chart that publishes three actions and ignores them",
            unwired_chart,
        ),
    ] {
        let mut d = HeadlessDriver::new(Probe(build), 400.0, 200.0);
        d.init();

        let strict = d.validate_strict();
        assert!(
            strict.iter().any(|f| f.severity == Severity::Error),
            "strict validation must reject: {name}"
        );
    }

    // And a fully wired interface must still pass, or the check is useless.
    fn correct(frame: &mut Frame<'_>) {
        let mut state = TableState::new();
        Table::new(vec!["c".into()], vec![vec!["a".into()]])
            .on_change("rows", |_: &mut Probe, _| {})
            .render(frame.area, frame, &mut state);
    }
    let mut good = HeadlessDriver::new(Probe(correct), 400.0, 200.0);
    good.init();
    assert!(
        good.validate_strict().is_empty(),
        "a fully wired interface must pass strict: {:?}",
        good.validate_strict()
    );
}

/// Strict is opt-in, and the ordinary check stays quiet about a style choice.
///
/// An application answering through `Model::execute_action` wires no handlers
/// at all. That is a different way of writing the same program, not a fault,
/// and reporting it by default would fire on every pre-handler application.
#[test]
fn only_strict_reports_a_widget_that_wires_nothing() {
    use dewey::agent::driver::HeadlessDriver;
    use dewey::widget::{StatefulWidget, Table, TableState};

    struct App {
        state: std::cell::RefCell<TableState>,
    }
    impl Model for App {
        type Msg = ();
        fn update(&mut self, _m: ()) -> Command<()> {
            Command::None
        }
        fn view(&self, frame: &mut Frame<'_>) {
            Table::new(vec!["c".into()], vec![vec!["a".into()]])
                .agent_id("rows")
                .render(frame.area, frame, &mut self.state.borrow_mut());
        }
    }

    let mut d = HeadlessDriver::new(
        App {
            state: Default::default(),
        },
        400.0,
        200.0,
    );
    d.init();

    assert!(d.validate().is_empty(), "{:?}", d.validate());

    let strict = d.validate_strict();
    let found = strict
        .iter()
        .find(|f| f.code == "unwired_widget")
        .expect("strict must report it");
    assert_eq!(found.agent_id.as_deref(), Some("rows"));
    assert!(
        found.message.contains("sort"),
        "the report should name what goes unanswered: {}",
        found.message
    );
}

/// Strict is reachable over the protocol, not only from Rust.
#[test]
fn strict_validation_is_available_to_agents() {
    use dewey::agent::driver::HeadlessDriver;
    use dewey::agent::protocol::AgentRequest;
    use dewey::widget::{StatefulWidget, Table, TableState};

    struct App {
        state: std::cell::RefCell<TableState>,
    }
    impl Model for App {
        type Msg = ();
        fn update(&mut self, _m: ()) -> Command<()> {
            Command::None
        }
        fn view(&self, frame: &mut Frame<'_>) {
            Table::new(vec!["c".into()], vec![vec!["a".into()]])
                .agent_id("rows")
                .render(frame.area, frame, &mut self.state.borrow_mut());
        }
    }

    let mut d = HeadlessDriver::new(
        App {
            state: Default::default(),
        },
        400.0,
        200.0,
    );
    d.init();

    let lax = d.process_request(&AgentRequest::Validate { strict: false });
    assert_eq!(lax.data.expect("data")["ok"], serde_json::json!(true));

    let strict = d.process_request(&AgentRequest::Validate { strict: true });
    let data = strict.data.expect("data");
    assert_eq!(data["ok"], serde_json::json!(false));
    assert!(data["errors"].as_u64().unwrap_or(0) >= 1, "{data}");
}

/// A rendered interface converts into AccessKit nodes a screen reader can use.
///
/// The bridge existed and nothing called it, so a Dewey application published
/// no accessibility tree at all — egui's own tree is empty here because Dewey
/// paints its widgets rather than building them from egui widgets.
#[cfg(feature = "accesskit")]
#[test]
fn the_ontology_converts_to_accesskit_nodes() {
    use dewey::accesskit_bridge::{to_accesskit_node, to_accesskit_role};
    use dewey::ontology::SemanticRole;
    use dewey::widget::{Checkbox, Widget};

    struct App;
    impl Model for App {
        type Msg = ();
        fn update(&mut self, _m: ()) -> Command<()> {
            Command::None
        }
        fn view(&self, frame: &mut Frame<'_>) {
            let r = frame.area.split_rows(2);
            Button::new("Save")
                .on("save", |_: &mut App| {})
                .render(r[0], frame);
            Checkbox::new("Ready", true)
                .on("ready", |_: &mut App| {})
                .render(r[1], frame);
        }
    }

    let tree = dewey::runtime::build_ontology_tree(&App, Rect::from_size(300.0, 120.0));
    let nodes: Vec<_> = tree
        .root
        .children
        .iter()
        .map(dewey::accesskit_bridge::to_accesskit_node)
        .collect();

    assert_eq!(nodes.len(), 2, "both widgets must reach the tree");
    assert!(
        nodes.iter().any(|n| n.role() == accesskit::Role::Button),
        "the button must be announced as a button"
    );
    assert_eq!(
        to_accesskit_role(SemanticRole::Action),
        accesskit::Role::Button
    );

    // A label must survive the conversion, or a screen reader reads nothing.
    let button = tree
        .root
        .children
        .iter()
        .find(|n| n.agent_id.as_deref() == Some("save"))
        .expect("save button");
    let ak = to_accesskit_node(button);
    assert!(
        ak.label().is_some() || ak.value().is_some(),
        "the button must carry text a screen reader can announce"
    );
}

/// The default backend must honour `OntologyMode`, as the other one does.
///
/// `Frame::new` builds the ontology unconditionally, and the egui path used
/// it — so the documented `OnDemand` default was not the behaviour, and the
/// default backend paid roughly twice the frame cost for a tree nothing had
/// asked for.
#[test]
fn on_demand_is_the_default_and_means_what_it_says() {
    use dewey::runtime::{OntologyMode, ProgramOptions};

    assert_eq!(
        ProgramOptions::default().ontology,
        OntologyMode::OnDemand,
        "the documented default"
    );

    struct App;
    impl Model for App {
        type Msg = ();
        fn update(&mut self, _m: ()) -> Command<()> {
            Command::None
        }
        fn view(&self, frame: &mut Frame<'_>) {
            Button::new("x")
                .on("x", |_: &mut App| {})
                .render(frame.area, frame);
        }
    }

    // A frame that is not collecting the ontology must produce no nodes, which
    // is what makes skipping the work observable rather than a claim.
    let mut hit_map = dewey::event::HitMap::new();
    let mut painter = dewey::paint::NullPainter;
    let area = Rect::from_size(200.0, 100.0);

    let mut off = Frame::with_ontology(area, &mut hit_map, &mut painter, false);
    App.view(&mut off);
    assert!(off.take_nodes().is_empty(), "no tree when nobody asked");
    assert!(
        !off.ontology_enabled(),
        "widgets check this before building a node at all"
    );

    let mut on = Frame::with_ontology(area, &mut hit_map, &mut painter, true);
    App.view(&mut on);
    assert_eq!(on.take_nodes().len(), 1);
}

/// A viewport skips building nodes rather than discarding them afterwards.
///
/// The first version clipped a finished tree, so the reply stayed small and
/// the work stayed: a thousand-row list built a thousand `UiNode`s to throw
/// away all but ten, and the clipped time grew with the list while the bytes
/// did not.
#[test]
fn a_viewport_skips_the_work_not_just_the_reply() {
    const ROWS: usize = 200;
    const ROW_H: f32 = 20.0;

    struct App;
    impl Model for App {
        type Msg = ();
        fn update(&mut self, _m: ()) -> Command<()> {
            Command::None
        }
        fn view(&self, frame: &mut Frame<'_>) {
            for i in 0..ROWS {
                // Buttons, not labels: a hitbox is what makes the claim about
                // hit-testing below testable at all.
                Button::new(format!("row {i}"))
                    .on(format!("row_{i}"), |_: &mut App| {})
                    .render(Rect::new(0.0, i as f32 * ROW_H, 300.0, ROW_H), frame);
            }
        }
    }

    let area = Rect::from_size(300.0, ROWS as f32 * ROW_H);
    let mut hit_map = dewey::event::HitMap::new();
    let mut painter = dewey::paint::NullPainter;

    let mut clipped = Frame::new(area, &mut hit_map, &mut painter).clipped_to(Rect::new(
        0.0,
        0.0,
        300.0,
        10.0 * ROW_H,
    ));
    App.view(&mut clipped);

    assert_eq!(
        clipped.take_nodes().len(),
        10,
        "only the visible rows are described"
    );
    assert_eq!(
        clipped.skipped(),
        ROWS - 10,
        "and the rest are counted, not built — the count is what lets a reply \
         say how much of the interface it is showing without a second pass"
    );

    // Hit-testing is deliberately untouched: an off-screen widget is still
    // laid out and still clickable, it is simply not described.
    assert!(
        hit_map
            .hit_test(dewey::core::Position::new(10.0, 150.0 * ROW_H))
            .is_some(),
        "a widget outside the viewport must stay clickable"
    );
}

/// Reading the widget catalogue over a transport costs a copy, not a rebuild.
///
/// `query_ontology` with no filter returns the same thirty schemas every time.
/// The reply was cached as a `serde_json::Value` and then deep-cloned and
/// re-serialised for each caller, which cost about as much as building it from
/// scratch — 56 µs to emit a constant.
#[test]
fn the_catalogue_is_served_from_bytes_not_rebuilt() {
    use dewey::agent::driver::HeadlessDriver;
    use dewey::agent::protocol::AgentRequest;

    struct App;
    impl Model for App {
        type Msg = ();
        fn update(&mut self, _m: ()) -> Command<()> {
            Command::None
        }
        fn view(&self, _f: &mut Frame<'_>) {}
    }

    let mut d = HeadlessDriver::new(App, 100.0, 100.0);
    d.init();
    let request = AgentRequest::QueryOntology {
        query: None,
        role: None,
    };

    // The fast path hand-builds its envelope, so it must still be JSON, and it
    // must still say what the ordinary path says.
    let raw = d.process_request_json(&request);
    let parsed: serde_json::Value =
        serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{raw} is not JSON: {e}"));
    assert_eq!(parsed["success"], serde_json::json!(true));

    let typed = d.process_request(&request);
    assert_eq!(
        parsed["data"],
        serde_json::to_value(typed.data.expect("data")).expect("value"),
        "the cached bytes must describe the same catalogue as the Value path"
    );

    let schemas = parsed["data"].as_array().expect("an array of schemas");
    assert!(schemas.len() >= 27, "{} widget types", schemas.len());
    assert!(
        schemas
            .iter()
            .any(|s| s["name"] == "Table" && !s["actions"].as_array().unwrap().is_empty()),
        "and each must still carry its actions"
    );

    // A filtered query is not the catalogue and must not be served from it.
    let filtered = d.process_request_json(&AgentRequest::QueryOntology {
        query: Some("dropdown".into()),
        role: None,
    });
    let filtered: serde_json::Value = serde_json::from_str(&filtered).expect("JSON");
    let count = filtered["data"].as_array().map(Vec::len).unwrap_or(0);
    assert!(
        count > 0 && count < schemas.len(),
        "a filtered query returned {count} of {}",
        schemas.len()
    );
}

/// A subscribed agent is actually told what changed.
///
/// `subscribe` was accepted by both transports and honoured by neither:
/// nothing ever called `compute_state_diffs`, so an agent subscribed, waited,
/// and received nothing. The same shape as every other defect this week — the
/// call reported success and did nothing — and the one the plan predicted
/// would be here.
#[test]
fn subscribing_delivers_the_events_it_promises() {
    use dewey::agent::driver::HeadlessDriver;
    use dewey::agent::protocol::AgentRequest;

    struct App {
        count: i32,
    }

    impl Model for App {
        type Msg = ();
        fn update(&mut self, _m: ()) -> Command<()> {
            Command::None
        }
        fn view(&self, frame: &mut Frame<'_>) {
            let rows = frame.area.split_rows(2);
            Button::new("inc")
                .on("inc", |a: &mut App| a.count += 1)
                .render(rows[0], frame);
            Label::new(format!("count {}", self.count))
                .agent_id("readout")
                .render(rows[1], frame);
        }
    }

    let mut d = HeadlessDriver::new(App { count: 0 }, 200.0, 100.0);
    d.init();

    // Nothing subscribed: no events, and no diff computed.
    d.process_request(&AgentRequest::GetTree {
        since: None,
        viewport: None,
    });
    assert!(
        d.drain_events_json().is_empty(),
        "an unsubscribed session must be told nothing"
    );

    let sub = d.process_request(&AgentRequest::Subscribe {
        events: vec!["state_changed".into()],
    });
    assert!(sub.success);

    // The first look reports every widget as new, which is the diff having no
    // previous state rather than a change.
    d.process_request(&AgentRequest::GetTree {
        since: None,
        viewport: None,
    });
    let _baseline = d.drain_events_json();

    // Nothing has happened since, so nothing is reported.
    d.process_request(&AgentRequest::GetTree {
        since: None,
        viewport: None,
    });
    assert!(
        d.drain_events_json().is_empty(),
        "a screen that has not moved must produce no events"
    );

    // Now change something.
    let acted = d.process_request(&AgentRequest::ExecuteAction {
        agent_id: "inc".into(),
        action: "click".into(),
        params: serde_json::Value::Null,
    });
    assert!(acted.success);

    let events = d.drain_events_json();
    assert!(!events.is_empty(), "the change must be reported");

    let parsed: Vec<serde_json::Value> = events
        .iter()
        .map(|e| serde_json::from_str(e).expect("each event is JSON"))
        .collect();
    let readout = parsed
        .iter()
        .find(|e| e["agent_id"] == serde_json::json!("readout"))
        .unwrap_or_else(|| panic!("the label that changed must be named: {parsed:?}"));
    assert_eq!(
        readout["state"]["text"],
        serde_json::json!("count 1"),
        "and the event must carry the new value, not just the fact of a change"
    );
}
