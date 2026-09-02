//! Tabs widget — a tabbed panel selector.

use crate::core::{Color, Position, Rect, Style};
use crate::ontology::*;
use crate::runtime::Frame;
use crate::widget::StatefulWidget;

/// Tab state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TabState {
    pub selected: usize,
}

impl TabState {
    #[must_use]
    pub fn new() -> Self {
        Self { selected: 0 }
    }

    pub fn with_selected(mut self, index: usize) -> Self {
        self.selected = index;
        self
    }
}

/// A tab bar.
pub struct Tabs {
    labels: Vec<String>,
    style: Style,
    agent_id: std::borrow::Cow<'static, str>,
    on_value: Option<Box<dyn std::any::Any + Send>>,
}

impl Tabs {
    #[must_use]
    pub fn new(labels: Vec<String>) -> Self {
        Self {
            labels,
            style: Style::default(),
            agent_id: std::borrow::Cow::Borrowed(""),
            on_value: None,
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

    /// Name this widget and give it the change to apply on `select_tab`.
    ///
    /// Bound to the action `Tabs` advertises, so an agent following the
    /// ontology reaches it and the application writes no
    /// `execute_action` handler.
    #[must_use]
    pub fn on_select<M: 'static>(
        mut self,
        id: impl Into<std::borrow::Cow<'static, str>>,
        f: impl FnOnce(&mut M, usize) + Send + 'static,
    ) -> Self {
        let wrapped: crate::runtime::ValueMutation<M> =
            Box::new(move |m: &mut M, v: &serde_json::Value| {
                f(
                    m,
                    v.get("index")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0) as usize,
                )
            });
        self.agent_id = id.into();
        self.on_value = Some(Box::new(wrapped));
        self
    }

    pub fn agent_id(mut self, id: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        self.agent_id = id.into();
        self
    }
}

impl Discoverable for Tabs {
    fn schema(&self) -> WidgetSchema {
        let mut schema =
            WidgetSchema::new("Tabs", "A tab bar for switching views", SemanticRole::Tab);
        schema.usage_hint = Some("Tabs::new(vec![\"Home\".into(), \"Settings\".into()])".into());
        schema.tags = vec!["tabs".into(), "navigation".into(), "switch".into()];
        schema
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![
            AgentCapability::Selectable {
                multi_select: false,
                item_count: self.labels.len(),
            },
            AgentCapability::Focusable,
        ]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![AgentAction::with_params(
            "select_tab",
            "Select a tab by index",
            vec![ActionParam::required(
                "index",
                "Tab index",
                ActionParamType::Index,
            )],
            true,
        )]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::Tab
    }

    fn agent_state(&self) -> serde_json::Value {
        serde_json::json!({ "labels": self.labels })
    }

    fn execute_action(
        &mut self,
        _action: &str,
        _params: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        Err("Use StatefulWidget for state mutations".to_string())
    }

    fn agent_id(&self) -> Option<&str> {
        if self.agent_id.is_empty() {
            None
        } else {
            Some(&self.agent_id)
        }
    }

    fn accessibility_label(&self) -> Option<String> {
        Some(self.labels.join(", "))
    }
}

impl StatefulWidget for Tabs {
    type State = TabState;

    fn render(mut self, area: Rect, frame: &mut Frame<'_>, state: &mut TabState) {
        if !self.agent_id.is_empty() {
            frame.register_hitbox(self.agent_id.clone(), area, 1);
            if let Some(handler) = self.on_value.take() {
                frame.register_message(self.agent_id.clone(), "select_tab", handler);
            }
        }

        let tab_h = 28.0;
        let ts = self.style.resolved_text();
        let active_bg = self.style.background.unwrap_or(Color::DARK_GRAY);
        let mut x = area.x;
        for (i, label) in self.labels.iter().enumerate() {
            let text_w = frame.painter().measure_text(label, &ts).width + 16.0;
            let tab_rect = Rect::new(x, area.y, text_w, tab_h);
            if state.selected == i {
                frame.painter().fill_rect(tab_rect, active_bg, 4.0);
            }
            frame
                .painter()
                .text(Position::new(x + 8.0, area.y + 6.0), label, &ts);
            x += text_w;
        }
        // Bottom border
        frame.painter().line(
            Position::new(area.x, area.y + tab_h),
            Position::new(area.x + area.width, area.y + tab_h),
            Color::GRAY,
            1.0,
        );

        // Built last so owned fields move into the state instead of being
        // cloned; painting above only borrows them.
        if frame.ontology_enabled() && !self.agent_id.is_empty() {
            let node = UiNode::new("Tabs", SemanticRole::Tab)
                .with_id(self.agent_id.clone())
                .with_bounds(area.into())
                .with_property("labels", serde_json::Value::from(self.labels))
                .with_property("selected", serde_json::json!(state.selected));
            frame.register_widget(node);
        }
    }
}
