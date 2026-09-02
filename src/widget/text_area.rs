//! Text area widget — multi-line text editor.

use crate::core::{Color, Position, Rect, Style};
use crate::ontology::*;
use crate::runtime::Frame;
use crate::widget::StatefulWidget;

/// Multi-line text editor state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextAreaState {
    pub text: String,
    pub cursor_row: usize,
    pub cursor_col: usize,
}

impl TextAreaState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            text: String::new(),
            cursor_row: 0,
            cursor_col: 0,
        }
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
    }
}

impl Default for TextAreaState {
    fn default() -> Self {
        Self::new()
    }
}

/// A multi-line text editor.
pub struct TextArea {
    placeholder: String,
    style: Style,
    agent_id: std::borrow::Cow<'static, str>,
    handlers: Vec<(&'static str, Box<dyn std::any::Any + Send>)>,
}

impl TextArea {
    #[must_use]
    pub fn new() -> Self {
        Self {
            placeholder: String::new(),
            style: Style::default(),
            agent_id: std::borrow::Cow::Borrowed(""),
            handlers: Vec::new(),
        }
    }

    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
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

    /// Name this widget and give it the change to apply on `set_text`.
    ///
    /// Bound to the action `TextArea` advertises, so an agent following the
    /// ontology reaches it and the application writes no
    /// `execute_action` handler.
    #[must_use]
    pub fn on_input<M: 'static>(
        mut self,
        id: impl Into<std::borrow::Cow<'static, str>>,
        f: impl FnOnce(&mut M, &str) + Send + 'static,
    ) -> Self {
        self.agent_id = id.into();
        let handler: crate::runtime::ValueMutation<M> =
            Box::new(move |m: &mut M, v: &serde_json::Value| {
                f(m, v.get("text").and_then(|t| t.as_str()).unwrap_or(""));
            });
        self.handlers.push(("set_text", Box::new(handler)));
        self
    }

    /// Give this area the change to apply on `insert`.
    ///
    /// Separate from [`on_input`](Self::on_input) because the widget cannot
    /// perform the insertion itself: the text lives in the application, and
    /// only the fragment to add arrives with the action. Leave it unwired and
    /// `validate` will say so — `insert` would otherwise be a call that
    /// reports success and changes nothing.
    #[must_use]
    pub fn on_insert<M: 'static>(mut self, f: impl FnOnce(&mut M, &str) + Send + 'static) -> Self {
        let handler: crate::runtime::ValueMutation<M> =
            Box::new(move |m: &mut M, v: &serde_json::Value| {
                f(m, v.get("text").and_then(|t| t.as_str()).unwrap_or(""));
            });
        self.handlers.push(("insert", Box::new(handler)));
        self
    }

    pub fn agent_id(mut self, id: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        self.agent_id = id.into();
        self
    }
}

impl Default for TextArea {
    fn default() -> Self {
        Self::new()
    }
}

impl Discoverable for TextArea {
    fn schema(&self) -> WidgetSchema {
        let mut schema =
            WidgetSchema::new("TextArea", "A multi-line text editor", SemanticRole::Input);
        schema.usage_hint = Some("TextArea::new().placeholder(\"Type here...\")".into());
        schema.tags = vec![
            "textarea".into(),
            "editor".into(),
            "multiline".into(),
            "text".into(),
        ];
        schema
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![
            AgentCapability::TextInput {
                multiline: true,
                max_length: None,
            },
            AgentCapability::Focusable,
            AgentCapability::Scrollable {
                vertical: true,
                horizontal: false,
            },
        ]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![
            AgentAction::with_params(
                "set_text",
                "Set the entire text content",
                vec![ActionParam::required(
                    "text",
                    "Text content",
                    ActionParamType::String,
                )],
                true,
            ),
            AgentAction::with_params(
                "insert",
                "Insert text at the cursor",
                vec![ActionParam::required(
                    "text",
                    "Text to insert",
                    ActionParamType::String,
                )],
                true,
            ),
        ]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::Input
    }

    fn agent_state(&self) -> serde_json::Value {
        serde_json::json!({ "placeholder": self.placeholder })
    }

    fn agent_id(&self) -> Option<&str> {
        if self.agent_id.is_empty() {
            None
        } else {
            Some(&self.agent_id)
        }
    }

    fn accessibility_label(&self) -> Option<String> {
        if self.placeholder.is_empty() {
            None
        } else {
            Some(self.placeholder.clone())
        }
    }
}

impl StatefulWidget for TextArea {
    type State = TextAreaState;

    fn render(mut self, area: Rect, frame: &mut Frame<'_>, state: &mut TextAreaState) {
        if !self.agent_id.is_empty() {
            if frame.describes(area) {
                let node = UiNode::new("TextArea", SemanticRole::Input)
                    .with_id(self.agent_id.clone())
                    .with_bounds(area.into())
                    .with_property("text", serde_json::json!(state.text))
                    .with_property("line_count", serde_json::json!(state.text.lines().count()));
                frame.register_widget(node);
            }
            frame.register_hitbox(self.agent_id.clone(), area, 1);
            for (action, handler) in self.handlers.drain(..) {
                frame.register_message(self.agent_id.clone(), action, handler);
            }
        }

        let border_radius = self.style.border_radius.unwrap_or(2.0);
        frame
            .painter()
            .stroke_rect(area, Color::GRAY, 1.0, border_radius);
        frame.painter().push_clip(area);
        let ts = self.style.resolved_text();
        if state.text.is_empty() {
            let mut pts = ts.clone();
            pts.color = Color::GRAY;
            frame.painter().text(
                Position::new(area.x + 4.0, area.y + 4.0),
                &self.placeholder,
                &pts,
            );
        } else {
            let line_h = ts.line_height.unwrap_or(ts.font_size * 1.4);
            for (i, line) in state.text.lines().enumerate() {
                let y = area.y + 4.0 + i as f32 * line_h;
                frame
                    .painter()
                    .text(Position::new(area.x + 4.0, y), line, &ts);
            }
        }
        frame.painter().pop_clip();
    }
}
