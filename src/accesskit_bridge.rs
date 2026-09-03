//! Bridge between Dewey's ontology and [AccessKit](https://accesskit.dev/).
//!
//! Enable with `features = ["accesskit"]`. Every widget an agent can address
//! is also published to the platform's accessibility API, so a screen reader
//! sees the same interface the agent does — and a test harness that already
//! speaks AccessKit can drive a Dewey application without learning this
//! project's protocol.
//!
//! Dewey paints its widgets itself rather than building them out of egui
//! widgets, so egui's own accessibility tree would otherwise be empty: a
//! Dewey application was silently unusable with a screen reader. [`publish`]
//! fixes that by claiming an invisible egui rectangle per addressable widget
//! and stamping the ontology node onto it, which leaves egui to own node ids
//! and parenting.

use crate::ontology::{SemanticRole, UiNode};

/// Convert a Dewey [`SemanticRole`] to an AccessKit [`accesskit::Role`].
pub fn to_accesskit_role(role: SemanticRole) -> accesskit::Role {
    match role {
        SemanticRole::Display => accesskit::Role::Label,
        SemanticRole::Input => accesskit::Role::TextInput,
        SemanticRole::Navigation => accesskit::Role::Navigation,
        SemanticRole::Container => accesskit::Role::Group,
        SemanticRole::Progress => accesskit::Role::ProgressIndicator,
        SemanticRole::Selection => accesskit::Role::ListBox,
        SemanticRole::DataVisualization => accesskit::Role::Figure,
        SemanticRole::Decoration => accesskit::Role::GenericContainer,
        SemanticRole::Action => accesskit::Role::Button,
        SemanticRole::Scrollable => accesskit::Role::ScrollView,
        SemanticRole::Modal => accesskit::Role::Dialog,
        SemanticRole::Menu => accesskit::Role::Menu,
        SemanticRole::Toolbar => accesskit::Role::Toolbar,
        SemanticRole::Tab => accesskit::Role::Tab,
        SemanticRole::TreeNode => accesskit::Role::TreeItem,
        SemanticRole::Canvas => accesskit::Role::Canvas,
        SemanticRole::Media => accesskit::Role::Image,
        SemanticRole::System | SemanticRole::Diagnostic | SemanticRole::Configuration => {
            accesskit::Role::GenericContainer
        }
        SemanticRole::Custom => accesskit::Role::Unknown,
    }
}

/// Convert a Dewey [`UiNode`] to an AccessKit [`accesskit::Node`].
///
/// The returned node has the mapped role, label, description, and value set
/// from the `UiNode`'s properties and accessibility metadata.
pub fn to_accesskit_node(ui_node: &UiNode) -> accesskit::Node {
    let role = to_accesskit_role(ui_node.role);
    let mut node = accesskit::Node::new(role);

    // Label. Widgets carry their text as a property rather than in the node's
    // `label` field — `Button` and `Checkbox` under "label", `Label` under
    // "text" — so a bridge that read only `label` announced every one of them
    // as unnamed, which is the difference between a usable screen reader and
    // a list of anonymous buttons.
    let announced = ui_node.label.as_deref().or_else(|| {
        ["label", "text", "title", "placeholder"]
            .iter()
            .find_map(|key| ui_node.state.get(key).and_then(serde_json::Value::as_str))
            .filter(|text| !text.is_empty())
    });
    if let Some(label) = announced {
        node.set_label(label);
    }

    // Description from accessibility
    if let Some(desc) = &ui_node.accessibility().description {
        node.set_description(desc.as_str());
    }

    // Value text
    if let Some(val) = &ui_node.accessibility().value_text {
        node.set_value(val.as_str());
    }

    // Disabled state
    if ui_node.accessibility().disabled == Some(true) {
        node.set_disabled();
    }

    // Expanded state
    if let Some(expanded) = ui_node.accessibility().expanded {
        node.set_expanded(expanded);
    }

    // Selected state
    if ui_node.accessibility().selected == Some(true) {
        node.set_selected(true);
    }

    // Required state
    if ui_node.accessibility().required == Some(true) {
        node.set_required();
    }

    // Keyboard shortcut
    if let Some(shortcut) = &ui_node.accessibility().shortcut {
        node.set_keyboard_shortcut(shortcut.as_str());
    }

    // Live region
    if let Some(live) = &ui_node.accessibility().live {
        let live_setting = match live.as_str() {
            "assertive" => accesskit::Live::Assertive,
            "polite" => accesskit::Live::Polite,
            _ => accesskit::Live::Off,
        };
        node.set_live(live_setting);
    }

    node
}

/// Publish every addressable widget in `tree` to the platform accessibility
/// API, through egui.
///
/// `newly_focused` is the widget Dewey's focus ring has just moved to, or
/// `None` when it has not moved. A screen reader announces the focused node
/// and nothing else, so a tree published without one is a list that can be
/// read to a user but not walked — which is what this published for as long as
/// pressing Tab did nothing at all. It is passed only on a change, because
/// asking egui for focus on every frame would fight anything else that wants
/// it.
///
/// Does nothing when no assistive technology is attached: egui only builds an
/// AccessKit tree when the platform has asked for one, and
/// `accesskit_node_builder` returns `None` until then, so the per-widget cost
/// is a rectangle allocation that is skipped in the ordinary case.
#[cfg(feature = "egui-backend")]
pub fn publish(ui: &mut egui::Ui, tree: &crate::ontology::UiTree, newly_focused: Option<&str>) {
    fn walk(ui: &mut egui::Ui, node: &UiNode, newly_focused: Option<&str>) {
        if let (Some(id), Some(bounds)) = (node.agent_id.as_deref(), node.bounds.as_ref()) {
            let rect = egui::Rect::from_min_size(
                egui::pos2(bounds.x, bounds.y),
                egui::vec2(bounds.width.max(1.0), bounds.height.max(1.0)),
            );
            let response = ui.allocate_rect(rect, egui::Sense::click());
            let built = to_accesskit_node(node);
            ui.ctx().accesskit_node_builder(response.id, |target| {
                *target = built;
            });
            if newly_focused == Some(id) {
                response.request_focus();
            }
        }
        for child in &node.children {
            walk(ui, child, newly_focused);
        }
    }

    // One probe: if egui is not collecting accessibility this frame, every
    // call below would be a no-op with an allocation each, so skip the walk.
    if ui
        .ctx()
        .accesskit_node_builder(egui::Id::new("dewey_a11y_probe"), |_| ())
        .is_none()
    {
        return;
    }
    walk(ui, &tree.root, newly_focused);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ontology::registry::Accessibility;

    #[test]
    fn semantic_role_to_accesskit() {
        assert_eq!(
            to_accesskit_role(SemanticRole::Action),
            accesskit::Role::Button
        );
        assert_eq!(
            to_accesskit_role(SemanticRole::Input),
            accesskit::Role::TextInput
        );
        assert_eq!(
            to_accesskit_role(SemanticRole::Modal),
            accesskit::Role::Dialog
        );
        assert_eq!(to_accesskit_role(SemanticRole::Tab), accesskit::Role::Tab);
        assert_eq!(
            to_accesskit_role(SemanticRole::Canvas),
            accesskit::Role::Canvas
        );
    }

    #[test]
    fn ui_node_to_accesskit_node() {
        let mut ui = UiNode::new("Button", SemanticRole::Action);
        ui.label = Some("Save".into());
        ui.accessibility = Some(Box::new(Accessibility {
            description: Some("Save current file".into()),
            shortcut: Some("Ctrl+S".into()),
            disabled: Some(false),
            ..Default::default()
        }));

        let ak = to_accesskit_node(&ui);
        assert_eq!(ak.role(), accesskit::Role::Button);
        assert_eq!(ak.label(), Some("Save"));
        assert_eq!(ak.description(), Some("Save current file"));
        assert_eq!(ak.keyboard_shortcut(), Some("Ctrl+S"));
    }
}
