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
    open: bool,
    on_open: Option<Box<dyn std::any::Any + Send>>,
}

/// The height of the menu bar, and of one item in the open list.
const BAR_HEIGHT: f32 = 28.0;
const ITEM_HEIGHT: f32 = 24.0;

impl Menu {
    #[must_use]
    pub fn new(title: impl Into<String>, items: Vec<MenuItem>) -> Self {
        Self {
            title: title.into(),
            items,
            style: Style::default(),
            agent_id: std::borrow::Cow::Borrowed(""),
            on_value: None,
            open: false,
            on_open: None,
        }
    }

    /// Whether the item list is showing.
    ///
    /// The application owns it, as it owns a `Modal`'s and a `Select`'s.
    #[must_use]
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Give this menu the change to apply when it is opened or closed.
    ///
    /// The bool is the state it should move to. Without this a menu cannot be
    /// opened by a person: it drew a bar with a title, the items were never
    /// painted, and clicking the bar had nothing to fire.
    #[must_use]
    pub fn on_open<M: 'static>(
        mut self,
        id: impl Into<std::borrow::Cow<'static, str>>,
        f: impl FnOnce(&mut M, bool) + Send + 'static,
    ) -> Self {
        let open = !self.open;
        let wrapped: crate::runtime::Mutation<M> = Box::new(move |m: &mut M| f(m, open));
        self.agent_id = id.into();
        self.on_open = Some(Box::new(wrapped));
        self
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
        vec![
            AgentAction::with_params(
                "select_item",
                "Select a menu item",
                vec![ActionParam::required(
                    "label",
                    "Menu item label",
                    ActionParamType::String,
                )],
                true,
            ),
            AgentAction::simple("toggle_open", "Show or hide the item list", true),
        ]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::Menu
    }

    fn agent_state(&self) -> serde_json::Value {
        let items: Vec<_> = self.items.iter().map(|i| &i.label).collect();
        serde_json::json!({ "title": self.title, "items": items, "open": self.open })
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
        let bar = Rect::new(area.x, area.y, area.width, BAR_HEIGHT);
        // The list drops from the bar. Width follows the longest label rather
        // than the bar, which is usually the whole window.
        let list = Rect::new(
            bar.x,
            bar.y + bar.height,
            (bar.width * 0.4).max(120.0),
            self.items.len() as f32 * ITEM_HEIGHT,
        );

        if !self.agent_id.is_empty() {
            frame.register_hitbox(self.agent_id.clone(), bar, 1);
            // Outside `describes`: a handler registered only on frames that
            // build the ontology tree is a handler that does not exist on an
            // ordinary frame, and the default backend builds the tree only
            // when an agent asks. A click on this menu reached nothing.
            if let Some(handler) = self.on_value.take() {
                frame.register_message(self.agent_id.clone(), "select_item", handler);
            }
            if let Some(handler) = self.on_open.take() {
                frame.register_message(self.agent_id.clone(), "toggle_open", handler);
            }

            let open = self.open;
            let labels: Vec<(String, bool)> = self
                .items
                .iter()
                .map(|i| (i.label.clone(), i.enabled))
                .collect();
            frame.register_click(
                self.agent_id.clone(),
                crate::runtime::ClickParams::from_position(move |at| {
                    if bar.contains(at) {
                        return Some(crate::runtime::Click::action(
                            "toggle_open",
                            serde_json::Value::Null,
                        ));
                    }
                    if !open || !list.contains(at) {
                        return None;
                    }
                    let row = ((at.y - list.y) / ITEM_HEIGHT).floor();
                    if row < 0.0 {
                        return None;
                    }
                    let (label, enabled) = labels.get(row as usize)?;
                    // A greyed item is drawn as unavailable and behaves so.
                    if !*enabled {
                        return None;
                    }
                    Some(crate::runtime::Click::action(
                        "select_item",
                        serde_json::json!({ "label": label }),
                    ))
                }),
            );
        }

        let bar_bg = self.style.background.unwrap_or(Color::DARK_GRAY);
        frame.painter().fill_rect(bar, bar_bg, 0.0);
        let ts = self.style.resolved_text();
        frame
            .painter()
            .text(Position::new(bar.x + 8.0, bar.y + 6.0), &self.title, &ts);

        if frame.describes(area) && !self.agent_id.is_empty() {
            let items: Vec<serde_json::Value> = self
                .items
                .iter()
                .map(|i| {
                    serde_json::json!({
                        "label": i.label,
                        "shortcut": i.shortcut,
                        "enabled": i.enabled,
                    })
                })
                .collect();
            let node = UiNode::new("Menu", SemanticRole::Menu)
                .with_id(self.agent_id.clone())
                .with_bounds(area.into())
                .with_property("title", serde_json::Value::from(self.title.clone()))
                .with_property("open", serde_json::json!(self.open))
                // The items were held, advertised through `select_item`, and
                // described to nobody: the node carried a title and nothing
                // else, so an agent could not learn what there was to pick.
                .with_property("items", serde_json::Value::Array(items));
            frame.register_widget(node);
        }

        // Drawn after every other widget: a menu drops over what is below it,
        // and painting here would put it under whatever renders next.
        if self.open && !self.items.is_empty() {
            let items = std::mem::take(&mut self.items);
            let id = self.agent_id.clone();
            let bg = self.style.background.unwrap_or(Color::DARK_GRAY);
            frame.overlay(move |frame| {
                frame.painter().fill_rect(list, bg, 4.0);
                frame.painter().stroke_rect(list, Color::GRAY, 1.0, 4.0);
                let ts = crate::core::style::TextStyle::default();
                let mut greyed = ts.clone();
                greyed.color = Color::GRAY;
                for (i, item) in items.iter().enumerate() {
                    let y = list.y + i as f32 * ITEM_HEIGHT;
                    let style = if item.enabled { &ts } else { &greyed };
                    frame
                        .painter()
                        .text(Position::new(list.x + 8.0, y + 4.0), &item.label, style);
                    if let Some(shortcut) = &item.shortcut {
                        frame.painter().text(
                            Position::new(list.x + list.width - 48.0, y + 4.0),
                            shortcut,
                            &greyed,
                        );
                    }
                }
                if !id.is_empty() {
                    frame.register_hitbox(id, list, 1);
                }
            });
        }
    }
}
