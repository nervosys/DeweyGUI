//! Tooltip widget — displays a hover tip that wraps inner content.

use crate::core::{Color, Position, Rect, Style};
use crate::ontology::*;
use crate::runtime::Frame;
use crate::widget::Widget;

/// A tooltip that wraps a label and shows hover text.
///
/// Renders the label text. The tooltip text is exposed via the ontology
/// for agent discovery. Visual tooltip popups are handled by the backend.
pub struct Tooltip {
    /// The visible label text.
    label: String,
    /// The tooltip text shown on hover.
    text: String,
    style: Style,
    agent_id: std::borrow::Cow<'static, str>,
}

impl Tooltip {
    /// Create a new tooltip with text that appears on hover.
    /// `label` is shown inline; `text` appears when the user hovers.
    #[must_use]
    pub fn new(label: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            text: text.into(),
            style: Style::default(),
            agent_id: std::borrow::Cow::Borrowed(""),
        }
    }

    /// Create a tooltip with only hover text (empty label).
    pub fn hover_only(text: impl Into<String>) -> Self {
        Self {
            label: String::new(),
            text: text.into(),
            style: Style::default(),
            agent_id: std::borrow::Cow::Borrowed(""),
        }
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn fg(mut self, color: Color) -> Self {
        self.style.foreground = Some(color);
        self
    }

    pub fn agent_id(mut self, id: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        self.agent_id = id.into();
        self
    }
}

impl Discoverable for Tooltip {
    fn schema(&self) -> WidgetSchema {
        let mut schema =
            WidgetSchema::new("Tooltip", "A tooltip shown on hover", SemanticRole::Display);
        schema.usage_hint = Some("Tooltip::new(\"Hover me\", \"Extra info\")".into());
        schema.tags = vec!["tooltip".into(), "hover".into(), "hint".into()];
        schema
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![AgentCapability::HasTooltip]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::Display
    }

    fn agent_state(&self) -> serde_json::Value {
        serde_json::json!({ "label": self.label, "text": self.text })
    }

    fn execute_action(
        &mut self,
        _action: &str,
        _params: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        Err("Tooltip has no actions".to_string())
    }

    fn agent_id(&self) -> Option<&str> {
        if self.agent_id.is_empty() {
            None
        } else {
            Some(&self.agent_id)
        }
    }

    fn accessibility_label(&self) -> Option<String> {
        if self.label.is_empty() {
            None
        } else {
            Some(self.label.clone())
        }
    }
}

impl Widget for Tooltip {
    fn render(self, area: Rect, frame: &mut Frame<'_>) {
        if frame.ontology_enabled() && !self.agent_id.is_empty() {
            let node = UiNode::new("Tooltip", SemanticRole::Display)
                .with_id(self.agent_id.clone())
                .with_bounds(area.into())
                .with_property("label", serde_json::json!(self.label))
                .with_property("text", serde_json::json!(self.text));
            frame.register_widget(node);
        }

        let label = if self.label.is_empty() {
            "(?)"
        } else {
            &self.label
        };
        let ts = self.style.resolved_text();
        frame
            .painter()
            .text(Position::new(area.x, area.y), label, &ts);
    }
}
