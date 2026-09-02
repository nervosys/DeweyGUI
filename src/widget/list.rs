//! List widget — a vertical list of selectable items.

use crate::core::{Color, Position, Rect, Style};
use crate::ontology::*;
use crate::runtime::Frame;
use crate::widget::StatefulWidget;

/// List state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ListState {
    pub selected: Option<usize>,
    pub offset: usize,
}

impl ListState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            selected: None,
            offset: 0,
        }
    }

    pub fn with_selected(mut self, index: usize) -> Self {
        self.selected = Some(index);
        self
    }

    pub fn select_next(&mut self, len: usize) {
        if len == 0 {
            return;
        }
        self.selected = Some(match self.selected {
            Some(i) => (i + 1).min(len - 1),
            None => 0,
        });
    }

    pub fn select_prev(&mut self) {
        self.selected = self.selected.map(|i| i.saturating_sub(1));
    }
}

/// A vertical list of items.
pub struct List {
    items: Vec<String>,
    style: Style,
    agent_id: std::borrow::Cow<'static, str>,
    on_value: Option<Box<dyn std::any::Any + Send>>,
}

impl List {
    #[must_use]
    pub fn new(items: Vec<String>) -> Self {
        Self {
            items,
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

    /// Name this widget and give it the change to apply on `select`.
    ///
    /// Bound to the action `List` advertises, so an agent following the
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

impl Discoverable for List {
    fn schema(&self) -> WidgetSchema {
        let mut schema = WidgetSchema::new(
            "List",
            "A vertical list of selectable items",
            SemanticRole::Selection,
        );
        schema.usage_hint = Some("List::new(vec![\"Item 1\".into(), \"Item 2\".into()])".into());
        schema.tags = vec!["list".into(), "items".into(), "selection".into()];
        schema
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![
            AgentCapability::Selectable {
                multi_select: false,
                item_count: self.items.len(),
            },
            AgentCapability::Scrollable {
                vertical: true,
                horizontal: false,
            },
            AgentCapability::Focusable,
        ]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![AgentAction::with_params(
            "select",
            "Select an item by index",
            vec![ActionParam::required(
                "index",
                "The index to select",
                ActionParamType::Index,
            )],
            true,
        )]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::Selection
    }

    fn agent_state(&self) -> serde_json::Value {
        serde_json::json!({ "items": self.items, "count": self.items.len() })
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
        Some(format!("List ({} items)", self.items.len()))
    }
}

impl StatefulWidget for List {
    type State = ListState;

    fn render(mut self, area: Rect, frame: &mut Frame<'_>, state: &mut ListState) {
        if !self.agent_id.is_empty() {
            frame.register_hitbox(self.agent_id.clone(), area, 1);
            if let Some(handler) = self.on_value.take() {
                frame.register_message(self.agent_id.clone(), "select", handler);
            }
        }

        frame.painter().push_clip(area);
        let item_h = 24.0;
        let ts = self.style.resolved_text();
        for (i, item) in self.items.iter().enumerate() {
            let y = area.y + i as f32 * item_h;
            let row = Rect::new(area.x, y, area.width, item_h);
            if state.selected == Some(i) {
                frame
                    .painter()
                    .fill_rect(row, Color::BLUE.with_alpha(0.3), 0.0);
            }
            frame
                .painter()
                .text(Position::new(area.x + 4.0, y + 4.0), item, &ts);
        }
        frame.painter().pop_clip();

        // Built last so owned fields move into the state instead of being
        // cloned; painting above only borrows them.
        if frame.describes(area) && !self.agent_id.is_empty() {
            let node = UiNode::new("List", SemanticRole::Selection)
                .with_id(self.agent_id.clone())
                .with_bounds(area.into())
                .with_property("items", serde_json::Value::from(self.items))
                .with_property("selected", serde_json::json!(state.selected));
            frame.register_widget(node);
        }
    }
}
