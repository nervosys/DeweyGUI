//! Splitter widget — a resizable split pane.

use crate::core::{Color, Position, Rect, Style};
use crate::ontology::*;
use crate::runtime::Frame;
use crate::widget::StatefulWidget;

/// The direction of the split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

/// Persistent state for a splitter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SplitterState {
    /// The split ratio (0.0 to 1.0). 0.5 means equal halves.
    pub ratio: f32,
}

impl SplitterState {
    #[must_use]
    pub fn new(ratio: f32) -> Self {
        Self {
            ratio: ratio.clamp(0.0, 1.0),
        }
    }
}

impl Default for SplitterState {
    fn default() -> Self {
        Self { ratio: 0.5 }
    }
}

/// A resizable split pane that divides an area into two regions.
///
/// The agent can read or set the split ratio programmatically.
pub struct Splitter {
    direction: SplitDirection,
    min_ratio: f32,
    max_ratio: f32,
    style: Style,
    agent_id: std::borrow::Cow<'static, str>,
    on_value: Option<Box<dyn std::any::Any + Send>>,
}

impl Splitter {
    #[must_use]
    pub fn new(direction: SplitDirection) -> Self {
        Self {
            direction,
            min_ratio: 0.1,
            max_ratio: 0.9,
            style: Style::default(),
            agent_id: std::borrow::Cow::Borrowed(""),
            on_value: None,
        }
    }

    pub fn horizontal() -> Self {
        Self::new(SplitDirection::Horizontal)
    }

    pub fn vertical() -> Self {
        Self::new(SplitDirection::Vertical)
    }

    pub fn min_ratio(mut self, min: f32) -> Self {
        self.min_ratio = min.clamp(0.0, 1.0);
        self
    }

    pub fn max_ratio(mut self, max: f32) -> Self {
        self.max_ratio = max.clamp(0.0, 1.0);
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

    /// Name this widget and give it the change to apply on `set_ratio`.
    ///
    /// Bound to the action `Splitter` advertises, so an agent following the
    /// ontology reaches it and the application writes no
    /// `execute_action` handler.
    #[must_use]
    pub fn on_change<M: 'static>(
        mut self,
        id: impl Into<std::borrow::Cow<'static, str>>,
        f: impl FnOnce(&mut M, f64) + Send + 'static,
    ) -> Self {
        let wrapped: crate::runtime::ValueMutation<M> =
            Box::new(move |m: &mut M, v: &serde_json::Value| {
                f(
                    m,
                    v.get("ratio")
                        .and_then(serde_json::Value::as_f64)
                        .unwrap_or(0.5),
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

    /// Compute the two child rectangles from the parent area and current ratio.
    pub fn compute_rects(area: Rect, direction: SplitDirection, ratio: f32) -> (Rect, Rect) {
        match direction {
            SplitDirection::Horizontal => {
                let left_w = area.width * ratio;
                let left = Rect::new(area.x, area.y, left_w, area.height);
                let right = Rect::new(area.x + left_w, area.y, area.width - left_w, area.height);
                (left, right)
            }
            SplitDirection::Vertical => {
                let top_h = area.height * ratio;
                let top = Rect::new(area.x, area.y, area.width, top_h);
                let bottom = Rect::new(area.x, area.y + top_h, area.width, area.height - top_h);
                (top, bottom)
            }
        }
    }
}

impl Discoverable for Splitter {
    fn schema(&self) -> WidgetSchema {
        let mut schema = WidgetSchema::new(
            "Splitter",
            "A resizable split pane",
            SemanticRole::Container,
        );
        schema.usage_hint = Some("Splitter::new(SplitDirection::Horizontal)".into());
        schema.tags = vec![
            "splitter".into(),
            "split".into(),
            "resize".into(),
            "pane".into(),
        ];
        schema
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![
            AgentCapability::Resizable {
                min_width: None,
                min_height: None,
                max_width: None,
                max_height: None,
            },
            AgentCapability::RangeEditable {
                min: self.min_ratio as f64,
                max: self.max_ratio as f64,
                step: Some(0.01),
            },
        ]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![AgentAction::with_params(
            "set_ratio",
            "Set the split ratio (0.0 = fully collapsed left/top, 1.0 = fully expanded)",
            vec![ActionParam::required(
                "ratio",
                "Split ratio (0.0 to 1.0)",
                ActionParamType::Float,
            )],
            true,
        )]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::Container
    }

    fn agent_state(&self) -> serde_json::Value {
        serde_json::json!({
            "direction": match self.direction {
                SplitDirection::Horizontal => "horizontal",
                SplitDirection::Vertical => "vertical",
            },
            "min_ratio": self.min_ratio,
            "max_ratio": self.max_ratio,
        })
    }

    fn agent_id(&self) -> Option<&str> {
        if self.agent_id.is_empty() {
            None
        } else {
            Some(&self.agent_id)
        }
    }
}

impl StatefulWidget for Splitter {
    type State = SplitterState;

    fn render(mut self, area: Rect, frame: &mut Frame<'_>, state: &mut SplitterState) {
        state.ratio = state.ratio.clamp(self.min_ratio, self.max_ratio);

        if !self.agent_id.is_empty() {
            if frame.describes(area) {
                let node = UiNode::new("Splitter", SemanticRole::Container)
                    .with_id(self.agent_id.clone())
                    .with_bounds(area.into())
                    .with_property("ratio", serde_json::json!(state.ratio))
                    .with_property(
                        "direction",
                        serde_json::json!(match self.direction {
                            SplitDirection::Horizontal => "horizontal",
                            SplitDirection::Vertical => "vertical",
                        }),
                    );
                frame.register_widget(node);
            }
            frame.register_hitbox(self.agent_id.clone(), area, 1);
            if let Some(handler) = self.on_value.take() {
                frame.register_message(self.agent_id.clone(), "set_ratio", handler);
            }
        }

        let (first, second) = Self::compute_rects(area, self.direction, state.ratio);

        // First panel
        let panel_bg = self.style.background.unwrap_or(Color::DARK_GRAY);
        frame.painter().fill_rect(first, panel_bg, 0.0);
        let ts = self.style.resolved_text();
        frame
            .painter()
            .text(Position::new(first.x + 4.0, first.y + 4.0), "Panel A", &ts);

        // Divider
        match self.direction {
            SplitDirection::Horizontal => {
                frame.painter().line(
                    Position::new(second.x, area.y),
                    Position::new(second.x, area.y + area.height),
                    Color::GRAY,
                    2.0,
                );
            }
            SplitDirection::Vertical => {
                frame.painter().line(
                    Position::new(area.x, second.y),
                    Position::new(area.x + area.width, second.y),
                    Color::GRAY,
                    2.0,
                );
            }
        }

        // Second panel
        frame.painter().fill_rect(second, panel_bg, 0.0);
        frame.painter().text(
            Position::new(second.x + 4.0, second.y + 4.0),
            "Panel B",
            &ts,
        );
    }
}
