//! Bridge between Dewey's ontology and [AccessKit](https://accesskit.dev/).
//!
//! Enable with `features = ["accesskit"]`. Provides conversion from Dewey's
//! `SemanticRole` and `UiNode` types to AccessKit `Role` and `Node`.

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

    // Label
    if let Some(label) = &ui_node.label {
        node.set_label(label.as_str());
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
