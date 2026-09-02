//! Container widget — a layout container with optional styling.

use crate::core::style::TextStyle;
use crate::core::{Color, Position, Rect, Style};
use crate::ontology::*;
use crate::runtime::Frame;
use crate::widget::Widget;

/// A styled container that holds child content.
///
/// # Examples
///
/// ```
/// # use dewey::prelude::*;
/// Container::new()
///     .bg(Color::DARK_GRAY)
///     .rounded(12.0)
///     .border(Color::GRAY, 1.0);
/// ```
pub struct Container {
    style: Style,
    title: Option<String>,
    agent_id: std::borrow::Cow<'static, str>,
}

impl Container {
    #[must_use]
    pub fn new() -> Self {
        Self {
            style: Style::default(),
            title: None,
            agent_id: std::borrow::Cow::Borrowed(""),
        }
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn bg(mut self, color: Color) -> Self {
        self.style.background = Some(color);
        self
    }

    pub fn fg(mut self, color: Color) -> Self {
        self.style.foreground = Some(color);
        self
    }

    pub fn rounded(mut self, radius: f32) -> Self {
        self.style.border_radius = Some(radius);
        self
    }

    pub fn border(mut self, color: Color, width: f32) -> Self {
        self.style.border_color = Some(color);
        self.style.border_width = Some(width);
        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn agent_id(mut self, id: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        self.agent_id = id.into();
        self
    }
}

impl Default for Container {
    fn default() -> Self {
        Self::new()
    }
}

impl Discoverable for Container {
    fn schema(&self) -> WidgetSchema {
        let mut schema = WidgetSchema::new(
            "Container",
            "A styled layout container",
            SemanticRole::Container,
        );
        schema.usage_hint = Some("Container::new().title(\"Section\")".into());
        schema.tags = vec!["container".into(), "layout".into(), "group".into()];
        schema
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::Container
    }

    fn agent_state(&self) -> serde_json::Value {
        serde_json::json!({ "title": self.title })
    }

    fn agent_id(&self) -> Option<&str> {
        if self.agent_id.is_empty() {
            None
        } else {
            Some(&self.agent_id)
        }
    }

    fn accessibility_label(&self) -> Option<String> {
        self.title.clone()
    }
}

impl Widget for Container {
    fn render(self, area: Rect, frame: &mut Frame<'_>) {
        if frame.describes(area) && !self.agent_id.is_empty() {
            let mut node = UiNode::new("Container", SemanticRole::Container)
                .with_id(self.agent_id.clone())
                .with_bounds(area.into());
            if let Some(ref title) = self.title {
                node = node.with_property("title", serde_json::json!(title));
            }
            frame.register_widget(node);
        }

        let bg = self.style.background.unwrap_or(Color::TRANSPARENT);
        let radius = self.style.border_radius.unwrap_or(0.0);
        if bg.a > 0.0 {
            frame.painter().fill_rect(area, bg, radius);
        }
        // Only draw border if explicitly specified
        if let (Some(border), Some(border_w)) = (self.style.border_color, self.style.border_width) {
            if border.a > 0.0 && border_w > 0.0 {
                frame.painter().stroke_rect(area, border, border_w, radius);
            }
        }
        if let Some(title) = &self.title {
            let ts = TextStyle {
                font_size: 18.0,
                color: Color::WHITE,
                ..Default::default()
            };
            frame
                .painter()
                .text(Position::new(area.x + 4.0, area.y + 4.0), title, &ts);
        }
    }
}
