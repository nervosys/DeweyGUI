//! Button widget — a clickable element.

use crate::core::{Color, Rect, Style};
use crate::ontology::*;
use crate::runtime::Frame;
use crate::widget::Widget;

/// A clickable button.
///
/// # Examples
///
/// ```
/// # use dewey::prelude::*;
/// Button::new("Save")
///     .bg(Color::BLUE)
///     .fg(Color::WHITE)
///     .rounded(8.0)
///     .text_size(16.0);
/// ```
pub struct Button {
    label: String,
    style: Style,
    enabled: bool,
    agent_id: std::borrow::Cow<'static, str>,
}

impl Button {
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            style: Style::default(),
            enabled: true,
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

    /// Set the foreground (text) color.
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

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn agent_id(mut self, id: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        self.agent_id = id.into();
        self
    }
}

impl Discoverable for Button {
    fn schema(&self) -> WidgetSchema {
        let mut schema = WidgetSchema::new("Button", "A clickable button", SemanticRole::Action);
        schema.usage_hint = Some("Button::new(\"Save\").enabled(true)".into());
        schema.tags = vec!["button".into(), "click".into(), "action".into()];
        schema
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![AgentCapability::Clickable, AgentCapability::Focusable]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![AgentAction::simple("click", "Click the button", false)]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::Action
    }

    fn agent_state(&self) -> serde_json::Value {
        serde_json::json!({ "label": self.label, "enabled": self.enabled })
    }

    fn execute_action(
        &mut self,
        action: &str,
        _params: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        match action {
            "click" if self.enabled => Ok(serde_json::json!({ "clicked": true })),
            "click" => Err("Button is disabled".to_string()),
            _ => Err(format!("Unknown action: {action}")),
        }
    }

    fn agent_id(&self) -> Option<&str> {
        if self.agent_id.is_empty() {
            None
        } else {
            Some(&self.agent_id)
        }
    }

    fn accessibility_label(&self) -> Option<String> {
        Some(self.label.clone())
    }
}

impl Widget for Button {
    fn render(self, area: Rect, frame: &mut Frame<'_>) {
        if !self.agent_id.is_empty() {
            if frame.ontology_enabled() {
                let node = UiNode::new("Button", SemanticRole::Action)
                    .with_id(self.agent_id.clone())
                    .with_bounds(area.into())
                    .with_property("label", serde_json::json!(self.label))
                    .with_property("enabled", serde_json::json!(self.enabled));
                frame.register_widget(node);
            }
            frame.register_hitbox(self.agent_id.clone(), area, 1);
        }

        let bg = if self.enabled {
            self.style.background.unwrap_or(Color::DARK_GRAY)
        } else {
            Color::GRAY
        };
        let fg = self.style.resolved_fg();
        let radius = self.style.border_radius.unwrap_or(6.0);
        frame.painter().fill_rect(area, bg, radius);
        if let Some(border_color) = self.style.border_color {
            let border_w = self.style.border_width.unwrap_or(1.0);
            frame
                .painter()
                .stroke_rect(area, border_color, border_w, radius);
        }
        let mut ts = self.style.resolved_text();
        ts.color = if self.enabled { fg } else { Color::GRAY };
        let text_size = frame.painter().measure_text(&self.label, &ts);
        let tx = area.x + (area.width - text_size.width) * 0.5;
        let ty = area.y + (area.height - text_size.height) * 0.5;
        frame
            .painter()
            .text(crate::core::Position::new(tx, ty), &self.label, &ts);
    }
}
