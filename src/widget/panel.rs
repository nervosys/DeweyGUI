//! Panel widget — a framed section of the UI.

use crate::core::{Color, Position, Rect, Style};
use crate::ontology::*;
use crate::runtime::Frame;
use crate::widget::Widget;

/// Panel position within a layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelSide {
    Left,
    Right,
    Top,
    Bottom,
    Center,
}

/// A panel that occupies a specific region.
pub struct Panel {
    side: PanelSide,
    title: Option<String>,
    style: Style,
    agent_id: std::borrow::Cow<'static, str>,
}

impl Panel {
    #[must_use]
    pub fn new(side: PanelSide) -> Self {
        Self {
            side,
            title: None,
            style: Style::default(),
            agent_id: std::borrow::Cow::Borrowed(""),
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
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

    pub fn rounded(mut self, radius: f32) -> Self {
        self.style.border_radius = Some(radius);
        self
    }

    pub fn agent_id(mut self, id: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        self.agent_id = id.into();
        self
    }
}

impl Discoverable for Panel {
    fn schema(&self) -> WidgetSchema {
        let mut schema =
            WidgetSchema::new("Panel", "A framed panel region", SemanticRole::Container);
        schema.usage_hint = Some("Panel::new(PanelSide::Left).title(\"Explorer\")".into());
        schema.tags = vec!["panel".into(), "region".into(), "layout".into()];
        schema
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![AgentCapability::Resizable {
            min_width: None,
            min_height: None,
            max_width: None,
            max_height: None,
        }]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::Container
    }

    fn agent_state(&self) -> serde_json::Value {
        serde_json::json!({ "side": format!("{:?}", self.side), "title": self.title })
    }

    fn execute_action(
        &mut self,
        _action: &str,
        _params: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        Err("Panel has no actions".to_string())
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

impl Widget for Panel {
    fn render(self, area: Rect, frame: &mut Frame<'_>) {
        if frame.ontology_enabled() && !self.agent_id.is_empty() {
            let node = UiNode::new("Panel", SemanticRole::Container)
                .with_id(self.agent_id.clone())
                .with_bounds(area.into())
                .with_property("side", serde_json::json!(format!("{:?}", self.side)));
            frame.register_widget(node);
        }

        let bg = self.style.background.unwrap_or(Color::DARK_GRAY);
        let radius = self.style.border_radius.unwrap_or(0.0);
        frame.painter().fill_rect(area, bg, radius);
        if let (Some(bc), Some(bw)) = (self.style.border_color, self.style.border_width) {
            frame.painter().stroke_rect(area, bc, bw, radius);
        } else {
            frame.painter().stroke_rect(area, Color::GRAY, 1.0, radius);
        }
        if let Some(title) = &self.title {
            let mut ts = self.style.resolved_text();
            if ts.font_size == 14.0 {
                ts.font_size = 18.0;
            }
            frame
                .painter()
                .text(Position::new(area.x + 4.0, area.y + 4.0), title, &ts);
        }
    }
}
