//! Canvas widget — a custom drawing surface with declarative drawing commands.

use crate::core::style::TextStyle;
use crate::core::{Color, Position, Rect};
use crate::ontology::*;
use crate::runtime::Frame;
use crate::widget::Widget;

/// A drawing command for the canvas.
#[derive(Debug, Clone)]
pub enum DrawCommand {
    /// Draw a line between two points.
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        color: [u8; 4],
        width: f32,
    },
    /// Draw a rectangle outline.
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: [u8; 4],
        width: f32,
    },
    /// Draw a filled rectangle.
    FilledRect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: [u8; 4],
    },
    /// Draw a circle outline.
    Circle {
        cx: f32,
        cy: f32,
        radius: f32,
        color: [u8; 4],
        width: f32,
    },
    /// Draw a filled circle.
    FilledCircle {
        cx: f32,
        cy: f32,
        radius: f32,
        color: [u8; 4],
    },
    /// Draw text at a position.
    Text {
        x: f32,
        y: f32,
        text: String,
        size: f32,
        color: [u8; 4],
    },
}

/// A custom drawing canvas with a declarative drawing command list.
pub struct Canvas {
    agent_id: std::borrow::Cow<'static, str>,
    commands: Vec<DrawCommand>,
    background: Option<[u8; 4]>,
}

impl Canvas {
    #[must_use]
    pub fn new() -> Self {
        Self {
            agent_id: std::borrow::Cow::Borrowed(""),
            commands: Vec::new(),
            background: None,
        }
    }

    pub fn agent_id(mut self, id: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        self.agent_id = id.into();
        self
    }

    /// Set a background fill color.
    pub fn background(mut self, color: [u8; 4]) -> Self {
        self.background = Some(color);
        self
    }

    /// Add a line drawing command.
    pub fn line(mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: [u8; 4], width: f32) -> Self {
        self.commands.push(DrawCommand::Line {
            x1,
            y1,
            x2,
            y2,
            color,
            width,
        });
        self
    }

    /// Add a rectangle outline drawing command.
    pub fn rect(mut self, x: f32, y: f32, w: f32, h: f32, color: [u8; 4], width: f32) -> Self {
        self.commands.push(DrawCommand::Rect {
            x,
            y,
            w,
            h,
            color,
            width,
        });
        self
    }

    /// Add a filled rectangle drawing command.
    pub fn filled_rect(mut self, x: f32, y: f32, w: f32, h: f32, color: [u8; 4]) -> Self {
        self.commands
            .push(DrawCommand::FilledRect { x, y, w, h, color });
        self
    }

    /// Add a circle outline drawing command.
    pub fn circle(mut self, cx: f32, cy: f32, radius: f32, color: [u8; 4], width: f32) -> Self {
        self.commands.push(DrawCommand::Circle {
            cx,
            cy,
            radius,
            color,
            width,
        });
        self
    }

    /// Add a filled circle drawing command.
    pub fn filled_circle(mut self, cx: f32, cy: f32, radius: f32, color: [u8; 4]) -> Self {
        self.commands.push(DrawCommand::FilledCircle {
            cx,
            cy,
            radius,
            color,
        });
        self
    }

    /// Add a text drawing command.
    pub fn text(
        mut self,
        x: f32,
        y: f32,
        text: impl Into<String>,
        size: f32,
        color: [u8; 4],
    ) -> Self {
        self.commands.push(DrawCommand::Text {
            x,
            y,
            text: text.into(),
            size,
            color,
        });
        self
    }

    /// Add a raw draw command.
    pub fn draw(mut self, cmd: DrawCommand) -> Self {
        self.commands.push(cmd);
        self
    }
}

impl Default for Canvas {
    fn default() -> Self {
        Self::new()
    }
}

impl Discoverable for Canvas {
    fn schema(&self) -> WidgetSchema {
        let mut schema =
            WidgetSchema::new("Canvas", "A custom drawing surface", SemanticRole::Canvas);
        schema.usage_hint =
            Some("Canvas::new().rect(10.0, 10.0, 100.0, 50.0, [255,0,0,255])".into());
        schema.tags = vec![
            "canvas".into(),
            "drawing".into(),
            "graphics".into(),
            "custom".into(),
        ];
        schema
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![AgentCapability::Zoomable {
            min_zoom: 0.1,
            max_zoom: 10.0,
        }]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![AgentAction::simple(
            "clear",
            "Clear all drawing commands",
            true,
        )]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::Canvas
    }

    fn agent_state(&self) -> serde_json::Value {
        serde_json::json!({
            "command_count": self.commands.len(),
            "has_background": self.background.is_some(),
        })
    }

    fn execute_action(
        &mut self,
        action: &str,
        _params: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        match action {
            "clear" => {
                let count = self.commands.len();
                self.commands.clear();
                Ok(serde_json::json!({ "cleared": count }))
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
}

impl Widget for Canvas {
    fn render(self, area: Rect, frame: &mut Frame<'_>) {
        if !self.agent_id.is_empty() {
            if frame.ontology_enabled() {
                let node = UiNode::new("Canvas", SemanticRole::Canvas)
                    .with_id(self.agent_id.clone())
                    .with_bounds(area.into())
                    .with_property("command_count", serde_json::json!(self.commands.len()));
                frame.register_widget(node);
            }
            frame.register_hitbox(self.agent_id.clone(), area, 1);
        }

        // Background fill
        if let Some(bg) = self.background {
            let c = Color::from_rgba8(bg[0], bg[1], bg[2], bg[3]);
            frame.painter().fill_rect(area, c, 0.0);
        }

        // Border
        frame.painter().stroke_rect(area, Color::GRAY, 1.0, 0.0);

        // Execute draw commands
        for cmd in &self.commands {
            match cmd {
                DrawCommand::Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    color,
                    width,
                } => {
                    let c = Color::from_rgba8(color[0], color[1], color[2], color[3]);
                    frame.painter().line(
                        Position::new(area.x + x1, area.y + y1),
                        Position::new(area.x + x2, area.y + y2),
                        c,
                        *width,
                    );
                }
                DrawCommand::Rect {
                    x,
                    y,
                    w,
                    h,
                    color,
                    width,
                } => {
                    let c = Color::from_rgba8(color[0], color[1], color[2], color[3]);
                    frame.painter().stroke_rect(
                        Rect::new(area.x + x, area.y + y, *w, *h),
                        c,
                        *width,
                        0.0,
                    );
                }
                DrawCommand::FilledRect { x, y, w, h, color } => {
                    let c = Color::from_rgba8(color[0], color[1], color[2], color[3]);
                    frame
                        .painter()
                        .fill_rect(Rect::new(area.x + x, area.y + y, *w, *h), c, 0.0);
                }
                DrawCommand::Circle {
                    cx,
                    cy,
                    radius,
                    color,
                    width,
                } => {
                    let c = Color::from_rgba8(color[0], color[1], color[2], color[3]);
                    frame.painter().stroke_circle(
                        Position::new(area.x + cx, area.y + cy),
                        *radius,
                        c,
                        *width,
                    );
                }
                DrawCommand::FilledCircle {
                    cx,
                    cy,
                    radius,
                    color,
                } => {
                    let c = Color::from_rgba8(color[0], color[1], color[2], color[3]);
                    frame.painter().fill_circle(
                        Position::new(area.x + cx, area.y + cy),
                        *radius,
                        c,
                    );
                }
                DrawCommand::Text {
                    x,
                    y,
                    text,
                    size,
                    color,
                } => {
                    let c = Color::from_rgba8(color[0], color[1], color[2], color[3]);
                    let ts = TextStyle {
                        font_size: *size,
                        color: c,
                        ..Default::default()
                    };
                    frame
                        .painter()
                        .text(Position::new(area.x + x, area.y + y), text, &ts);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_clear_action() {
        let mut canvas = Canvas::new()
            .line(0.0, 0.0, 100.0, 100.0, [255, 0, 0, 255], 2.0)
            .circle(50.0, 50.0, 25.0, [0, 255, 0, 255], 1.0);

        assert_eq!(canvas.commands.len(), 2);

        let result = canvas
            .execute_action("clear", &serde_json::json!({}))
            .unwrap();
        assert_eq!(result["cleared"], 2);
        assert_eq!(canvas.commands.len(), 0);
    }

    #[test]
    fn canvas_draw_commands() {
        let canvas = Canvas::new()
            .background([0, 0, 0, 255])
            .filled_rect(10.0, 10.0, 80.0, 80.0, [255, 0, 0, 255])
            .filled_circle(50.0, 50.0, 20.0, [0, 0, 255, 255])
            .text(10.0, 10.0, "Hello", 14.0, [255, 255, 255, 255]);

        assert_eq!(canvas.commands.len(), 3);
        assert!(canvas.background.is_some());
    }
}
