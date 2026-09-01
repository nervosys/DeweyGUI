//! Color picker widget — an interactive color selector.

use crate::core::style::TextStyle;
use crate::core::{Color, Position, Rect, Style};
use crate::ontology::*;
use crate::runtime::Frame;
use crate::widget::StatefulWidget;

/// Persistent state for a color picker.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorPickerState {
    pub color: Color,
}

impl ColorPickerState {
    #[must_use]
    pub fn new(color: Color) -> Self {
        Self { color }
    }
}

impl Default for ColorPickerState {
    fn default() -> Self {
        Self {
            color: Color::WHITE,
        }
    }
}

/// An interactive color picker widget.
///
/// Displays a color swatch and allows the agent to get/set the selected color
/// via RGBA components or hex string.
pub struct ColorPicker {
    label: String,
    show_alpha: bool,
    style: Style,
    agent_id: std::borrow::Cow<'static, str>,
}

impl ColorPicker {
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            show_alpha: true,
            style: Style::default(),
            agent_id: std::borrow::Cow::Borrowed(""),
        }
    }

    pub fn show_alpha(mut self, show: bool) -> Self {
        self.show_alpha = show;
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn fg(mut self, color: Color) -> Self {
        self.style.foreground = Some(color);
        self
    }

    pub fn agent_id(mut self, id: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        self.agent_id = id.into();
        self
    }
}

impl Discoverable for ColorPicker {
    fn schema(&self) -> WidgetSchema {
        let mut schema = WidgetSchema::new(
            "ColorPicker",
            "An interactive color selector",
            SemanticRole::Input,
        );
        schema.usage_hint = Some("ColorPicker::new(\"Color\").show_alpha(true)".into());
        schema.tags = vec!["color".into(), "picker".into(), "palette".into()];
        schema
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![AgentCapability::Focusable, AgentCapability::Clickable]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![
            AgentAction::with_params(
                "set_color",
                "Set the selected color",
                vec![
                    ActionParam::optional(
                        "r",
                        "Red (0-255)",
                        ActionParamType::Integer,
                        serde_json::json!(0),
                    ),
                    ActionParam::optional(
                        "g",
                        "Green (0-255)",
                        ActionParamType::Integer,
                        serde_json::json!(0),
                    ),
                    ActionParam::optional(
                        "b",
                        "Blue (0-255)",
                        ActionParamType::Integer,
                        serde_json::json!(0),
                    ),
                    ActionParam::optional(
                        "a",
                        "Alpha (0-255)",
                        ActionParamType::Integer,
                        serde_json::json!(255),
                    ),
                    ActionParam::optional(
                        "hex",
                        "Hex color string (#RRGGBB or #RRGGBBAA)",
                        ActionParamType::String,
                        serde_json::json!(""),
                    ),
                ],
                true,
            ),
            AgentAction::simple("get_color", "Get the current color", false),
        ]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::Input
    }

    fn agent_state(&self) -> serde_json::Value {
        serde_json::json!({ "label": self.label, "show_alpha": self.show_alpha })
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
        Some(self.label.clone())
    }
}

impl StatefulWidget for ColorPicker {
    type State = ColorPickerState;

    fn render(self, area: Rect, frame: &mut Frame<'_>, state: &mut ColorPickerState) {
        if !self.agent_id.is_empty() {
            if frame.ontology_enabled() {
                let node = UiNode::new("ColorPicker", SemanticRole::Input)
                    .with_id(self.agent_id.clone())
                    .with_bounds(area.into())
                    .with_label(&self.label)
                    .with_property("r", serde_json::json!((state.color.r * 255.0) as u8))
                    .with_property("g", serde_json::json!((state.color.g * 255.0) as u8))
                    .with_property("b", serde_json::json!((state.color.b * 255.0) as u8))
                    .with_property("a", serde_json::json!((state.color.a * 255.0) as u8));
                frame.register_widget(node);
            }
            frame.register_hitbox(self.agent_id.clone(), area, 1);
        }

        // Draw a color swatch filled with the current color
        let swatch_size = area.height.min(area.width).min(32.0);
        let swatch = Rect::new(area.x, area.y, swatch_size, swatch_size);
        frame.painter().fill_rect(swatch, state.color, 4.0);
        frame.painter().stroke_rect(swatch, Color::GRAY, 1.0, 4.0);

        // Label to the right
        if !self.label.is_empty() {
            let ts = self.style.resolved_text();
            frame.painter().text(
                Position::new(area.x + swatch_size + 8.0, area.y + 4.0),
                &self.label,
                &ts,
            );
        }

        // Hex value below
        let hex = format!(
            "#{:02X}{:02X}{:02X}{:02X}",
            (state.color.r * 255.0) as u8,
            (state.color.g * 255.0) as u8,
            (state.color.b * 255.0) as u8,
            (state.color.a * 255.0) as u8,
        );
        let hex_ts = TextStyle {
            font_size: 12.0,
            color: Color::GRAY,
            ..Default::default()
        };
        frame.painter().text(
            Position::new(area.x, area.y + swatch_size + 4.0),
            &hex,
            &hex_ts,
        );
    }
}
