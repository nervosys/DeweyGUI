//! An accessibility tree, in this crate's own vocabulary.
//!
//! **Not AccessKit.** [`Role`] and [`AccessibilityNode`] are defined here
//! rather than mapped onto the platform APIs, so nothing built from this
//! module reaches a screen reader without a conversion an embedder writes.
//! `deweygui` publishes to AccessKit through its own bridge instead.
//!
//! Nothing in this crate drives it.

use serde::{Deserialize, Serialize};

/// Platform-independent accessible role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    Window,
    Button,
    TextInput,
    Label,
    Checkbox,
    RadioButton,
    Slider,
    ProgressBar,
    List,
    ListItem,
    Table,
    TableRow,
    TableCell,
    Menu,
    MenuItem,
    MenuBar,
    Tab,
    TabPanel,
    Tree,
    TreeItem,
    Dialog,
    Tooltip,
    ScrollBar,
    Toolbar,
    Group,
    Image,
    Link,
    Separator,
    Custom,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

/// ARIA-style state flags for an accessible element.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AccessibleState {
    pub checked: Option<bool>,
    pub selected: bool,
    pub expanded: Option<bool>,
    pub disabled: bool,
    pub focused: bool,
    pub read_only: bool,
    pub required: bool,
    pub value_now: Option<f64>,
    pub value_min: Option<f64>,
    pub value_max: Option<f64>,
    pub value_text: Option<String>,
}

/// A single node in the accessibility tree.
#[derive(Debug, Clone)]
pub struct AccessibilityNode {
    pub id: String,
    pub role: Role,
    pub label: Option<String>,
    pub description: Option<String>,
    pub state: AccessibleState,
    pub children: Vec<AccessibilityNode>,
}

impl AccessibilityNode {
    pub fn new(id: impl Into<String>, role: Role) -> Self {
        Self {
            id: id.into(),
            role,
            label: None,
            description: None,
            state: AccessibleState::default(),
            children: Vec::new(),
        }
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn state(mut self, state: AccessibleState) -> Self {
        self.state = state;
        self
    }

    pub fn child(mut self, child: AccessibilityNode) -> Self {
        self.children.push(child);
        self
    }

    /// Find a node by ID (depth-first).
    pub fn find(&self, id: &str) -> Option<&AccessibilityNode> {
        if self.id == id {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.find(id) {
                return Some(found);
            }
        }
        None
    }

    /// Find all nodes with a given role.
    pub fn find_by_role(&self, role: Role) -> Vec<&AccessibilityNode> {
        let mut results = Vec::new();
        if self.role == role {
            results.push(self);
        }
        for child in &self.children {
            results.extend(child.find_by_role(role));
        }
        results
    }

    /// Total number of nodes (including self).
    pub fn count(&self) -> usize {
        1 + self.children.iter().map(|c| c.count()).sum::<usize>()
    }
}

/// The root accessibility tree for the application.
pub struct AccessibilityTree {
    root: Option<AccessibilityNode>,
}

impl AccessibilityTree {
    pub fn new() -> Self {
        Self { root: None }
    }

    pub fn set_root(&mut self, root: AccessibilityNode) {
        self.root = Some(root);
    }

    pub fn root(&self) -> Option<&AccessibilityNode> {
        self.root.as_ref()
    }

    pub fn find(&self, id: &str) -> Option<&AccessibilityNode> {
        self.root.as_ref()?.find(id)
    }

    pub fn find_by_role(&self, role: Role) -> Vec<&AccessibilityNode> {
        match &self.root {
            Some(root) => root.find_by_role(role),
            None => Vec::new(),
        }
    }

    pub fn node_count(&self) -> usize {
        self.root.as_ref().map_or(0, |r| r.count())
    }
}

impl Default for AccessibilityTree {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_display() {
        assert_eq!(format!("{}", Role::Button), "Button");
    }

    #[test]
    fn node_builder() {
        let node = AccessibilityNode::new("btn1", Role::Button)
            .label("Submit")
            .description("Submit the form");
        assert_eq!(node.label.as_deref(), Some("Submit"));
        assert_eq!(node.role, Role::Button);
    }

    #[test]
    fn node_children() {
        let tree = AccessibilityNode::new("root", Role::Window)
            .child(AccessibilityNode::new("btn1", Role::Button).label("OK"))
            .child(AccessibilityNode::new("btn2", Role::Button).label("Cancel"));
        assert_eq!(tree.children.len(), 2);
    }

    #[test]
    fn node_find() {
        let tree = AccessibilityNode::new("root", Role::Window).child(
            AccessibilityNode::new("toolbar", Role::Toolbar)
                .child(AccessibilityNode::new("btn1", Role::Button)),
        );
        assert!(tree.find("btn1").is_some());
        assert!(tree.find("nope").is_none());
    }

    #[test]
    fn node_find_by_role() {
        let tree = AccessibilityNode::new("root", Role::Window)
            .child(AccessibilityNode::new("b1", Role::Button))
            .child(AccessibilityNode::new("t1", Role::TextInput))
            .child(AccessibilityNode::new("b2", Role::Button));
        let buttons = tree.find_by_role(Role::Button);
        assert_eq!(buttons.len(), 2);
    }

    #[test]
    fn node_count() {
        let tree = AccessibilityNode::new("root", Role::Window)
            .child(AccessibilityNode::new("a", Role::Button))
            .child(
                AccessibilityNode::new("b", Role::Group)
                    .child(AccessibilityNode::new("c", Role::Label)),
            );
        assert_eq!(tree.count(), 4);
    }

    #[test]
    fn accessible_state_defaults() {
        let state = AccessibleState::default();
        assert!(!state.selected);
        assert!(!state.disabled);
        assert!(state.checked.is_none());
    }

    #[test]
    fn tree_empty() {
        let tree = AccessibilityTree::new();
        assert_eq!(tree.node_count(), 0);
        assert!(tree.root().is_none());
    }

    #[test]
    fn tree_with_root() {
        let mut tree = AccessibilityTree::new();
        tree.set_root(
            AccessibilityNode::new("app", Role::Window)
                .child(AccessibilityNode::new("btn", Role::Button)),
        );
        assert_eq!(tree.node_count(), 2);
        assert!(tree.find("btn").is_some());
    }

    #[test]
    fn tree_find_by_role() {
        let mut tree = AccessibilityTree::new();
        tree.set_root(
            AccessibilityNode::new("app", Role::Window)
                .child(AccessibilityNode::new("b1", Role::Button))
                .child(AccessibilityNode::new("b2", Role::Button)),
        );
        assert_eq!(tree.find_by_role(Role::Button).len(), 2);
    }
}
