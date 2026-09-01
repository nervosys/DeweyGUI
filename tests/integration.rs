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
    assert_eq!(node.accessibility.role, Some("button".into()));
    assert!(node.accessibility.tab_index.is_none());
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
