//! Label widget — displays static or dynamic text.

use crate::core::{Color, Position, Rect, Style};
use crate::ontology::*;
use crate::runtime::Frame;
use crate::widget::Widget;

/// A text label for displaying static or dynamic text.
///
/// # Examples
///
/// ```
/// # use dewey::prelude::*;
/// Label::new("Hello, Dewey!")
///     .fg(Color::hex("#1A73E8"))
///     .text_size(18.0)
///     .bold();
/// ```
pub struct Label {
    text: String,
    style: Style,
    agent_id: std::borrow::Cow<'static, str>,
}

impl Label {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: Style::default(),
            agent_id: std::borrow::Cow::Borrowed(""),
        }
    }

    /// Override the full visual style.
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// Set the background color.
    pub fn bg(mut self, color: Color) -> Self {
        self.style.background = Some(color);
        self
    }

    /// Set the text color.
    pub fn fg(mut self, color: Color) -> Self {
        self.style.foreground = Some(color);
        self
    }

    /// Set the corner radius.
    pub fn rounded(mut self, radius: f32) -> Self {
        self.style.border_radius = Some(radius);
        self
    }

    /// Set the text font size.
    pub fn text_size(mut self, size: f32) -> Self {
        self.style = self.style.text_size(size);
        self
    }

    /// Set the text weight to bold.
    pub fn bold(mut self) -> Self {
        self.style = self.style.bold();
        self
    }

    pub fn agent_id(mut self, id: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        self.agent_id = id.into();
        self
    }
}

impl Discoverable for Label {
    fn schema(&self) -> WidgetSchema {
        let mut schema = WidgetSchema::new("Label", "A text label", SemanticRole::Display);
        schema.usage_hint = Some("Label::new(\"Hello world\")".into());
        schema.tags = vec!["label".into(), "text".into(), "display".into()];
        schema
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![AgentCapability::Focusable]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::Display
    }

    fn agent_state(&self) -> serde_json::Value {
        serde_json::json!({ "text": self.text })
    }

    fn execute_action(
        &mut self,
        _action: &str,
        _params: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        Err("Label has no actions".to_string())
    }

    fn agent_id(&self) -> Option<&str> {
        if self.agent_id.is_empty() {
            None
        } else {
            Some(&self.agent_id)
        }
    }

    fn accessibility_label(&self) -> Option<String> {
        Some(self.text.clone())
    }
}

impl Widget for Label {
    fn render(self, area: Rect, frame: &mut Frame<'_>) {
        if frame.ontology_enabled() && !self.agent_id.is_empty() {
            let node = UiNode::new("Label", SemanticRole::Display)
                .with_id(self.agent_id.clone())
                .with_bounds(area.into())
                .with_property("text", serde_json::json!(self.text));
            frame.register_widget(node);
        }

        // Draw background if specified
        if let Some(bg) = self.style.background {
            if bg.a > 0.0 {
                let radius = self.style.border_radius.unwrap_or(0.0);
                frame.painter().fill_rect(area, bg, radius);
            }
        }

        let ts = self.style.resolved_text();
        let text_size = frame.painter().measure_text(&self.text, &ts);
        // Vertically center text within the area, with left padding
        let tx = area.x + 4.0;
        let ty = area.y + (area.height - text_size.height).max(0.0) * 0.5;
        frame.painter().text(Position::new(tx, ty), &self.text, &ts);
    }
}
