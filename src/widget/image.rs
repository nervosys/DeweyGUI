//! Image widget — displays images from URI or raw RGBA pixel data.

use crate::core::{Color, Position, Rect, Style};
use crate::ontology::*;
use crate::runtime::Frame;
use crate::widget::Widget;

/// An image display widget.
pub struct Image {
    source: ImageSource,
    alt: String,
    style: Style,
    agent_id: std::borrow::Cow<'static, str>,
    fit: ImageFit,
}

/// Image source.
#[derive(Clone)]
pub enum ImageSource {
    /// A URI/path to an image file. egui loads `file://` and embedded `bytes://` URIs.
    Uri(String),
    /// Raw RGBA pixels (uploaded as a texture).
    Rgba {
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    },
}

/// How the image is sized within its area.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImageFit {
    /// Scale to fill the area, preserving aspect ratio (may crop).
    Cover,
    /// Scale to fit within the area, preserving aspect ratio (may letterbox).
    #[default]
    Contain,
    /// Stretch to fill the exact area.
    Fill,
    /// Show at original size (no scaling).
    Original,
}

impl Image {
    #[must_use]
    pub fn from_uri(uri: impl Into<String>) -> Self {
        Self {
            source: ImageSource::Uri(uri.into()),
            alt: String::new(),
            style: Style::default(),
            agent_id: std::borrow::Cow::Borrowed(""),
            fit: ImageFit::default(),
        }
    }

    #[must_use]
    pub fn from_rgba(width: u32, height: u32, pixels: Vec<u8>) -> Self {
        Self {
            source: ImageSource::Rgba {
                width,
                height,
                pixels,
            },
            alt: String::new(),
            style: Style::default(),
            agent_id: std::borrow::Cow::Borrowed(""),
            fit: ImageFit::default(),
        }
    }

    pub fn alt(mut self, alt: impl Into<String>) -> Self {
        self.alt = alt.into();
        self
    }

    pub fn agent_id(mut self, id: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        self.agent_id = id.into();
        self
    }

    pub fn fit(mut self, fit: ImageFit) -> Self {
        self.fit = fit;
        self
    }
}

impl Discoverable for Image {
    fn schema(&self) -> WidgetSchema {
        WidgetSchema::new("Image", "An image display", SemanticRole::Media)
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![AgentCapability::Zoomable {
            min_zoom: 0.1,
            max_zoom: 10.0,
        }]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::Media
    }

    fn agent_state(&self) -> serde_json::Value {
        match &self.source {
            ImageSource::Uri(uri) => {
                serde_json::json!({ "source": "uri", "uri": uri, "alt": self.alt, "fit": format!("{:?}", self.fit) })
            }
            ImageSource::Rgba { width, height, .. } => {
                serde_json::json!({ "source": "rgba", "width": width, "height": height, "alt": self.alt, "fit": format!("{:?}", self.fit) })
            }
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
        if self.alt.is_empty() {
            None
        } else {
            Some(self.alt.clone())
        }
    }
}

impl Widget for Image {
    fn render(self, area: Rect, frame: &mut Frame<'_>) {
        // Draw a placeholder frame with the alt text.
        // Actual image decoding and texture upload is handled by
        // backend-specific extensions outside the core Painter trait.
        frame.painter().stroke_rect(area, Color::GRAY, 1.0, 0.0);
        let label = if self.alt.is_empty() {
            "[image]"
        } else {
            &self.alt
        };
        let mut ts = self.style.resolved_text();
        if ts.font_size == 14.0 {
            ts.font_size = 12.0;
        }
        if ts.color == Color::WHITE {
            ts.color = Color::GRAY;
        }
        let sz = frame.painter().measure_text(label, &ts);
        let tx = area.x + (area.width - sz.width) * 0.5;
        let ty = area.y + (area.height - sz.height) * 0.5;
        frame.painter().text(Position::new(tx, ty), label, &ts);

        // Built last so owned fields (text, item vectors) move into the
        // state instead of being cloned; painting above only borrows them.
        if frame.describes(area) && !self.agent_id.is_empty() {
            let node = UiNode::new("Image", SemanticRole::Media)
                .with_id(self.agent_id.clone())
                .with_bounds(area.into())
                .with_property("alt", serde_json::Value::from(self.alt));
            frame.register_widget(node);
        }
    }
}
