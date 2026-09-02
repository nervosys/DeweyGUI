//! Virtual list — efficiently renders only visible items from large datasets.
//!
//! Instead of creating widget instances for every item, the virtual list
//! only renders items that are within the visible viewport, recycling
//! elements as the user scrolls.

use crate::core::Rect;
use crate::ontology::*;
use crate::runtime::Frame;
use crate::widget::StatefulWidget;

/// State for a virtual list.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VirtualListState {
    /// Scroll offset in logical pixels.
    pub scroll_offset: f32,
    /// Total number of items.
    pub total_items: usize,
}

impl Default for VirtualListState {
    fn default() -> Self {
        Self {
            scroll_offset: 0.0,
            total_items: 0,
        }
    }
}

impl VirtualListState {
    #[must_use]
    pub fn new(total_items: usize) -> Self {
        Self {
            scroll_offset: 0.0,
            total_items,
        }
    }

    /// Scroll to make a specific item index visible.
    pub fn scroll_to(&mut self, index: usize, item_height: f32) {
        self.scroll_offset = index as f32 * item_height;
    }
}

/// A virtualized list that only renders visible items.
///
/// Provides the same ontology interface as a regular list, but performs
/// windowed rendering for performance with large datasets.
pub struct VirtualList<F> {
    /// Height of each item in logical pixels.
    item_height: f32,
    /// Callback that renders a single item given its index and rect.
    render_item: F,
    /// Number of extra items to render above/below the viewport.
    overscan: usize,
    agent_id: std::borrow::Cow<'static, str>,
    /// The change to apply when an agent scrolls this widget.
    on_scroll: Option<Box<dyn std::any::Any + Send>>,
}

impl<F> VirtualList<F>
where
    F: Fn(usize, Rect, &mut Frame<'_>),
{
    pub fn new(item_height: f32, render_item: F) -> Self {
        Self {
            item_height,
            render_item,
            overscan: 2,
            agent_id: std::borrow::Cow::Borrowed(""),
            on_scroll: None,
        }
    }

    pub fn overscan(mut self, overscan: usize) -> Self {
        self.overscan = overscan;
        self
    }

    /// Name this list and give it the change to apply on `scroll_to`.
    ///
    /// The scroll offset lives in [`VirtualListState`], so without a handler
    /// an agent cannot bring an item into view — which for a list that only
    /// renders its visible window means it cannot see the item at all.
    #[must_use]
    pub fn on_scroll<M: 'static>(
        mut self,
        id: impl Into<std::borrow::Cow<'static, str>>,
        f: impl FnOnce(&mut M, usize) + Send + 'static,
    ) -> Self {
        self.agent_id = id.into();
        let handler: crate::runtime::ValueMutation<M> =
            Box::new(move |m: &mut M, v: &serde_json::Value| {
                f(
                    m,
                    v.get("index")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0) as usize,
                );
            });
        self.on_scroll = Some(Box::new(handler));
        self
    }

    pub fn agent_id(mut self, id: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        self.agent_id = id.into();
        self
    }

    /// Compute the range of visible item indices.
    pub fn visible_range(
        scroll_offset: f32,
        viewport_height: f32,
        item_height: f32,
        total_items: usize,
        overscan: usize,
    ) -> std::ops::Range<usize> {
        if item_height <= 0.0 || total_items == 0 {
            return 0..0;
        }
        let first = (scroll_offset / item_height).floor() as usize;
        let visible_count = (viewport_height / item_height).ceil() as usize;
        let start = first.saturating_sub(overscan);
        let end = (first + visible_count + overscan).min(total_items);
        start..end
    }
}

impl<F> Discoverable for VirtualList<F>
where
    F: Fn(usize, Rect, &mut Frame<'_>),
{
    fn schema(&self) -> WidgetSchema {
        let mut schema = WidgetSchema::new(
            "VirtualList",
            "A virtualized list for large datasets",
            SemanticRole::Scrollable,
        );
        schema.usage_hint =
            Some("VirtualList::new(24.0, |idx, rect, frame| { /* render item */ })".into());
        schema.tags = vec![
            "virtual".into(),
            "list".into(),
            "performance".into(),
            "scroll".into(),
        ];
        schema
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![
            AgentCapability::Scrollable {
                vertical: true,
                horizontal: false,
            },
            AgentCapability::Focusable,
        ]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![
            AgentAction::with_params(
                "scroll_to",
                "Scroll to make a specific item visible",
                vec![ActionParam::required(
                    "index",
                    "Item index to scroll to",
                    ActionParamType::Integer,
                )],
                true,
            ),
            AgentAction::simple(
                "get_visible_range",
                "Get the range of currently visible items",
                false,
            ),
        ]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::Scrollable
    }

    fn agent_state(&self) -> serde_json::Value {
        serde_json::json!({
            "item_height": self.item_height,
            "overscan": self.overscan,
        })
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
}

impl<F> StatefulWidget for VirtualList<F>
where
    F: Fn(usize, Rect, &mut Frame<'_>),
{
    type State = VirtualListState;

    fn render(mut self, area: Rect, frame: &mut Frame<'_>, state: &mut VirtualListState) {
        if !self.agent_id.is_empty() {
            if let Some(handler) = self.on_scroll.take() {
                frame.register_message(self.agent_id.clone(), "scroll_to", handler);
            }
        }

        let total_content_height = state.total_items as f32 * self.item_height;
        let max_scroll = (total_content_height - area.height).max(0.0);
        state.scroll_offset = state.scroll_offset.clamp(0.0, max_scroll);

        let range = Self::visible_range(
            state.scroll_offset,
            area.height,
            self.item_height,
            state.total_items,
            self.overscan,
        );

        if !self.agent_id.is_empty() {
            if frame.ontology_enabled() {
                let node = UiNode::new("VirtualList", SemanticRole::Scrollable)
                    .with_id(self.agent_id.clone())
                    .with_bounds(area.into())
                    .with_property("total_items", serde_json::json!(state.total_items))
                    .with_property("scroll_offset", serde_json::json!(state.scroll_offset))
                    .with_property("visible_start", serde_json::json!(range.start))
                    .with_property("visible_end", serde_json::json!(range.end));
                frame.register_widget(node);
            }
            frame.register_hitbox(self.agent_id.clone(), area, 1);
        }

        for i in range {
            let item_y = area.y + (i as f32 * self.item_height) - state.scroll_offset;
            let item_rect = Rect::new(area.x, item_y, area.width, self.item_height);
            // Only render if the item is at least partially visible
            if item_rect.y + item_rect.height > area.y && item_rect.y < area.y + area.height {
                (self.render_item)(i, item_rect, frame);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_range_basic() {
        // 100px viewport, 20px items, 10 total items, no scroll
        let range =
            VirtualList::<fn(usize, Rect, &mut Frame<'_>)>::visible_range(0.0, 100.0, 20.0, 10, 0);
        assert_eq!(range, 0..5);
    }

    #[test]
    fn visible_range_scrolled() {
        // Scrolled 40px down = 2 items
        let range =
            VirtualList::<fn(usize, Rect, &mut Frame<'_>)>::visible_range(40.0, 100.0, 20.0, 10, 0);
        assert_eq!(range, 2..7);
    }

    #[test]
    fn visible_range_with_overscan() {
        let range =
            VirtualList::<fn(usize, Rect, &mut Frame<'_>)>::visible_range(40.0, 100.0, 20.0, 10, 2);
        assert_eq!(range, 0..9);
    }

    #[test]
    fn visible_range_empty() {
        let range =
            VirtualList::<fn(usize, Rect, &mut Frame<'_>)>::visible_range(0.0, 100.0, 20.0, 0, 0);
        assert_eq!(range, 0..0);
    }
}
