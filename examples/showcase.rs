//! Widget Showcase — demonstrates every Dewey widget in a single window.

use dewey::prelude::*;
use std::cell::RefCell;

struct App {
    select_state: RefCell<dewey::widget::select::SelectState>,
    list_state: RefCell<dewey::widget::list::ListState>,
    checkbox_checked: bool,
    radio_selected: usize,
    show_modal: bool,
    select_open: bool,
    canvas_cleared: bool,
    expanded: std::collections::HashMap<String, bool>,
}

impl App {
    fn new() -> Self {
        Self {
            select_state: RefCell::new(dewey::widget::select::SelectState::new()),
            list_state: RefCell::new(dewey::widget::list::ListState::new()),
            checkbox_checked: false,
            radio_selected: 0,
            select_open: false,
            canvas_cleared: false,
            expanded: std::collections::HashMap::new(),
            show_modal: false,
        }
    }
}

#[derive(Debug)]
enum Msg {
    ToggleCheckbox,
    SelectRadio(usize),
    ToggleModal,
}

impl Model for App {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) -> Command<Msg> {
        match msg {
            Msg::ToggleCheckbox => self.checkbox_checked = !self.checkbox_checked,
            Msg::SelectRadio(idx) => self.radio_selected = idx,
            Msg::ToggleModal => self.show_modal = !self.show_modal,
        }
        Command::None
    }

    fn view(&self, frame: &mut Frame<'_>) {
        let area = frame.area;

        // Top-level vertical split: tabs, then content
        let chunks = Layout::new(
            Direction::Vertical,
            [Constraint::Length(30.0), Constraint::Fill(1.0)],
        )
        .split(area);

        // Header label
        Label::new("Widget Showcase")
            .agent_id("title_label")
            .render(chunks[0], frame);

        // Content: two columns
        let cols = Layout::new(
            Direction::Horizontal,
            [Constraint::Percentage(50.0), Constraint::Percentage(50.0)],
        )
        .split(chunks[1]);

        // Left column: input widgets
        let left_rows = Layout::new(
            Direction::Vertical,
            [
                Constraint::Length(25.0), // button
                Constraint::Length(25.0), // checkbox
                Constraint::Length(25.0), // radio
                Constraint::Length(25.0), // progress
                Constraint::Length(25.0), // tooltip
                Constraint::Fill(1.0),    // spacer
            ],
        )
        .split(cols[0]);

        // Opens the modal, which no pointer could reach before: it was bound
        // to a key press alone.
        Button::new("Click Me!")
            .action("demo_btn", Msg::ToggleModal)
            .render(left_rows[0], frame);

        // Every widget here is wired. A showcase is what an agent reads to
        // learn what this framework's widgets do, and six of these advertised
        // actions with nothing behind them: calling one was reported as
        // success and changed nothing.
        //
        // That sentence was written while `demo_btn` above was still one of
        // them. `Button`'s click was declared non-mutating, and the strict
        // check that finds unwired controls only considers mutating actions,
        // so the commonest control in any interface was the one it could not
        // see. Seven more dead buttons across three other examples came out
        // with it when the flag was corrected.
        Checkbox::new("Enable feature", self.checkbox_checked)
            .action("demo_checkbox", Msg::ToggleCheckbox)
            .render(left_rows[1], frame);

        Radio::new("Option A", self.radio_selected == 0)
            .on_select("radio_a", |a: &mut App| a.radio_selected = 0)
            .render(left_rows[2], frame);

        ProgressBar::new(0.65)
            .agent_id("demo_progress")
            .render(left_rows[3], frame);

        Tooltip::new("Hover me", "I am a tooltip with extra information")
            .agent_id("demo_tooltip")
            .render(left_rows[4], frame);

        // Right column: data widgets
        let right_rows = Layout::new(
            Direction::Vertical,
            [
                Constraint::Length(25.0),  // select
                Constraint::Length(100.0), // list
                Constraint::Length(100.0), // tree
                Constraint::Fill(1.0),     // canvas
            ],
        )
        .split(cols[1]);

        Select::new(
            "Fruit",
            vec![
                "Apple".into(),
                "Banana".into(),
                "Cherry".into(),
                "Date".into(),
            ],
        )
        .on_select("demo_select", |a: &mut App, i| {
            a.select_state.borrow_mut().selected = i;
        })
        .on_open("demo_select", |a: &mut App, open| a.select_open = open)
        .render(right_rows[0], frame, &mut self.select_state.borrow_mut());

        List::new(vec![
            "Item 1".into(),
            "Item 2".into(),
            "Item 3".into(),
            "Item 4".into(),
            "Item 5".into(),
        ])
        .on_select("demo_list", |a: &mut App, i| {
            a.list_state.borrow_mut().selected = Some(i);
        })
        .render(right_rows[1], frame, &mut self.list_state.borrow_mut());

        // Tree
        Tree::new(TreeNode::branch(
            "Project",
            vec![
                TreeNode::branch(
                    "src",
                    vec![TreeNode::leaf("main.rs"), TreeNode::leaf("lib.rs")],
                ),
                TreeNode::branch("tests", vec![TreeNode::leaf("integration.rs")]),
                TreeNode::leaf("Cargo.toml"),
            ],
        ))
        .on_change("demo_tree", |a: &mut App, change| {
            use dewey::widget::tree::TreeChange;
            let expanded = matches!(change, TreeChange::Expand(_) | TreeChange::ExpandAll);
            if let TreeChange::Expand(path) | TreeChange::Collapse(path) = change {
                a.expanded.insert(path.to_string(), expanded);
            }
        })
        .render(right_rows[2], frame);

        // Canvas with some drawing commands
        Canvas::new()
            .on_clear("demo_canvas", |a: &mut App| a.canvas_cleared = true)
            .background([32, 32, 32, 255])
            .filled_rect(5.0, 5.0, 40.0, 40.0, [200, 50, 50, 255])
            .filled_circle(70.0, 25.0, 15.0, [50, 200, 50, 255])
            .line(5.0, 50.0, 90.0, 50.0, [50, 50, 200, 255], 2.0)
            .text(5.0, 55.0, "Canvas", 12.0, [255, 255, 255, 255])
            .render(right_rows[3], frame);

        // Modal (toggleable)
        if self.show_modal {
            Modal::new("Information", true)
                .agent_id("demo_modal")
                .body("This is a modal dialog.")
                .body("Press Escape or click outside to close.")
                .width(300.0)
                .render(area, frame);
        }
    }

    fn handle_event(&self, event: Event) -> Option<Msg> {
        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Char('c'),
                ..
            }) => Some(Msg::ToggleCheckbox),
            Event::Key(KeyEvent {
                code: KeyCode::Char('m'),
                ..
            }) => Some(Msg::ToggleModal),
            Event::Key(KeyEvent {
                code: KeyCode::Char('1'),
                ..
            }) => Some(Msg::SelectRadio(0)),
            Event::Key(KeyEvent {
                code: KeyCode::Char('2'),
                ..
            }) => Some(Msg::SelectRadio(1)),
            _ => None,
        }
    }

    fn register_ontology(&self, registry: &mut OntologyRegistry) {
        registry.register_schema(WidgetSchema::new(
            "WidgetShowcase",
            "Showcases all Dewey widgets",
            SemanticRole::Container,
        ));
    }

    fn title(&self) -> &str {
        "Dewey — Widget Showcase"
    }
}

fn main() -> std::result::Result<(), eframe::Error> {
    env_logger::init();
    Program::new(App::new())
        .with_options(ProgramOptions {
            width: 900.0,
            height: 700.0,
            ..Default::default()
        })
        .run()
}
