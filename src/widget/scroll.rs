//! Scroll area widget — a scrollable content region.

use crate::core::Rect;
use crate::ontology::*;
use crate::runtime::Frame;
use crate::widget::StatefulWidget;

/// How far one notch of the wheel moves the content.
///
/// Both backends report a wheel turn in notches rather than pixels, so this
/// is the conversion. Named because it is a decision, not a fact.
const LINE_HEIGHT: f32 = 24.0;

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
///
/// # Agent view
///
/// What an agent sees of this widget, and what it can call on it.
/// Generated from the widget's own answers; `tests/agent_view.rs`
/// fails the build if this block and the code disagree.
///
/// - role: `Scrollable`
/// - actions: `scroll_to`(x, y)
/// - state it publishes: `horizontal`, `vertical`
/// - capabilities: `Scrollable`
/// - live value: held by the application in `ScrollState`, not by the widget
pub struct ScrollArea {
    horizontal: bool,
    vertical: bool,
    agent_id: std::borrow::Cow<'static, str>,
    /// The change to apply when an agent scrolls this widget.
    on_scroll: Option<Box<dyn std::any::Any + Send>>,
}

impl ScrollArea {
    #[must_use]
    pub fn vertical() -> Self {
        Self {
            horizontal: false,
            vertical: true,
            agent_id: std::borrow::Cow::Borrowed(""),
            on_scroll: None,
        }
    }

    #[must_use]
    pub fn horizontal() -> Self {
        Self {
            horizontal: true,
            vertical: false,
            agent_id: std::borrow::Cow::Borrowed(""),
            on_scroll: None,
        }
    }

    #[must_use]
    pub fn both() -> Self {
        Self {
            horizontal: true,
            vertical: true,
            agent_id: std::borrow::Cow::Borrowed(""),
            on_scroll: None,
        }
    }

    /// Name this area and give it the change to apply on `scroll_to`.
    ///
    /// Both offsets are optional in the protocol, and they arrive that way:
    /// an agent that scrolls only vertically passes `None` for `x`, and
    /// clamping that to zero would silently reset horizontal position.
    #[must_use]
    pub fn on_scroll<M: 'static>(
        mut self,
        id: impl Into<std::borrow::Cow<'static, str>>,
        f: impl FnOnce(&mut M, Option<f32>, Option<f32>) + Send + 'static,
    ) -> Self {
        self.agent_id = id.into();
        let handler: crate::runtime::ValueMutation<M> =
            Box::new(move |m: &mut M, v: &serde_json::Value| {
                let num = |k: &str| {
                    v.get(k)
                        .and_then(serde_json::Value::as_f64)
                        .map(|n| n as f32)
                };
                f(m, num("x"), num("y"));
            });
        self.on_scroll = Some(Box::new(handler));
        self
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

    fn render(mut self, area: Rect, frame: &mut Frame<'_>, state: &mut ScrollState) {
        if !self.agent_id.is_empty() {
            if let Some(handler) = self.on_scroll.take() {
                frame.register_message(self.agent_id.clone(), "scroll_to", handler);
            }
            // Without a hitbox nothing could be found under the wheel, and
            // without this nothing knew what a turn meant here. A scrollable
            // region that does not scroll under the pointer left every
            // application catching `Event::Mouse` and doing the arithmetic
            // itself.
            frame.register_hitbox(self.agent_id.clone(), area, 0);
            let (at_x, at_y) = (state.offset_x, state.offset_y);
            frame.register_click(
                self.agent_id.clone(),
                // A click inside a scroll region belongs to whatever is drawn
                // in it, not to the region: `scroll_to` takes a destination
                // and a click is not one.
                crate::runtime::ClickParams::Unavailable,
            );
            frame.register_scroll(
                self.agent_id.clone(),
                crate::runtime::ScrollParams::from_delta(move |_at, dx, dy| {
                    // `scroll_to` takes an absolute offset and the wheel gives
                    // a delta, so the current offset is carried in here. Down
                    // is a negative `delta_y`, which is what both backends
                    // report and what a scrollbar moving down means.
                    let x = (at_x - dx * LINE_HEIGHT).max(0.0);
                    let y = (at_y - dy * LINE_HEIGHT).max(0.0);
                    if x == at_x && y == at_y {
                        return None;
                    }
                    Some(crate::runtime::Click::params(
                        serde_json::json!({ "x": x, "y": y }),
                    ))
                }),
            );
        }

        if frame.describes(area) && !self.agent_id.is_empty() {
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
