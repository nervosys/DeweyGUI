//! Ontology registry, UI tree, and node types.
//!
//! The [`OntologyRegistry`] catalogs widget schemas, while [`UiTree`] and
//! [`UiNode`] represent the live widget tree that agents can inspect.

use super::{AgentCapability, Discoverable, SemanticRole, WidgetSchema};
use crate::core::rect::Rect;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Global registry of widget types and the live UI tree.
///
/// 1. **Type catalog**: Agents can list all available widget types, search by
///    name/role/tag, and read schemas before interacting.
/// 2. **UI tree**: The current widget hierarchy exposed as a navigable tree.
#[derive(Debug, Default)]
pub struct OntologyRegistry {
    schemas: HashMap<String, WidgetSchema>,
    tree: Option<UiTree>,
}

impl OntologyRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    // ── Type Catalog ─────────────────────────────────────────────────

    /// Register a widget type's schema.
    pub fn register_schema(&mut self, schema: WidgetSchema) {
        self.schemas.insert(schema.name.clone(), schema);
    }

    /// Register a discoverable widget type (convenience).
    pub fn register<W: Discoverable>(&mut self, instance: &W) {
        self.register_schema(instance.schema());
    }

    /// List all registered widget type names.
    pub fn list_types(&self) -> Vec<&str> {
        self.schemas.keys().map(|s| s.as_str()).collect()
    }

    /// Get the schema for a widget type by name.
    pub fn get_schema(&self, name: &str) -> Option<&WidgetSchema> {
        self.schemas.get(name)
    }

    /// Find widget types matching a semantic role.
    pub fn find_by_role(&self, role: SemanticRole) -> Vec<&WidgetSchema> {
        self.schemas
            .values()
            .filter(|s| s.default_role == role)
            .collect()
    }

    /// Search widget types by tag (case-insensitive substring match).
    pub fn search(&self, query: &str) -> Vec<&WidgetSchema> {
        let query_lower = query.to_lowercase();
        self.schemas
            .values()
            .filter(|s| {
                s.name.to_lowercase().contains(&query_lower)
                    || s.description.to_lowercase().contains(&query_lower)
                    || s.tags
                        .iter()
                        .any(|t| t.to_lowercase().contains(&query_lower))
            })
            .collect()
    }

    /// Export the full type catalog as JSON.
    pub fn export_catalog(&self) -> serde_json::Value {
        serde_json::to_value(&self.schemas).unwrap_or_default()
    }

    /// Validate params against a declared action schema.
    pub fn validate_action_params(
        &self,
        widget_type: &str,
        action: &str,
        params: &serde_json::Value,
    ) -> Result<(), String> {
        let Some(schema) = self.schemas.get(widget_type) else {
            return Ok(());
        };
        let Some(declared) = schema.actions.iter().find(|a| a.name == action) else {
            return Ok(());
        };
        declared.validate_params(params)
    }

    // ── Live UI Tree ─────────────────────────────────────────────────

    /// Set the current UI tree snapshot.
    pub fn set_tree(&mut self, tree: UiTree) {
        self.tree = Some(tree);
    }

    /// Get the current UI tree.
    pub fn tree(&self) -> Option<&UiTree> {
        self.tree.as_ref()
    }

    /// Find a node in the UI tree by its agent ID.
    pub fn find_node(&self, agent_id: &str) -> Option<&UiNode> {
        self.tree.as_ref().and_then(|t| t.find(agent_id))
    }

    /// Export the UI tree as JSON for agent consumption.
    pub fn export_tree(&self) -> serde_json::Value {
        match &self.tree {
            Some(tree) => serde_json::to_value(tree).unwrap_or_default(),
            None => serde_json::Value::Null,
        }
    }
}

/// A snapshot of the live UI widget tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiTree {
    pub root: UiNode,
}

impl UiTree {
    pub fn new(root: UiNode) -> Self {
        Self { root }
    }

    /// Depth-first search for a node by agent_id.
    pub fn find(&self, agent_id: &str) -> Option<&UiNode> {
        self.root.find(agent_id)
    }

    /// Collect all nodes matching a role.
    pub fn find_by_role(&self, role: SemanticRole) -> Vec<&UiNode> {
        let mut results = Vec::new();
        self.root.collect_by_role(role, &mut results);
        results
    }

    /// Collect all focusable nodes.
    pub fn focusable_nodes(&self) -> Vec<&UiNode> {
        let mut results = Vec::new();
        self.root.collect_by_capability("focusable", &mut results);
        results
    }
}

/// A node in the UI tree representing a single widget instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiNode {
    /// Optional unique agent-addressable ID.
    pub agent_id: Option<std::borrow::Cow<'static, str>>,
    /// The widget type name (matches a registered schema).
    pub widget_type: std::borrow::Cow<'static, str>,
    /// Semantic role of this instance.
    pub role: SemanticRole,
    /// Capabilities of this instance.
    pub capabilities: Vec<AgentCapability>,
    /// Current state snapshot as JSON.
    pub state: serde_json::Value,
    /// Accessibility label.
    pub label: Option<String>,
    /// Bounding rectangle in logical pixel coordinates.
    pub bounds: Option<NodeBounds>,
    /// Accessibility attributes for screen readers and assistive agents.
    #[serde(default, skip_serializing_if = "Accessibility::is_empty")]
    pub accessibility: Accessibility,
    /// Child nodes.
    pub children: Vec<UiNode>,
}

/// Bounding rectangle of a UI node in logical pixel coordinates.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct NodeBounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl From<Rect> for NodeBounds {
    fn from(r: Rect) -> Self {
        Self {
            x: r.x,
            y: r.y,
            width: r.width,
            height: r.height,
        }
    }
}

/// Accessibility attributes following ARIA conventions.
///
/// Agents and screen readers use these to understand widget semantics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Accessibility {
    /// ARIA role override (e.g., "button", "textbox", "dialog").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Description for screen readers (more detailed than label).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether the widget is disabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
    /// Current value for range/input widgets (e.g., slider value text).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_text: Option<String>,
    /// Whether the widget is in an expanded state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expanded: Option<bool>,
    /// Whether the widget is selected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected: Option<bool>,
    /// Whether the widget is required (form validation).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    /// Keyboard shortcut to activate this widget (e.g., "Ctrl+S").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shortcut: Option<String>,
    /// Tab index for focus ordering. `None` = natural order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab_index: Option<i32>,
    /// Live region announcement level: "off", "polite", "assertive".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live: Option<String>,
}

impl Accessibility {
    pub fn is_empty(&self) -> bool {
        self.role.is_none()
            && self.description.is_none()
            && self.disabled.is_none()
            && self.value_text.is_none()
            && self.expanded.is_none()
            && self.selected.is_none()
            && self.required.is_none()
            && self.shortcut.is_none()
            && self.tab_index.is_none()
            && self.live.is_none()
    }
}

impl UiNode {
    #[must_use]
    pub fn new(widget_type: impl Into<std::borrow::Cow<'static, str>>, role: SemanticRole) -> Self {
        Self {
            agent_id: None,
            widget_type: widget_type.into(),
            role,
            capabilities: Vec::new(),
            state: serde_json::Value::Null,
            label: None,
            bounds: None,
            accessibility: Accessibility::default(),
            children: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_id(mut self, id: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        self.agent_id = Some(id.into());
        self
    }

    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    #[must_use]
    pub fn with_bounds(mut self, bounds: NodeBounds) -> Self {
        self.bounds = Some(bounds);
        self
    }

    #[must_use]
    pub fn with_state(mut self, state: serde_json::Value) -> Self {
        self.state = state;
        self
    }

    #[must_use]
    pub fn with_capability(mut self, cap: AgentCapability) -> Self {
        self.capabilities.push(cap);
        self
    }

    #[must_use]
    pub fn with_child(mut self, child: UiNode) -> Self {
        self.children.push(child);
        self
    }

    /// Set accessibility attributes.
    #[must_use]
    pub fn with_accessibility(mut self, acc: Accessibility) -> Self {
        self.accessibility = acc;
        self
    }

    /// Convenience: set a named property in the state JSON object.
    #[must_use]
    pub fn with_property(mut self, key: &str, value: serde_json::Value) -> Self {
        if self.state.is_null() {
            self.state = serde_json::json!({});
        }
        if let Some(obj) = self.state.as_object_mut() {
            obj.insert(key.to_string(), value);
        }
        self
    }

    /// Depth-first search by agent_id.
    pub fn find(&self, agent_id: &str) -> Option<&UiNode> {
        if self.agent_id.as_deref() == Some(agent_id) {
            return Some(self);
        }
        for child in &self.children {
            if let Some(node) = child.find(agent_id) {
                return Some(node);
            }
        }
        None
    }

    /// Collect nodes matching a semantic role.
    pub fn collect_by_role<'a>(&'a self, role: SemanticRole, results: &mut Vec<&'a UiNode>) {
        if self.role == role {
            results.push(self);
        }
        for child in &self.children {
            child.collect_by_role(role, results);
        }
    }

    /// Collect nodes with a specific capability.
    pub fn collect_by_capability<'a>(&'a self, cap_name: &str, results: &mut Vec<&'a UiNode>) {
        if self.capabilities.iter().any(|c| c.name() == cap_name) {
            results.push(self);
        }
        for child in &self.children {
            child.collect_by_capability(cap_name, results);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_search() {
        let mut reg = OntologyRegistry::new();
        reg.register_schema(WidgetSchema {
            name: "Button".into(),
            description: "A clickable button".into(),
            default_role: SemanticRole::Action,
            properties: vec![],
            actions: vec![],
            usage_hint: None,
            tags: vec!["button".into(), "action".into()],
        });
        assert_eq!(reg.search("button").len(), 1);
        assert_eq!(reg.search("nonexistent").len(), 0);
    }

    #[test]
    fn ui_tree_find() {
        let tree = UiTree::new(
            UiNode::new("Panel", SemanticRole::Container)
                .with_id("root")
                .with_child(
                    UiNode::new("Button", SemanticRole::Action)
                        .with_id("btn-1")
                        .with_capability(AgentCapability::Focusable),
                ),
        );
        assert!(tree.find("btn-1").is_some());
        assert!(tree.find("missing").is_none());
        assert_eq!(tree.focusable_nodes().len(), 1);
    }
}
