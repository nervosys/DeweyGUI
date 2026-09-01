//! Text input widget — single-line text entry.

use crate::core::{Color, Position, Rect, Style};
use crate::ontology::*;
use crate::runtime::Frame;
use crate::widget::StatefulWidget;

/// State for a text input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextInputState {
    pub text: String,
    pub cursor: usize,
    pub focused: bool,
}

impl TextInputState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            focused: false,
        }
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self.cursor = self.text.len();
        self
    }
}

impl Default for TextInputState {
    fn default() -> Self {
        Self::new()
    }
}

/// A single-line text input.
///
/// # Examples
///
/// ```
/// # use dewey::prelude::*;
/// TextInput::new()
///     .placeholder("Enter your name…")
///     .bg(Color::DARK_GRAY)
///     .rounded(8.0);
/// ```
pub struct TextInput {
    placeholder: String,
    style: Style,
    agent_id: std::borrow::Cow<'static, str>,
}

impl TextInput {
    #[must_use]
    pub fn new() -> Self {
        Self {
            placeholder: String::new(),
            style: Style::default(),
            agent_id: std::borrow::Cow::Borrowed(""),
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

    pub fn agent_id(mut self, id: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        self.agent_id = id.into();
        self
    }
}

impl Default for TextInput {
    fn default() -> Self {
        Self::new()
    }
}

impl Discoverable for TextInput {
    fn schema(&self) -> WidgetSchema {
        let mut schema =
            WidgetSchema::new("TextInput", "A single-line text input", SemanticRole::Input);
        schema.usage_hint = Some("TextInput::new().placeholder(\"Enter name...\")".into());
        schema.tags = vec!["input".into(), "text".into(), "field".into()];
        schema
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![
            AgentCapability::TextInput {
                multiline: false,
                max_length: None,
            },
            AgentCapability::Focusable,
        ]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![
            AgentAction::with_params(
                "set_text",
                "Set the input text",
                vec![ActionParam::required(
                    "text",
                    "The text to set",
                    ActionParamType::String,
                )],
                true,
            ),
            AgentAction::simple("clear", "Clear the input", true),
        ]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::Input
    }

    fn agent_state(&self) -> serde_json::Value {
        serde_json::json!({ "placeholder": self.placeholder })
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

    fn accessibility_label(&self) -> Option<String> {
        if self.placeholder.is_empty() {
            None
        } else {
            Some(self.placeholder.clone())
        }
    }
}

impl StatefulWidget for TextInput {
    type State = TextInputState;

    fn render(self, area: Rect, frame: &mut Frame<'_>, state: &mut TextInputState) {
        if !self.agent_id.is_empty() {
            if frame.ontology_enabled() {
                let node = UiNode::new("TextInput", SemanticRole::Input)
                    .with_id(self.agent_id.clone())
                    .with_bounds(area.into())
                    .with_property("text", serde_json::json!(state.text))
                    .with_property("placeholder", serde_json::json!(self.placeholder))
                    .with_property("focused", serde_json::json!(state.focused));
                frame.register_widget(node);
            }
            frame.register_hitbox(self.agent_id.clone(), area, 1);
        }

        // Background fill
        let bg = self
            .style
            .background
            .unwrap_or(Color::rgba(0.12, 0.12, 0.14, 1.0));
        let border_color = if state.focused {
            Color::rgba(0.3, 0.5, 0.9, 0.8)
        } else {
            Color::rgba(0.3, 0.3, 0.35, 0.6)
        };
        let radius = self.style.border_radius.unwrap_or(6.0);
        frame.painter().fill_rect(area, bg, radius);
        frame.painter().stroke_rect(area, border_color, 1.0, radius);

        // Text or placeholder
        let ts = self.style.resolved_text();
        let pad_x = 10.0;
        let text_y = area.y + (area.height - ts.font_size * 1.3) * 0.5;
        if state.text.is_empty() {
            let mut pts = ts.clone();
            pts.color = Color::rgba(0.45, 0.45, 0.5, 1.0);
            frame.painter().text(
                Position::new(area.x + pad_x, text_y),
                &self.placeholder,
                &pts,
            );
        } else {
            frame
                .painter()
                .text(Position::new(area.x + pad_x, text_y), &state.text, &ts);
        }
        // Cursor
        if state.focused {
            let before = &state.text[..state.cursor.min(state.text.len())];
            let cursor_x = frame.painter().measure_text(before, &ts).width;
            frame.painter().line(
                Position::new(area.x + pad_x + cursor_x, area.y + 6.0),
                Position::new(area.x + pad_x + cursor_x, area.y + area.height - 6.0),
                Color::WHITE,
                1.5,
            );
        }
    }
}
