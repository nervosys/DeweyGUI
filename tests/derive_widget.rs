//! `#[derive(Widget)]` generates what its documentation says it does.
//!
//! The macro ships behind the `derive` feature, is re-exported from this
//! crate's root, and was used by nothing: not a widget, not a test, not an
//! example. Its only usage example is an `ignore`d doctest, so nothing
//! compiled that either — and CI builds only the default features and
//! `accesskit`, so the whole crate went unbuilt on every push.
//!
//! A proc macro nobody invokes is worse off than an ordinary unused function:
//! it can emit code that does not compile, and there is nothing to notice.
#![cfg(feature = "derive")]

use dewey::Widget;
use dewey::ontology::{Discoverable, SemanticRole};

/// The struct from the macro's own documentation, compiled for once.
#[derive(Widget)]
#[widget(name = "StatusBar", role = "Display", desc = "Shows status info")]
struct StatusBar {
    text: String,
    connected: bool,
    #[widget(skip)]
    #[allow(dead_code)]
    internal: usize,
}

/// No container attributes: every default has to hold on its own.
#[derive(Widget)]
struct Bare {
    value: i32,
}

#[test]
fn the_container_attributes_reach_the_schema() {
    let bar = StatusBar {
        text: "ready".into(),
        connected: true,
        internal: 7,
    };
    let schema = bar.schema();
    assert_eq!(schema.name, "StatusBar");
    assert_eq!(schema.description, "Shows status info");
    assert_eq!(bar.semantic_role(), SemanticRole::Display);
}

#[test]
fn the_defaults_are_the_documented_ones() {
    let bare = Bare { value: 1 };
    // "Widget type name (default: struct name)" and "default: Display".
    assert_eq!(bare.schema().name, "Bare");
    assert_eq!(bare.schema().description, "");
    assert_eq!(bare.semantic_role(), SemanticRole::Display);
}

#[test]
fn fields_reach_agent_state_and_skipped_ones_do_not() {
    let bar = StatusBar {
        text: "ready".into(),
        connected: true,
        internal: 7,
    };
    let state = bar.agent_state();
    assert_eq!(state["text"], "ready");
    assert_eq!(state["connected"], true);
    assert!(
        state.get("internal").is_none(),
        "`#[widget(skip)]` is documented as excluding a field from \
         `agent_state()`, and it is there: {state}"
    );
}

/// The generated `execute_action` refuses by name rather than succeeding
/// silently, which is the same rule every hand-written widget follows.
#[test]
fn an_unknown_action_is_refused_and_names_the_widget() {
    let mut bar = StatusBar {
        text: String::new(),
        connected: false,
        internal: 0,
    };
    let err = bar
        .execute_action("frobnicate", &serde_json::Value::Null)
        .expect_err("a derived widget advertises no actions, so any call is unknown");
    assert!(err.contains("StatusBar"), "{err}");
    assert!(err.contains("frobnicate"), "{err}");
}

/// A derived widget advertises nothing, so `validate --strict` has nothing to
/// report about it. Worth pinning: the generated `actions()` returning a
/// non-empty list with no handlers would make every derived widget an error.
#[test]
fn a_derived_widget_advertises_no_actions() {
    let bare = Bare { value: 1 };
    assert!(bare.actions().is_empty());
    assert!(bare.capabilities().is_empty());
}
