//! Scroll area widget — a scrollable content region.

use crate::core::Rect;
use crate::ontology::*;
use crate::runtime::Frame;
use crate::widget::StatefulWidget;

/// Scroll state.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ScrollState {
    pub offset_x: f32,
    pub offset_y: f32,
}

impl ScrollState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            offset_x: 0.0,
            offset_y: 0.0,
        }
    }
}

/// A scrollable area.
pub struct ScrollArea {
    horizontal: bool,
    vertical: bool,
    agent_id: std::borrow::Cow<'static, str>,
}

impl ScrollArea {
    #[must_use]
    pub fn vertical() -> Self {
        Self {
            horizontal: false,
            vertical: true,
            agent_id: std::borrow::Cow::Borrowed(""),
        }
    }

    #[must_use]
    pub fn horizontal() -> Self {
        Self {
            horizontal: true,
            vertical: false,
            agent_id: std::borrow::Cow::Borrowed(""),
        }
    }

    #[must_use]
    pub fn both() -> Self {
        Self {
            horizontal: true,
            vertical: true,
            agent_id: std::borrow::Cow::Borrowed(""),
        }
    }

    pub fn agent_id(mut self, id: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        self.agent_id = id.into();
        self
    }
}

impl Discoverable for ScrollArea {
    fn schema(&self) -> WidgetSchema {
        let mut schema = WidgetSchema::new(
            "ScrollArea",
            "A scrollable content area",
            SemanticRole::Scrollable,
        );
        schema.usage_hint = Some("ScrollArea::new().vertical(true).horizontal(false)".into());
        schema.tags = vec!["scroll".into(), "overflow".into(), "viewport".into()];
        schema
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![AgentCapability::Scrollable {
            vertical: self.vertical,
            horizontal: self.horizontal,
        }]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![AgentAction::with_params(
            "scroll_to",
            "Scroll to a position",
            vec![
                ActionParam::optional(
                    "x",
                    "Horizontal offset",
                    ActionParamType::Float,
                    serde_json::json!(0.0),
                ),
                ActionParam::optional(
                    "y",
                    "Vertical offset",
                    ActionParamType::Float,
                    serde_json::json!(0.0),
                ),
            ],
            true,
        )]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::Scrollable
    }

    fn agent_state(&self) -> serde_json::Value {
        serde_json::json!({ "horizontal": self.horizontal, "vertical": self.vertical })
    }

    fn execute_action(
        &mut self,
        _action: &str,
        _params: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        Err("Use StatefulWidget for scroll state mutations".to_string())
    }

    fn agent_id(&self) -> Option<&str> {
        if self.agent_id.is_empty() {
            None
        } else {
            Some(&self.agent_id)
        }
    }
}

impl StatefulWidget for ScrollArea {
    type State = ScrollState;

    fn render(self, area: Rect, frame: &mut Frame<'_>, state: &mut ScrollState) {
        if frame.ontology_enabled() && !self.agent_id.is_empty() {
            let node = UiNode::new("ScrollArea", SemanticRole::Scrollable)
                .with_id(self.agent_id.clone())
                .with_bounds(area.into())
                .with_property("scroll_x", serde_json::json!(state.offset_x))
                .with_property("scroll_y", serde_json::json!(state.offset_y));
            frame.register_widget(node);
        }

        // Apply clip for scroll region. Child content is rendered by the
        // user's view function which receives the clipped area.
        frame.painter().push_clip(area);
        frame.painter().pop_clip();
    }
}
