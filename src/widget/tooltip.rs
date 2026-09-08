//! Tooltip widget — displays a hover tip that wraps inner content.

use crate::core::{Color, Position, Rect, Style};
use crate::ontology::*;
use crate::runtime::Frame;
use crate::widget::Widget;

/// A tooltip that wraps a label and shows hover text.
///
/// Renders the label inline, and the tip over everything else while the
/// pointer is inside the label's bounds.
///
/// This used to say "visual tooltip popups are handled by the backend". None
/// was: the tip text was held, published to the ontology, and drawn by
/// nothing — so an agent could read a tooltip that no person could ever
/// see.
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
        let label = if self.label.is_empty() {
            "(?)"
        } else {
            &self.label
        };
        let ts = self.style.resolved_text();
        frame
            .painter()
            .text(Position::new(area.x, area.y), label, &ts);

        // The tip, while the pointer is on the label. Deferred, because a tip
        // that paints here is painted under whatever the view renders next —
        // which for a tooltip, whose whole job is to sit on top, is every
        // time.
        if frame.hovered(area) && !self.text.is_empty() {
            let text = self.text.clone();
            let anchor = Position::new(area.x, area.y + area.height + 4.0);
            let fg = self.style.foreground.unwrap_or(Color::WHITE);
            frame.overlay(move |frame| {
                let tip_ts = crate::core::style::TextStyle {
                    color: fg,
                    ..Default::default()
                };
                let size = frame.painter().measure_text(&text, &tip_ts);
                let box_rect = Rect::new(anchor.x, anchor.y, size.width + 12.0, size.height + 8.0);
                frame
                    .painter()
                    .fill_rect(box_rect, Color::BLACK.with_alpha(0.85), 4.0);
                frame.painter().stroke_rect(box_rect, Color::GRAY, 1.0, 4.0);
                frame.painter().text(
                    Position::new(box_rect.x + 6.0, box_rect.y + 4.0),
                    &text,
                    &tip_ts,
                );
            });
        }

        // Built last so owned fields (text, item vectors) move into the
        // state instead of being cloned; painting above only borrows them.
        if frame.describes(area) && !self.agent_id.is_empty() {
            let node = UiNode::new("Tooltip", SemanticRole::Display)
                .with_id(self.agent_id.clone())
                .with_bounds(area.into())
                .with_property("label", serde_json::Value::from(self.label))
                .with_property("text", serde_json::Value::from(self.text))
                // Whether the tip is on screen right now, which is a different
                // fact from whether the widget has one.
                .with_property("showing", serde_json::json!(frame.hovered(area)));
            frame.register_widget(node);
        }
    }
}
