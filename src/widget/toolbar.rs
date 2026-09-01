//! Toolbar widget — a horizontal row of action buttons.

use crate::core::{Color, Position, Rect, Style};
use crate::ontology::*;
use crate::runtime::Frame;
use crate::widget::Widget;

/// A single toolbar item.
#[derive(Debug, Clone)]
pub struct ToolbarItem {
    /// Unique identifier for this item.
    pub id: String,
    /// Display label.
    pub label: String,
    /// Icon name (semantic, for agent interpretation).
    pub icon: Option<String>,
    /// Whether the item is enabled.
    pub enabled: bool,
    /// Whether the item is toggled on.
    pub toggled: bool,
}

impl ToolbarItem {
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            enabled: true,
            toggled: false,
        }
    }

    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn toggled(mut self, toggled: bool) -> Self {
        self.toggled = toggled;
        self
    }
}

/// A horizontal toolbar of action buttons.
pub struct Toolbar {
    items: Vec<ToolbarItem>,
    style: Style,
    agent_id: std::borrow::Cow<'static, str>,
}

impl Toolbar {
    #[must_use]
    pub fn new(items: Vec<ToolbarItem>) -> Self {
        Self {
            items,
            style: Style::default(),
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

    pub fn agent_id(mut self, id: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        self.agent_id = id.into();
        self
    }
}

impl Discoverable for Toolbar {
    fn schema(&self) -> WidgetSchema {
        let mut schema = WidgetSchema::new(
            "Toolbar",
            "A horizontal row of action buttons",
            SemanticRole::Navigation,
        );
        schema.usage_hint = Some("Toolbar::new(vec![ToolbarItem::new(\"save\", \"Save\")])".into());
        schema.tags = vec!["toolbar".into(), "actions".into(), "buttons".into()];
        schema
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![AgentCapability::Clickable, AgentCapability::Focusable]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![
            AgentAction::with_params(
                "click_item",
                "Click a toolbar item by ID",
                vec![ActionParam::required(
                    "item_id",
                    "ID of the toolbar item to click",
                    ActionParamType::String,
                )],
                false,
            ),
            AgentAction::simple("list_items", "List all toolbar items", false),
        ]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::Navigation
    }

    fn agent_state(&self) -> serde_json::Value {
        let items: Vec<_> = self
            .items
            .iter()
            .map(|i| {
                serde_json::json!({
                    "id": i.id,
                    "label": i.label,
                    "icon": i.icon,
                    "enabled": i.enabled,
                    "toggled": i.toggled,
                })
            })
            .collect();
        serde_json::json!({ "items": items })
    }

    fn execute_action(
        &mut self,
        action: &str,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        match action {
            "click_item" => {
                let item_id = params
                    .get("item_id")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing item_id")?;
                match self.items.iter().find(|i| i.id == item_id) {
                    Some(item) if item.enabled => Ok(serde_json::json!({ "clicked": item_id })),
                    Some(_) => Err(format!("Item '{item_id}' is disabled")),
                    None => Err(format!("Unknown item: {item_id}")),
                }
            }
            "list_items" => Ok(self.agent_state()),
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
        let labels: Vec<&str> = self.items.iter().map(|i| i.label.as_str()).collect();
        Some(format!("Toolbar: {}", labels.join(", ")))
    }
}

impl Widget for Toolbar {
    fn render(self, area: Rect, frame: &mut Frame<'_>) {
        if !self.agent_id.is_empty() {
            if frame.ontology_enabled() {
                let mut node = UiNode::new("Toolbar", SemanticRole::Navigation)
                    .with_id(self.agent_id.clone())
                    .with_bounds(area.into());

                let items_json: Vec<_> = self
                    .items
                    .iter()
                    .map(|i| {
                        serde_json::json!({
                            "id": i.id,
                            "label": i.label,
                            "enabled": i.enabled,
                            "toggled": i.toggled,
                        })
                    })
                    .collect();
                node = node.with_property("items", serde_json::json!(items_json));
                frame.register_widget(node);
            }
            frame.register_hitbox(self.agent_id.clone(), area, 1);
        }

        // Toolbar background
        let toolbar_bg = self.style.background.unwrap_or(Color::DARK_GRAY);
        frame.painter().fill_rect(area, toolbar_bg, 0.0);

        let ts = self.style.resolved_text();
        let mut disabled_ts = ts.clone();
        disabled_ts.color = Color::GRAY;
        let mut x = area.x + 4.0;
        for item in &self.items {
            let style = if item.enabled { &ts } else { &disabled_ts };
            let sz = frame.painter().measure_text(&item.label, style);
            let btn_w = sz.width + 16.0;
            let btn_rect = Rect::new(x, area.y + 2.0, btn_w, area.height - 4.0);
            if item.toggled {
                frame
                    .painter()
                    .fill_rect(btn_rect, Color::from_rgba8(60, 60, 120, 255), 4.0);
            }
            frame.painter().text(
                Position::new(x + 8.0, area.y + (area.height - sz.height) * 0.5),
                &item.label,
                style,
            );
            x += btn_w + 4.0;
        }
    }
}
