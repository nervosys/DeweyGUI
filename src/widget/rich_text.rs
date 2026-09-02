//! Rich text widget — renders styled text spans and basic Markdown.
//!
//! Supports inline formatting: **bold**, *italic*, `code`, ~~strikethrough~~,
//! and \[links\](url). Paragraphs are separated by blank lines.

use crate::core::style::{Color, FontWeight, TextStyle};
use crate::core::{Position, Rect};
use crate::ontology::*;
use crate::runtime::Frame;
use crate::widget::Widget;

/// A styled text span.
#[derive(Debug, Clone)]
pub struct TextSpan {
    /// The text content.
    pub text: String,
    /// Whether the text is bold.
    pub bold: bool,
    /// Whether the text is italic.
    pub italic: bool,
    /// Whether the text is monospace/code.
    pub code: bool,
    /// Whether the text has strikethrough.
    pub strikethrough: bool,
    /// Optional link URL.
    pub link: Option<String>,
    /// Optional color override.
    pub color: Option<Color>,
}

impl TextSpan {
    /// Create a plain text span.
    #[must_use]
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            bold: false,
            italic: false,
            code: false,
            strikethrough: false,
            link: None,
            color: None,
        }
    }

    /// Create a bold text span.
    #[must_use]
    pub fn bold(text: impl Into<String>) -> Self {
        Self {
            bold: true,
            ..Self::plain(text)
        }
    }

    /// Create an italic text span.
    #[must_use]
    pub fn italic(text: impl Into<String>) -> Self {
        Self {
            italic: true,
            ..Self::plain(text)
        }
    }

    /// Create a code (monospace) text span.
    #[must_use]
    pub fn code(text: impl Into<String>) -> Self {
        Self {
            code: true,
            ..Self::plain(text)
        }
    }

    /// Create a link text span.
    #[must_use]
    pub fn link(text: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            link: Some(url.into()),
            color: Some(Color::rgba(0.4, 0.65, 1.0, 1.0)),
            ..Self::plain(text)
        }
    }

    /// Set a color override on this span.
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }
}

/// A rich text widget that renders styled text spans.
/// What an agent asked a [`RichText`] to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RichTextChange<'a> {
    /// Replace the content with this Markdown.
    SetMarkdown(&'a str),
    /// Remove all content.
    Clear,
}

pub struct RichText {
    spans: Vec<TextSpan>,
    agent_id: std::borrow::Cow<'static, str>,
    font_size: f32,
    base_color: Color,
    handlers: Vec<(&'static str, Box<dyn std::any::Any + Send>)>,
}

impl RichText {
    /// Create from a list of styled spans.
    #[must_use]
    pub fn new(spans: Vec<TextSpan>) -> Self {
        Self {
            spans,
            agent_id: std::borrow::Cow::Borrowed(""),
            handlers: Vec::new(),
            font_size: 14.0,
            base_color: Color::WHITE,
        }
    }

    /// Parse basic Markdown into rich text.
    ///
    /// Supported syntax: `**bold**`, `*italic*`, `` `code` ``,
    /// `~~strikethrough~~`, `[text](url)`.
    pub fn from_markdown(md: &str) -> Self {
        Self::new(parse_markdown(md))
    }

    /// Set the agent ID.
    /// Name this block and give it the change to apply on `set_markdown` and
    /// `clear`.
    ///
    /// The spans are parsed from the model each frame, so replacing them on
    /// the widget lasts one redraw.
    #[must_use]
    pub fn on_change<M: 'static>(
        mut self,
        id: impl Into<std::borrow::Cow<'static, str>>,
        f: impl Fn(&mut M, RichTextChange<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.agent_id = id.into();
        let f = std::sync::Arc::new(f);
        for action in ["set_markdown", "clear"] {
            let f = f.clone();
            let handler: crate::runtime::ValueMutation<M> =
                Box::new(move |m: &mut M, v: &serde_json::Value| {
                    let change = if action == "clear" {
                        RichTextChange::Clear
                    } else {
                        RichTextChange::SetMarkdown(
                            v.get("content")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or_default(),
                        )
                    };
                    f(m, change);
                });
            self.handlers.push((action, Box::new(handler)));
        }
        self
    }

    pub fn agent_id(mut self, id: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        self.agent_id = id.into();
        self
    }

    /// Set the base font size.
    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    /// Set the base text color.
    pub fn color(mut self, color: Color) -> Self {
        self.base_color = color;
        self
    }
}

impl Discoverable for RichText {
    fn schema(&self) -> WidgetSchema {
        let mut schema = WidgetSchema::new(
            "RichText",
            "A rich text display with styled spans and Markdown support",
            SemanticRole::Display,
        );
        schema.usage_hint = Some("RichText::from_markdown(\"**bold** and *italic*\")".into());
        schema.tags = vec![
            "text".into(),
            "rich".into(),
            "markdown".into(),
            "formatted".into(),
        ];
        schema
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![AgentCapability::Focusable, AgentCapability::Copyable]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![
            AgentAction::with_params(
                "set_markdown",
                "Replace content with parsed Markdown",
                vec![ActionParam::required(
                    "content",
                    "Markdown string",
                    ActionParamType::String,
                )],
                true,
            ),
            AgentAction::simple("clear", "Remove all spans", true),
        ]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::Display
    }

    fn agent_state(&self) -> serde_json::Value {
        let plain: String = self.spans.iter().map(|s| s.text.as_str()).collect();
        let spans: Vec<serde_json::Value> = self
            .spans
            .iter()
            .map(|s| {
                let mut obj = serde_json::json!({ "text": s.text });
                if s.bold {
                    obj["bold"] = serde_json::json!(true);
                }
                if s.italic {
                    obj["italic"] = serde_json::json!(true);
                }
                if s.code {
                    obj["code"] = serde_json::json!(true);
                }
                if s.strikethrough {
                    obj["strikethrough"] = serde_json::json!(true);
                }
                if let Some(ref link) = s.link {
                    obj["link"] = serde_json::json!(link);
                }
                obj
            })
            .collect();
        serde_json::json!({
            "text": plain,
            "span_count": self.spans.len(),
            "spans": spans,
        })
    }

    fn execute_action(
        &mut self,
        action: &str,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        match action {
            "set_markdown" => {
                let content = params["content"].as_str().ok_or("missing content")?;
                self.spans = parse_markdown(content);
                Ok(serde_json::json!({ "span_count": self.spans.len() }))
            }
            "clear" => {
                self.spans.clear();
                Ok(serde_json::json!({ "span_count": 0 }))
            }
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
        let plain: String = self.spans.iter().map(|s| s.text.as_str()).collect();
        if plain.is_empty() { None } else { Some(plain) }
    }
}

impl Widget for RichText {
    fn render(mut self, area: Rect, frame: &mut Frame<'_>) {
        if !self.agent_id.is_empty() {
            for (action, handler) in self.handlers.drain(..) {
                frame.register_message(self.agent_id.clone(), action, handler);
            }
        }

        if frame.ontology_enabled() && !self.agent_id.is_empty() {
            let plain: String = self.spans.iter().map(|s| s.text.as_str()).collect();
            let node = UiNode::new("RichText", SemanticRole::Display)
                .with_id(self.agent_id.clone())
                .with_bounds(area.into())
                .with_property("text", serde_json::json!(plain));
            frame.register_widget(node);
        }

        let mut x = area.x;
        let y = area.y;

        for span in &self.spans {
            let color = span.color.unwrap_or(self.base_color);
            let weight = if span.bold {
                FontWeight::Bold
            } else {
                FontWeight::Regular
            };
            let ts = TextStyle {
                font_size: self.font_size,
                color,
                weight,
                italic: span.italic,
                strikethrough: span.strikethrough,
                underline: span.link.is_some(),
                ..Default::default()
            };

            if span.code {
                // Draw code background
                let size = frame.painter().measure_text(&span.text, &ts);
                frame.painter().fill_rect(
                    Rect::new(x, y, size.width + 4.0, size.height),
                    Color::rgba(0.2, 0.2, 0.25, 0.6),
                    2.0,
                );
                frame
                    .painter()
                    .text(Position::new(x + 2.0, y), &span.text, &ts);
                x += size.width + 6.0;
            } else {
                let size = frame.painter().measure_text(&span.text, &ts);
                frame.painter().text(Position::new(x, y), &span.text, &ts);
                x += size.width;
            }
        }
    }
}

/// Parse basic inline Markdown into spans.
fn parse_markdown(input: &str) -> Vec<TextSpan> {
    let mut spans = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut buf = String::new();

    while i < len {
        // **bold**
        if i + 1 < len && chars[i] == '*' && chars[i + 1] == '*' {
            if !buf.is_empty() {
                spans.push(TextSpan::plain(std::mem::take(&mut buf)));
            }
            i += 2;
            let mut bold_buf = String::new();
            while i + 1 < len && !(chars[i] == '*' && chars[i + 1] == '*') {
                bold_buf.push(chars[i]);
                i += 1;
            }
            if i + 1 < len {
                i += 2; // skip closing **
            }
            spans.push(TextSpan::bold(bold_buf));
            continue;
        }

        // ~~strikethrough~~
        if i + 1 < len && chars[i] == '~' && chars[i + 1] == '~' {
            if !buf.is_empty() {
                spans.push(TextSpan::plain(std::mem::take(&mut buf)));
            }
            i += 2;
            let mut strike_buf = String::new();
            while i + 1 < len && !(chars[i] == '~' && chars[i + 1] == '~') {
                strike_buf.push(chars[i]);
                i += 1;
            }
            if i + 1 < len {
                i += 2;
            }
            spans.push(TextSpan {
                strikethrough: true,
                ..TextSpan::plain(strike_buf)
            });
            continue;
        }

        // *italic*
        if chars[i] == '*' {
            if !buf.is_empty() {
                spans.push(TextSpan::plain(std::mem::take(&mut buf)));
            }
            i += 1;
            let mut ital_buf = String::new();
            while i < len && chars[i] != '*' {
                ital_buf.push(chars[i]);
                i += 1;
            }
            if i < len {
                i += 1;
            }
            spans.push(TextSpan::italic(ital_buf));
            continue;
        }

        // `code`
        if chars[i] == '`' {
            if !buf.is_empty() {
                spans.push(TextSpan::plain(std::mem::take(&mut buf)));
            }
            i += 1;
            let mut code_buf = String::new();
            while i < len && chars[i] != '`' {
                code_buf.push(chars[i]);
                i += 1;
            }
            if i < len {
                i += 1;
            }
            spans.push(TextSpan::code(code_buf));
            continue;
        }

        // [text](url)
        if chars[i] == '[' {
            if !buf.is_empty() {
                spans.push(TextSpan::plain(std::mem::take(&mut buf)));
            }
            i += 1;
            let mut link_text = String::new();
            while i < len && chars[i] != ']' {
                link_text.push(chars[i]);
                i += 1;
            }
            if i < len {
                i += 1; // skip ]
            }
            if i < len && chars[i] == '(' {
                i += 1;
                let mut url = String::new();
                while i < len && chars[i] != ')' {
                    url.push(chars[i]);
                    i += 1;
                }
                if i < len {
                    i += 1;
                }
                spans.push(TextSpan::link(link_text, url));
            } else {
                // Not a real link, treat as plain text
                buf.push('[');
                buf.push_str(&link_text);
                buf.push(']');
            }
            continue;
        }

        buf.push(chars[i]);
        i += 1;
    }

    if !buf.is_empty() {
        spans.push(TextSpan::plain(buf));
    }

    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_plain() {
        let spans = parse_markdown("hello world");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "hello world");
        assert!(!spans[0].bold);
    }

    #[test]
    fn parse_bold() {
        let spans = parse_markdown("a **bold** b");
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[1].text, "bold");
        assert!(spans[1].bold);
    }

    #[test]
    fn parse_italic() {
        let spans = parse_markdown("a *italic* b");
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[1].text, "italic");
        assert!(spans[1].italic);
    }

    #[test]
    fn parse_code() {
        let spans = parse_markdown("a `code` b");
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[1].text, "code");
        assert!(spans[1].code);
    }

    #[test]
    fn parse_link() {
        let spans = parse_markdown("click [here](https://example.com) now");
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[1].text, "here");
        assert_eq!(spans[1].link.as_deref(), Some("https://example.com"));
    }

    #[test]
    fn parse_strikethrough() {
        let spans = parse_markdown("a ~~removed~~ b");
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[1].text, "removed");
        assert!(spans[1].strikethrough);
    }
}
