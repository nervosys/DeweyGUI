//! # Ontology — Agent Discoverability System
//!
//! Provides structured metadata for every component in a Dewey application.
//! AI agents can introspect widget schemas, discover available actions,
//! query capabilities, and navigate the UI tree programmatically.

mod action;
mod capability;
pub mod registry;
mod schema;

pub use action::{ActionParam, ActionParamType, AgentAction};
pub use capability::AgentCapability;
pub mod builtin;
pub mod diagnostics;
pub use diagnostics::{Diagnostic, Severity};
pub use registry::{Accessibility, NodeBounds, OntologyRegistry, Properties, UiNode, UiTree};
pub use schema::{PropertyConstraint, PropertySchema, PropertyType, SemanticRole, WidgetSchema};

/// Trait for widgets that expose metadata to agents.
///
/// Any widget implementing this trait becomes discoverable: agents can introspect
/// its schema, invoke actions, and read its state as structured data.
pub trait Discoverable {
    /// Returns the schema describing this widget type. May use instance state.
    fn schema(&self) -> WidgetSchema;

    /// Returns the capabilities of this specific widget instance.
    fn capabilities(&self) -> Vec<AgentCapability>;

    /// Returns the actions available on this specific widget instance.
    fn actions(&self) -> Vec<AgentAction>;

    /// Returns the semantic role of this widget.
    fn semantic_role(&self) -> SemanticRole;

    /// Returns the current state as a JSON value for agent inspection.
    fn agent_state(&self) -> serde_json::Value;

    /// Attempt to execute a named action with the given JSON parameters.
    ///
    /// **This is not the path an agent takes.** Widgets are values built
    /// afresh inside `view` on every frame, so a change made here is
    /// discarded at the end of the frame that made it. Nothing in the agent
    /// protocol calls it; `execute_action` over the wire reaches the handler
    /// a widget registered (`Button::on`, `Table::on_change`, …) and then
    /// [`Model::execute_action`](crate::runtime::Model::execute_action),
    /// both of which change the application's own state.
    ///
    /// It remains useful for widgets that own durable state, and for testing
    /// a widget's action semantics in isolation.
    fn execute_action(
        &mut self,
        action: &str,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, String>;

    /// An optional unique identifier for this widget instance in the UI tree.
    fn agent_id(&self) -> Option<&str> {
        None
    }

    /// An optional accessibility label for screen readers and agents.
    fn accessibility_label(&self) -> Option<String> {
        None
    }
}
