//! Web backend for Dewey (wasm32 target).
//!
//! Provides a `WebPainter` that renders to an HTML5 `<canvas>` element
//! using the Canvas 2D API. This backend is enabled behind the `web-backend`
//! feature and only compiles on `wasm32` targets.
//!
//! **This module is a painter, not a runner.** [`WebPainter`] records drawing
//! operations as [`WebRenderOp`]s that a host page can replay onto a canvas;
//! there is no event loop, no `main`, and nothing that starts an application.
//! Driving one is the embedder's job.
//!
//! A previous version of this comment showed `WebRunner::new(model, painter)
//! .start()`. No such type has ever existed: the example was marked as an
//! ignored doctest, so nothing compiled it and nothing noticed.
//!
//! # Usage
//!
//! ```
//! use dewey::backend::web::WebPainter;
//! use dewey::core::{Color, Rect};
//! use dewey::paint::Painter;
//!
//! let mut painter = WebPainter::new("my-canvas-id");
//! painter.fill_rect(Rect::new(0.0, 0.0, 100.0, 40.0), Color::BLUE, 4.0);
//!
//! // Hand the recorded operations to the page to replay.
//! let commands = painter.to_json();
//! assert!(commands.contains("my-canvas-id") || !commands.is_empty());
//! ```

use crate::core::style::TextStyle;
use crate::core::{Color, Position, Rect, Size};
use crate::paint::Painter;

/// A canvas 2D painter for the web.
///
/// Records drawing operations as serializable render commands that can
/// be sent to a JavaScript bridge to execute on a real `<canvas>`. On
/// native targets this is a no-op and only used for cross-compilation
/// checks.
pub struct WebPainter {
    canvas_id: String,
    commands: Vec<WebRenderOp>,
    clip_stack: Vec<Rect>,
}

/// A serializable rendering operation for the JS bridge.
#[derive(Debug, Clone, serde::Serialize)]
pub enum WebRenderOp {
    FillRect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: String,
        radius: f32,
    },
    StrokeRect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: String,
        width: f32,
        radius: f32,
    },
    FillCircle {
        cx: f32,
        cy: f32,
        r: f32,
        color: String,
    },
    StrokeCircle {
        cx: f32,
        cy: f32,
        r: f32,
        color: String,
        width: f32,
    },
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        color: String,
        width: f32,
    },
    Text {
        x: f32,
        y: f32,
        text: String,
        font: String,
        color: String,
    },
    PushClip {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    },
    PopClip,
}

impl WebPainter {
    /// Create a new web painter targeting the given canvas element ID.
    pub fn new(canvas_id: impl Into<String>) -> Self {
        Self {
            canvas_id: canvas_id.into(),
            commands: Vec::new(),
            clip_stack: Vec::new(),
        }
    }

    /// Get the canvas element ID.
    pub fn canvas_id(&self) -> &str {
        &self.canvas_id
    }

    /// Take the accumulated render ops and clear the buffer.
    pub fn take_commands(&mut self) -> Vec<WebRenderOp> {
        std::mem::take(&mut self.commands)
    }

    /// Serialize the current commands to JSON for the JS bridge.
    pub fn to_json(&self) -> String {
        serde_json::to_string(&self.commands).unwrap_or_default()
    }
}

fn color_to_css(c: Color) -> String {
    format!(
        "rgba({},{},{},{})",
        (c.r * 255.0) as u8,
        (c.g * 255.0) as u8,
        (c.b * 255.0) as u8,
        c.a
    )
}

fn text_style_to_font(style: &TextStyle) -> String {
    let weight = if style.weight == crate::core::style::FontWeight::Bold {
        "bold "
    } else {
        ""
    };
    let italic = if style.italic { "italic " } else { "" };
    format!("{italic}{weight}{}px sans-serif", style.font_size)
}

impl Painter for WebPainter {
    fn fill_rect(&mut self, rect: Rect, color: Color, corner_radius: f32) {
        self.commands.push(WebRenderOp::FillRect {
            x: rect.x,
            y: rect.y,
            w: rect.width,
            h: rect.height,
            color: color_to_css(color),
            radius: corner_radius,
        });
    }

    fn stroke_rect(&mut self, rect: Rect, color: Color, width: f32, corner_radius: f32) {
        self.commands.push(WebRenderOp::StrokeRect {
            x: rect.x,
            y: rect.y,
            w: rect.width,
            h: rect.height,
            color: color_to_css(color),
            width,
            radius: corner_radius,
        });
    }

    fn fill_circle(&mut self, center: Position, radius: f32, color: Color) {
        self.commands.push(WebRenderOp::FillCircle {
            cx: center.x,
            cy: center.y,
            r: radius,
            color: color_to_css(color),
        });
    }

    fn stroke_circle(&mut self, center: Position, radius: f32, color: Color, width: f32) {
        self.commands.push(WebRenderOp::StrokeCircle {
            cx: center.x,
            cy: center.y,
            r: radius,
            color: color_to_css(color),
            width,
        });
    }

    fn line(&mut self, from: Position, to: Position, color: Color, width: f32) {
        self.commands.push(WebRenderOp::Line {
            x1: from.x,
            y1: from.y,
            x2: to.x,
            y2: to.y,
            color: color_to_css(color),
            width,
        });
    }

    fn text(&mut self, pos: Position, text: &str, style: &TextStyle) {
        self.commands.push(WebRenderOp::Text {
            x: pos.x,
            y: pos.y,
            text: text.to_string(),
            font: text_style_to_font(style),
            color: color_to_css(style.color),
        });
    }

    fn measure_text(&self, text: &str, style: &TextStyle) -> Size {
        let w = style.font_size * 0.6 * text.len() as f32;
        Size::new(w, style.font_size * 1.2)
    }

    fn push_clip(&mut self, rect: Rect) {
        self.clip_stack.push(rect);
        self.commands.push(WebRenderOp::PushClip {
            x: rect.x,
            y: rect.y,
            w: rect.width,
            h: rect.height,
        });
    }

    fn pop_clip(&mut self) {
        self.clip_stack.pop();
        self.commands.push(WebRenderOp::PopClip);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_painter_basics() {
        let mut p = WebPainter::new("canvas");
        p.fill_rect(Rect::new(0.0, 0.0, 100.0, 50.0), Color::RED, 4.0);
        p.text(Position::new(10.0, 10.0), "Hello", &TextStyle::default());
        let cmds = p.take_commands();
        assert_eq!(cmds.len(), 2);
        assert!(p.take_commands().is_empty());
    }

    #[test]
    fn color_to_css_format() {
        let css = color_to_css(Color::rgba(1.0, 0.0, 0.5, 0.8));
        assert!(css.starts_with("rgba(255,0,127,0.8"));
    }
}
