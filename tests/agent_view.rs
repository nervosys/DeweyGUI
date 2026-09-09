//! The ontology, written into the source comments an agent reads.
//!
//! Twelve paid runs in `benches/agentic/` showed what a model reaches for when
//! it meets this crate: `examples/counter.rs` twelve times, `agent_headless.rs`
//! nine, `llms.txt` seven — and `src/` three times in twelve runs. It does not
//! query the ontology while writing code, and telling it to did not change
//! that. So the ontology has to be where it is already looking, which means
//! prose, in the files themselves.
//!
//! Each widget's doc comment therefore carries an **Agent view** block: the
//! role it publishes, the actions it accepts, and the state fields it exposes.
//! That is the same information `get_schema` returns, in the place a reader
//! finds it for free.
//!
//! Written down twice, it can disagree with itself, and a comment that lies
//! about an interface is worse than no comment. So this test builds every
//! built-in widget, asks it the same questions the protocol asks, renders the
//! block those answers imply, and compares it to what the file says. Drift
//! fails the build.
//!
//! Regenerating after a deliberate change:
//!
//! ```text
//! DEWEY_AGENT_VIEW=print cargo test --test agent_view -- --nocapture
//! ```

use dewey::ontology::Discoverable;
use dewey::widget::*;
use std::path::Path;

/// The block a widget's own answers imply, exactly as it should appear.
fn block_for(w: &dyn Discoverable, companion: Option<&str>) -> String {
    let mut actions: Vec<String> = w
        .actions()
        .iter()
        .map(|a| {
            if a.params.is_empty() {
                format!("`{}`", a.name)
            } else {
                let params: Vec<&str> = a.params.iter().map(|p| p.name.as_str()).collect();
                format!("`{}`({})", a.name, params.join(", "))
            }
        })
        .collect();
    actions.sort();
    let mut caps: Vec<String> = w
        .capabilities()
        .iter()
        // The variant, not this dummy instance's payload. `{c:?}` on a
        // `Toggleable { state: false }` would put one throwaway instance's
        // value into a doc describing the type.
        .map(|c| {
            let full = format!("{c:?}");
            let name = full
                .split_once([' ', '('])
                .map_or(full.as_str(), |(head, _)| head);
            format!("`{name}`")
        })
        .collect();
    caps.sort();
    let mut state: Vec<String> = match w.agent_state() {
        serde_json::Value::Object(map) => map.keys().map(|k| format!("`{k}`")).collect(),
        _ => vec![],
    };
    state.sort();

    let say = |v: &[String]| {
        if v.is_empty() {
            "none".to_string()
        } else {
            v.join(", ")
        }
    };

    // A widget with a companion `...State` type is rebuilt every frame and does
    // not own what it displays; the application does. Saying so is the
    // difference between a true list and a misleading one.
    let note = match companion {
        Some(ty) => {
            format!("\n/// - live value: held by the application in `{ty}`, not by the widget")
        }
        None => String::new(),
    };
    format!(
        "/// # Agent view\n\
         ///\n\
         /// What an agent sees of this widget, and what it can call on it.\n\
         /// Generated from the widget's own answers; `tests/agent_view.rs`\n\
         /// fails the build if this block and the code disagree.\n\
         ///\n\
         /// - role: `{:?}`\n\
         /// - actions: {}\n\
         /// - state it publishes: {}\n\
         /// - capabilities: {}{}",
        w.semantic_role(),
        say(&actions),
        say(&state),
        say(&caps),
        note,
    )
}

/// Every built-in widget, built the way `ontology::builtin` builds them.
fn every_widget() -> Vec<(&'static str, Box<dyn Discoverable>)> {
    vec![
        ("button", Box::new(Button::new("x"))),
        ("checkbox", Box::new(Checkbox::new("x", false))),
        ("input", Box::new(TextInput::new())),
        ("text_area", Box::new(TextArea::new())),
        ("slider", Box::new(Slider::new(0.0, 1.0))),
        ("list", Box::new(List::new(Vec::new()))),
        ("select", Box::new(Select::new("x", Vec::new()))),
        ("radio", Box::new(Radio::new("x", false))),
        ("tabs", Box::new(Tabs::new(Vec::new()))),
        ("table", Box::new(Table::new(Vec::new(), Vec::new()))),
        ("tree", Box::new(Tree::new(TreeNode::leaf("x")))),
        ("menu", Box::new(Menu::new("x", Vec::new()))),
        ("toolbar", Box::new(Toolbar::new(Vec::new()))),
        ("modal", Box::new(Modal::new("x", false))),
        ("date_picker", Box::new(DatePicker::new())),
        ("color_picker", Box::new(ColorPicker::new("x"))),
        ("command_palette", Box::new(CommandPalette::new(Vec::new()))),
        ("scroll", Box::new(ScrollArea::vertical())),
        (
            "splitter",
            Box::new(Splitter::new(SplitDirection::Vertical)),
        ),
        ("label", Box::new(Label::new("x"))),
        (
            "panel",
            Box::new(Panel::new(dewey::widget::panel::PanelSide::Left)),
        ),
        ("container", Box::new(Container::new())),
        ("progress", Box::new(ProgressBar::new(0.0))),
        ("tooltip", Box::new(Tooltip::new("x", "x"))),
        ("rich_text", Box::new(RichText::new(Vec::new()))),
        ("chart", Box::new(Chart::line("x"))),
        ("image", Box::new(Image::from_rgba(1, 1, vec![0, 0, 0, 0]))),
        ("canvas", Box::new(Canvas::new())),
        (
            "virtual_list",
            Box::new(VirtualList::new(
                1.0,
                |_i, _r, _f: &mut dewey::runtime::Frame<'_>| {},
            )),
        ),
    ]
}

#[test]
fn every_widget_documents_what_an_agent_can_do_with_it() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let printing = std::env::var("DEWEY_AGENT_VIEW").as_deref() == Ok("print");
    let mut missing = Vec::new();
    let mut stale = Vec::new();

    for (file, widget) in every_widget() {
        let path = root.join("src/widget").join(format!("{file}.rs"));
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
            .replace("\r\n", "\n");
        // A `pub struct XState` in the same file is the application-owned half.
        let companion = source
            .split("pub struct ")
            .skip(1)
            .filter_map(|rest| {
                rest.split(|c: char| !c.is_alphanumeric() && c != '_')
                    .next()
            })
            .find(|name| name.ends_with("State"))
            .map(str::to_string);
        let expected = block_for(widget.as_ref(), companion.as_deref());
        if printing {
            println!("=== src/widget/{file}.rs ===\n{expected}\n");
            continue;
        }

        let Some(start) = source.find("/// # Agent view") else {
            missing.push(format!("src/widget/{file}.rs\n{expected}"));
            continue;
        };
        // The block runs to the first line that is not a doc comment.
        let found: String = source[start..]
            .lines()
            .take_while(|l| l.trim_start().starts_with("///"))
            .collect::<Vec<_>>()
            .join("\n");
        if found.trim() != expected.trim() {
            stale.push(format!(
                "src/widget/{file}.rs\n--- the file says ---\n{found}\n\
                 --- the widget says ---\n{expected}"
            ));
        }
    }

    assert!(
        missing.is_empty(),
        "these widgets do not tell a reader what an agent can do with them. \
         Twelve paid runs said the source comments are where a model looks, so \
         an undocumented widget is invisible to the reader it matters most to:\n\n{}",
        missing.join("\n\n")
    );
    assert!(
        stale.is_empty(),
        "an Agent view block disagrees with the widget it describes. The block \
         is generated from the widget's own `schema`, `actions`, `agent_state` \
         and `capabilities`, so the file is what is wrong. Regenerate with \
         `DEWEY_AGENT_VIEW=print cargo test --test agent_view -- --nocapture`:\
         \n\n{}",
        stale.join("\n\n")
    );
}

/// Which actions are safe to repeat, pinned so a wrong answer is deliberate.
///
/// `AgentAction::simple(name, desc, mutates)` derives `idempotent: !mutates`,
/// so a `false` here is a promise to an agent that calling the action twice is
/// the same as calling it once — the promise a retry after a timeout is built
/// on. It is also what the strict `unwired_widget` check keys on: that check
/// only considers mutating actions, so an action wrongly marked read-only
/// becomes invisible to the check that finds controls wired to nothing.
///
/// Three were wrong. `Button::click` was one, and it made the commonest
/// control in any interface the one the check could not see — eight dead
/// buttons across four examples came out when it was corrected.
/// `CommandPalette::execute` runs a command and `Toolbar::click_item`
/// activates an item, and both were declared safe to repeat.
///
/// So the read-only set is written down. Adding to it should take an argument.
#[test]
fn only_genuine_queries_are_declared_repeatable() {
    // Each of these answers a question and changes nothing.
    const QUERIES: &[(&str, &str)] = &[
        ("color_picker", "get_color"),
        ("virtual_list", "get_visible_range"),
        ("command_palette", "list"),
        ("command_palette", "search"),
        ("toolbar", "list_items"),
    ];

    let mut unexpected = Vec::new();
    for (file, widget) in every_widget() {
        for action in widget.actions() {
            if action.mutates {
                assert!(
                    !action.idempotent,
                    "`{file}::{}` mutates and calls itself idempotent",
                    action.name
                );
                continue;
            }
            if !QUERIES.contains(&(file, action.name.as_str())) {
                unexpected.push(format!("{file}::{}", action.name));
            }
        }
    }
    assert!(
        unexpected.is_empty(),
        "these actions are declared non-mutating, which tells an agent they are          safe to repeat and hides them from the check that finds unwired          controls. If they really are queries, add them to QUERIES with a          reason; otherwise pass `true` for `mutates`:
  {}",
        unexpected.join("
  ")
    );
}
