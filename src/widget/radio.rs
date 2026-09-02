//! Radio button widget.

use crate::core::{Color, Position, Rect, Style};
use crate::ontology::*;
use crate::runtime::Frame;
use crate::widget::Widget;

/// A radio button with a label.
pub struct Radio {
    label: String,
    selected: bool,
    style: Style,
    agent_id: std::borrow::Cow<'static, str>,
    on_value: Option<Box<dyn std::any::Any + Send>>,
}

impl Radio {
    #[must_use]
    pub fn new(label: impl Into<String>, selected: bool) -> Self {
        Self {
            label: label.into(),
            selected,
            style: Style::default(),
            agent_id: std::borrow::Cow::Borrowed(""),
            on_value: None,
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

    /// Name this widget and give it the change to apply on `select`.
    ///
    /// Bound to the action `Radio` advertises, so an agent following the
    /// ontology reaches it and the application writes no
    /// `execute_action` handler.
    #[must_use]
    pub fn on_select<M: 'static>(
        mut self,
        id: impl Into<std::borrow::Cow<'static, str>>,
        f: impl FnOnce(&mut M) + Send + 'static,
    ) -> Self {
        let wrapped: crate::runtime::ValueMutation<M> =
            Box::new(move |m: &mut M, _v: &serde_json::Value| f(m));
        self.agent_id = id.into();
        self.on_value = Some(Box::new(wrapped));
        self
    }

    pub fn agent_id(mut self, id: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        self.agent_id = id.into();
        self
    }
}

impl Discoverable for Radio {
    fn schema(&self) -> WidgetSchema {
        let mut schema = WidgetSchema::new("Radio", "A radio button", SemanticRole::Input);
        schema.usage_hint = Some("Radio::new(\"Option A\", false)".into());
        schema.tags = vec!["radio".into(), "option".into(), "choice".into()];
        schema
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![
            AgentCapability::Selectable {
                multi_select: false,
                item_count: 1,
            },
            AgentCapability::Focusable,
        ]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![AgentAction::simple(
            "select",
            "Select this radio button",
            true,
        )]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::Input
    }

    fn agent_state(&self) -> serde_json::Value {
        serde_json::json!({ "selected": self.selected, "label": self.label })
    }

    fn execute_action(
        &mut self,
        action: &str,
        _params: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        match action {
            "select" => {
                self.selected = true;
                Ok(serde_json::json!({ "selected": true }))
            }
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

impl Widget for Radio {
    fn render(mut self, area: Rect, frame: &mut Frame<'_>) {
        if !self.agent_id.is_empty() {
            if frame.describes(area) {
                let node = UiNode::new("Radio", SemanticRole::Input)
                    .with_id(self.agent_id.clone())
                    .with_bounds(area.into())
                    .with_property("selected", serde_json::json!(self.selected));
                frame.register_widget(node);
            }
            frame.register_hitbox(self.agent_id.clone(), area, 1);
            if let Some(handler) = self.on_value.take() {
                frame.register_message(self.agent_id.clone(), "select", handler);
            }
        }

        let radius = 8.0;
        let center = Position::new(area.x + radius, area.y + radius + 2.0);
        frame
            .painter()
            .stroke_circle(center, radius, Color::WHITE, 1.0);
        if self.selected {
            frame
                .painter()
                .fill_circle(center, radius - 3.0, Color::WHITE);
        }
        let ts = self.style.resolved_text();
        frame.painter().text(
            Position::new(area.x + radius * 2.0 + 6.0, area.y + 2.0),
            &self.label,
            &ts,
        );
    }
}
