//! Modal widget — a dialog overlay with background dimming.

use crate::core::{Color, Position, Rect, Style};
use crate::ontology::*;
use crate::runtime::Frame;
use crate::widget::Widget;

/// A modal dialog that dims the background and renders as a floating window.
///
/// When `open` is `true`, a semi-transparent backdrop is drawn over the
/// application and a centered window renders the title and body content.
///
/// # Examples
///
/// ```
/// # use dewey::prelude::*;
/// Modal::new("Confirm Delete", true)
///     .body("Are you sure you want to delete this item?")
///     .bg(Color::DARK_GRAY)
///     .fg(Color::WHITE)
///     .rounded(12.0);
/// ```
pub struct Modal {
    title: String,
    open: bool,
    style: Style,
    agent_id: std::borrow::Cow<'static, str>,
    /// Content lines to show inside the modal body.
    body: Vec<String>,
    /// Width of the modal window (0.0 = auto).
    width: f32,
    handlers: Vec<(&'static str, Box<dyn std::any::Any + Send>)>,
}

impl Modal {
    #[must_use]
    pub fn new(title: impl Into<String>, open: bool) -> Self {
        Self {
            title: title.into(),
            open,
            style: Style::default(),
            agent_id: std::borrow::Cow::Borrowed(""),
            handlers: Vec::new(),
            body: Vec::new(),
            width: 0.0,
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

    pub fn rounded(mut self, radius: f32) -> Self {
        self.style.border_radius = Some(radius);
        self
    }

    /// Name this dialog and give it the change to apply on `open` and
    /// `close`.
    ///
    /// A closed modal renders nothing, so it has no node in the UI tree — but
    /// it still advertises `open`, and that call must work or an agent can
    /// never get the dialog back.
    #[must_use]
    pub fn on_change<M: 'static>(
        mut self,
        id: impl Into<std::borrow::Cow<'static, str>>,
        f: impl Fn(&mut M, bool) + Send + Sync + 'static,
    ) -> Self {
        self.agent_id = id.into();
        let f = std::sync::Arc::new(f);
        for action in ["open", "close"] {
            let f = f.clone();
            let handler: crate::runtime::Mutation<M> =
                Box::new(move |m: &mut M| f(m, action == "open"));
            self.handlers.push((action, Box::new(handler)));
        }
        self
    }

    pub fn agent_id(mut self, id: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        self.agent_id = id.into();
        self
    }

    /// Add a text paragraph to the modal body.
    pub fn body(mut self, text: impl Into<String>) -> Self {
        self.body.push(text.into());
        self
    }

    /// Set a fixed width for the modal window.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
}

/// Actions on the widget value itself.
///
/// Not the path an agent takes: a widget is rebuilt inside `view` on
/// every frame, so a change made here lasts until the next redraw.
/// An agent's `execute_action` reaches a handler and then
/// [`Model::execute_action`](crate::runtime::Model::execute_action).
/// This stays because the logic is worth testing on its own.
impl Modal {
    pub fn execute_action(
        &mut self,
        action: &str,
        _params: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        match action {
            "open" => {
                self.open = true;
                Ok(serde_json::json!({ "open": true }))
            }
            "close" => {
                self.open = false;
                Ok(serde_json::json!({ "open": false }))
            }
            _ => Err(format!("Unknown action: {action}")),
        }
    }
}

impl Discoverable for Modal {
    fn schema(&self) -> WidgetSchema {
        let mut schema = WidgetSchema::new("Modal", "A modal dialog window", SemanticRole::Modal);
        schema.usage_hint = Some("Modal::new(\"Confirm\", true).body(\"Are you sure?\")".into());
        schema.tags = vec![
            "modal".into(),
            "dialog".into(),
            "overlay".into(),
            "popup".into(),
        ];
        schema
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![AgentCapability::Closable, AgentCapability::Focusable]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![
            AgentAction::simple("open", "Open the modal", true),
            AgentAction::simple("close", "Close the modal", true),
        ]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::Modal
    }

    fn agent_state(&self) -> serde_json::Value {
        serde_json::json!({ "title": self.title, "open": self.open })
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

impl Widget for Modal {
    fn render(mut self, area: Rect, frame: &mut Frame<'_>) {
        // Before the early return: a closed dialog still answers `open`.
        if !self.agent_id.is_empty() {
            for (action, handler) in self.handlers.drain(..) {
                frame.register_message(self.agent_id.clone(), action, handler);
            }
        }
        if !self.open {
            return;
        }

        if frame.describes(area) && !self.agent_id.is_empty() {
            let node = UiNode::new("Modal", SemanticRole::Modal)
                .with_id(self.agent_id.clone())
                .with_bounds(area.into())
                .with_property("title", serde_json::json!(self.title))
                .with_property("open", serde_json::json!(self.open));
            frame.register_widget(node);
        }

        // Backdrop
        frame
            .painter()
            .fill_rect(area, Color::BLACK.with_alpha(0.5), 0.0);

        // Compute modal rect centered in area
        let modal_w = if self.width > 0.0 {
            self.width
        } else {
            area.width * 0.5
        };
        let modal_h = (self.body.len() as f32 + 2.0) * 24.0 + 40.0;
        let mx = area.x + (area.width - modal_w) * 0.5;
        let my = area.y + (area.height - modal_h) * 0.5;
        let modal_rect = Rect::new(mx, my, modal_w, modal_h);

        // Window
        let modal_bg = self.style.background.unwrap_or(Color::DARK_GRAY);
        let modal_radius = self.style.border_radius.unwrap_or(8.0);
        frame
            .painter()
            .fill_rect(modal_rect, modal_bg, modal_radius);
        frame
            .painter()
            .stroke_rect(modal_rect, Color::GRAY, 1.0, modal_radius);

        // Title
        let mut title_ts = self.style.resolved_text();
        if title_ts.font_size == 14.0 {
            title_ts.font_size = 18.0;
        }
        title_ts.weight = crate::core::style::FontWeight::Bold;
        frame
            .painter()
            .text(Position::new(mx + 12.0, my + 12.0), &self.title, &title_ts);

        // Separator
        frame.painter().line(
            Position::new(mx + 8.0, my + 36.0),
            Position::new(mx + modal_w - 8.0, my + 36.0),
            Color::GRAY,
            1.0,
        );

        // Body
        let body_ts = self.style.resolved_text();
        for (i, line) in self.body.iter().enumerate() {
            let y = my + 44.0 + i as f32 * 20.0;
            frame
                .painter()
                .text(Position::new(mx + 12.0, y), line, &body_ts);
        }
    }
}
