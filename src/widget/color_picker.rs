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
/// The components of a `set_color` action, each present only if the agent sent
/// it.
///
/// `hex` is expanded into components here, so a handler reads one shape
/// whichever form the agent used.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ColorChange {
    pub r: Option<u8>,
    pub g: Option<u8>,
    pub b: Option<u8>,
    pub a: Option<u8>,
}

impl ColorChange {
    fn from_params(v: &serde_json::Value) -> Self {
        let mut change = Self::default();
        if let Some(hex) = v.get("hex").and_then(serde_json::Value::as_str) {
            let digits = hex.strip_prefix('#').unwrap_or(hex);
            let byte = |i: usize| u8::from_str_radix(digits.get(i..i + 2)?, 16).ok();
            if digits.len() >= 6 {
                change.r = byte(0);
                change.g = byte(2);
                change.b = byte(4);
            }
            if digits.len() >= 8 {
                change.a = byte(6);
            }
        }
        let byte = |k: &str| {
            v.get(k)
                .and_then(serde_json::Value::as_u64)
                .map(|n| n.min(255) as u8)
        };
        change.r = byte("r").or(change.r);
        change.g = byte("g").or(change.g);
        change.b = byte("b").or(change.b);
        change.a = byte("a").or(change.a);
        change
    }

    /// The colour that results from applying this change to `base`.
    ///
    /// Components the agent left out are taken from `base` unchanged.
    #[must_use]
    pub fn applied_to(self, base: Color) -> Color {
        let keep = |sent: Option<u8>, current: f32| sent.map_or(current, |n| f32::from(n) / 255.0);
        Color::rgba(
            keep(self.r, base.r),
            keep(self.g, base.g),
            keep(self.b, base.b),
            keep(self.a, base.a),
        )
    }
}

pub struct ColorPicker {
    label: String,
    show_alpha: bool,
    style: Style,
    agent_id: std::borrow::Cow<'static, str>,
    /// The change to apply when an agent sets the colour.
    on_color: Option<Box<dyn std::any::Any + Send>>,
}

impl ColorPicker {
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            show_alpha: true,
            style: Style::default(),
            agent_id: std::borrow::Cow::Borrowed(""),
            on_color: None,
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

    /// Name this picker and give it the change to apply on `set_color`.
    ///
    /// The handler receives a [`ColorChange`] rather than a `Color`, because
    /// every component of the action is optional: an agent that sets only the
    /// alpha must not turn the colour black. Call
    /// [`ColorChange::applied_to`] with the colour you currently hold.
    #[must_use]
    pub fn on_color<M: 'static>(
        mut self,
        id: impl Into<std::borrow::Cow<'static, str>>,
        f: impl FnOnce(&mut M, ColorChange) + Send + 'static,
    ) -> Self {
        self.agent_id = id.into();
        let handler: crate::runtime::ValueMutation<M> =
            Box::new(move |m: &mut M, v: &serde_json::Value| {
                f(m, ColorChange::from_params(v));
            });
        self.on_color = Some(Box::new(handler));
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

    fn render(mut self, area: Rect, frame: &mut Frame<'_>, state: &mut ColorPickerState) {
        if !self.agent_id.is_empty() {
            if let Some(handler) = self.on_color.take() {
                frame.register_message(self.agent_id.clone(), "set_color", handler);
            }
        }

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

#[cfg(test)]
mod change_tests {
    use super::ColorChange;

    fn parse(v: serde_json::Value) -> ColorChange {
        ColorChange::from_params(&v)
    }

    #[test]
    fn hex_and_channels_produce_the_same_change() {
        let from_hex = parse(serde_json::json!({"hex": "#204060"}));
        let from_channels = parse(serde_json::json!({"r": 0x20, "g": 0x40, "b": 0x60}));
        assert_eq!(from_hex, from_channels);
        assert_eq!(from_hex.a, None, "no alpha given, none reported");
    }

    #[test]
    fn eight_digit_hex_carries_alpha() {
        assert_eq!(parse(serde_json::json!({"hex": "204060ff"})).a, Some(255));
    }

    #[test]
    fn explicit_channels_win_over_hex() {
        let ch = parse(serde_json::json!({"hex": "#000000", "g": 255}));
        assert_eq!((ch.r, ch.g, ch.b), (Some(0), Some(255), Some(0)));
    }

    #[test]
    fn omitted_components_leave_the_base_colour_alone() {
        let base = crate::core::Color::rgba(0.5, 0.5, 0.5, 1.0);
        let out = parse(serde_json::json!({"g": 0})).applied_to(base);
        assert!((out.r - 0.5).abs() < 1e-6);
        assert!((out.g).abs() < 1e-6);
        assert!((out.a - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_malformed_hex_changes_nothing() {
        assert_eq!(
            parse(serde_json::json!({"hex": "zzz"})),
            ColorChange::default()
        );
    }
}
