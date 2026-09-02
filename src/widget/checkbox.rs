//! Checkbox widget — a toggleable boolean control.

use crate::core::{Color, Position, Rect, Style};
use crate::ontology::*;
use crate::runtime::Frame;
use crate::widget::Widget;

/// A checkbox with a label.
pub struct Checkbox {
    label: String,
    checked: bool,
    style: Style,
    agent_id: std::borrow::Cow<'static, str>,
    on_toggle: Option<Box<dyn std::any::Any + Send>>,
}

impl Checkbox {
    #[must_use]
    pub fn new(label: impl Into<String>, checked: bool) -> Self {
        Self {
            label: label.into(),
            checked,
            style: Style::default(),
            agent_id: std::borrow::Cow::Borrowed(""),
            on_toggle: None,
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

    /// Name this checkbox and give it the message to send when it is toggled.
    ///
    /// See [`Button::action`](crate::widget::Button::action) — one call wires
    /// the widget for both a person and an agent.
    #[must_use]
    pub fn action<T: std::any::Any + Send>(
        mut self,
        id: impl Into<std::borrow::Cow<'static, str>>,
        msg: T,
    ) -> Self {
        self.agent_id = id.into();
        self.on_toggle = Some(Box::new(msg));
        self
    }

    /// Name this checkbox and give it a change to apply when toggled.
    ///
    /// The shorter form of [`Checkbox::action`] for an application that does not
    /// want a message type: no `Msg` variant and no `update` arm, driven by a
    /// person and an agent the same way.
    ///
    /// ```no_run
    /// # use dewey::prelude::*;
    /// # struct App { count: i32 }
    /// Checkbox::new("done", false).on("done", |app: &mut App| app.count += 1);
    /// ```
    ///
    /// Use [`Checkbox::action`] instead when the change must return a
    /// [`Command`](crate::runtime::Command).
    #[must_use]
    pub fn on<M: 'static>(
        mut self,
        id: impl Into<std::borrow::Cow<'static, str>>,
        change: impl FnOnce(&mut M) + Send + 'static,
    ) -> Self {
        let boxed: crate::runtime::Mutation<M> = Box::new(change);
        self.agent_id = id.into();
        self.on_toggle = Some(Box::new(boxed));
        self
    }

    pub fn agent_id(mut self, id: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        self.agent_id = id.into();
        self
    }
}

impl Discoverable for Checkbox {
    fn schema(&self) -> WidgetSchema {
        let mut schema =
            WidgetSchema::new("Checkbox", "A toggleable checkbox", SemanticRole::Input);
        schema.usage_hint = Some("Checkbox::new(\"Accept terms\", false)".into());
        schema.tags = vec!["checkbox".into(), "toggle".into(), "boolean".into()];
        schema
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![
            AgentCapability::Toggleable {
                state: self.checked,
            },
            AgentCapability::Focusable,
        ]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![AgentAction::simple("toggle", "Toggle the checkbox", true)]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::Input
    }

    fn agent_state(&self) -> serde_json::Value {
        serde_json::json!({ "checked": self.checked, "label": self.label })
    }

    fn execute_action(
        &mut self,
        action: &str,
        _params: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        match action {
            "toggle" => {
                self.checked = !self.checked;
                Ok(serde_json::json!({ "checked": self.checked }))
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

impl Widget for Checkbox {
    fn render(mut self, area: Rect, frame: &mut Frame<'_>) {
        if self.agent_id.is_empty() {
            frame.note_unaddressable("Checkbox");
        } else {
            frame.register_hitbox(self.agent_id.clone(), area, 1);
            if let Some(msg) = self.on_toggle.take() {
                frame.register_message(self.agent_id.clone(), "toggle", msg);
            }
        }

        let box_size = 16.0;
        let box_rect = Rect::new(area.x, area.y + 2.0, box_size, box_size);
        frame
            .painter()
            .stroke_rect(box_rect, Color::WHITE, 1.0, 2.0);
        if self.checked {
            let inner = Rect::new(area.x + 3.0, area.y + 5.0, box_size - 6.0, box_size - 6.0);
            frame.painter().fill_rect(inner, Color::WHITE, 1.0);
        }
        let ts = self.style.resolved_text();
        frame.painter().text(
            Position::new(area.x + box_size + 6.0, area.y + 2.0),
            &self.label,
            &ts,
        );

        // Built last so owned fields (text, item vectors) move into the
        // state instead of being cloned; painting above only borrows them.
        if frame.ontology_enabled() && !self.agent_id.is_empty() {
            let node = UiNode::new("Checkbox", SemanticRole::Input)
                .with_id(self.agent_id.clone())
                .with_bounds(area.into())
                .with_property("checked", serde_json::json!(self.checked))
                .with_property("label", serde_json::Value::from(self.label));
            frame.register_widget(node);
        }
    }
}
