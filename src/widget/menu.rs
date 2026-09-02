//! Menu widget — menus and menu items.

use crate::core::{Color, Position, Rect, Style};
use crate::ontology::*;
use crate::runtime::Frame;
use crate::widget::Widget;

/// A single menu item.
#[derive(Debug, Clone)]
pub struct MenuItem {
    pub label: String,
    pub shortcut: Option<String>,
    pub enabled: bool,
}

impl MenuItem {
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            shortcut: None,
            enabled: true,
        }
    }

    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// A menu containing items.
pub struct Menu {
    title: String,
    items: Vec<MenuItem>,
    style: Style,
    agent_id: std::borrow::Cow<'static, str>,
    on_value: Option<Box<dyn std::any::Any + Send>>,
}

impl Menu {
    #[must_use]
    pub fn new(title: impl Into<String>, items: Vec<MenuItem>) -> Self {
        Self {
            title: title.into(),
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

    /// Name this widget and give it the change to apply on `select_item`.
    ///
    /// Bound to the action `Menu` advertises, so an agent following the
    /// ontology reaches it and the application writes no
    /// `execute_action` handler.
    #[must_use]
    pub fn on_item<M: 'static>(
        mut self,
        id: impl Into<std::borrow::Cow<'static, str>>,
        f: impl FnOnce(&mut M, &str) + Send + 'static,
    ) -> Self {
        let wrapped: crate::runtime::ValueMutation<M> =
            Box::new(move |m: &mut M, v: &serde_json::Value| {
                f(m, v.get("label").and_then(|t| t.as_str()).unwrap_or(""))
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

impl Discoverable for Menu {
    fn schema(&self) -> WidgetSchema {
        let mut schema = WidgetSchema::new("Menu", "A dropdown menu", SemanticRole::Menu);
        schema.usage_hint = Some("Menu::new(\"File\", vec![MenuItem::new(\"Open\")])".into());
        schema.tags = vec!["menu".into(), "dropdown".into(), "navigation".into()];
        schema
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![
            AgentCapability::Selectable {
                multi_select: false,
                item_count: self.items.len(),
            },
            AgentCapability::Focusable,
        ]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![AgentAction::with_params(
            "select_item",
            "Select a menu item",
            vec![ActionParam::required(
                "label",
                "Menu item label",
                ActionParamType::String,
            )],
            true,
        )]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::Menu
    }

    fn agent_state(&self) -> serde_json::Value {
        let items: Vec<_> = self.items.iter().map(|i| &i.label).collect();
        serde_json::json!({ "title": self.title, "items": items })
    }

    fn agent_id(&self) -> Option<&str> {
        if self.agent_id.is_empty() {
            None
        } else {
            Some(&self.agent_id)
        }
    }

    fn accessibility_label(&self) -> Option<String> {
        Some(self.title.clone())
    }
}

impl Widget for Menu {
    fn render(mut self, area: Rect, frame: &mut Frame<'_>) {
        // Menu bar background
        let bar_h = 28.0;
        let bar = Rect::new(area.x, area.y, area.width, bar_h);
        let bar_bg = self.style.background.unwrap_or(Color::DARK_GRAY);
        frame.painter().fill_rect(bar, bar_bg, 0.0);
        let ts = self.style.resolved_text();
        frame
            .painter()
            .text(Position::new(area.x + 8.0, area.y + 6.0), &self.title, &ts);

        // Built last so owned fields (text, item vectors) move into the
        // state instead of being cloned; painting above only borrows them.
        if frame.describes(area) && !self.agent_id.is_empty() {
            let node = UiNode::new("Menu", SemanticRole::Menu)
                .with_id(self.agent_id.clone())
                .with_bounds(area.into())
                .with_property("title", serde_json::Value::from(self.title));
            frame.register_widget(node);
            if let Some(handler) = self.on_value.take() {
                frame.register_message(self.agent_id.clone(), "select_item", handler);
            }
        }
    }
}
