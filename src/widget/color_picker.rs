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

/// # Agent view
///
/// What an agent sees of this widget, and what it can call on it.
/// Generated from the widget's own answers; `tests/agent_view.rs`
/// fails the build if this block and the code disagree.
///
/// - role: `Input`
/// - actions: `get_color`, `set_color`(r, g, b, a, hex), `toggle_open`
/// - state it publishes: `label`, `show_alpha`
/// - capabilities: `Clickable`, `Focusable`
/// - live value: held by the application in `ColorPickerState`, not by the widget
pub struct ColorPicker {
    label: String,
    show_alpha: bool,
    style: Style,
    agent_id: std::borrow::Cow<'static, str>,
    /// The change to apply when an agent sets the colour.
    on_color: Option<Box<dyn std::any::Any + Send>>,
    open: bool,
    on_open: Option<Box<dyn std::any::Any + Send>>,
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
            open: false,
            on_open: None,
        }
    }

    /// Whether the picker is showing.
    ///
    /// The application owns it, as it owns a `Modal`'s, a `Select`'s and a
    /// `Menu`'s. `ColorPickerState` holds only the colour, and a second field
    /// would break every `ColorPickerState { color }` already written.
    #[must_use]
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Give this picker the change to apply when it is opened or closed.
    ///
    /// Without it there is nothing to open: the widget draws a swatch, and
    /// clicking a swatch that cannot open is the state this was in — a
    /// display an agent could write to and a person could only look at.
    #[must_use]
    pub fn on_open<M: 'static>(
        mut self,
        id: impl Into<std::borrow::Cow<'static, str>>,
        f: impl FnOnce(&mut M, bool) + Send + 'static,
    ) -> Self {
        let open = !self.open;
        let wrapped: crate::runtime::Mutation<M> = Box::new(move |m: &mut M| f(m, open));
        self.agent_id = id.into();
        self.on_open = Some(Box::new(wrapped));
        self
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
            AgentAction::simple("toggle_open", "Show or hide the picker", true),
        ]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::Input
    }

    fn agent_state(&self) -> serde_json::Value {
        serde_json::json!({ "label": self.label, "show_alpha": self.show_alpha })
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

/// The open picker's layout. The painting and the click map share it, because
/// the same arithmetic written twice is how a click picks the wrong colour.
const SWATCH_MAX: f32 = 32.0;
const SV_SIZE: f32 = 128.0;
const HUE_HEIGHT: f32 = 16.0;
const GAP: f32 = 6.0;
/// How many cells the gradients are drawn as.
///
/// `Painter` has no gradient primitive — nine shapes, none of them a ramp — so
/// a continuous square is drawn as a grid of flat fills. Sixteen is where the
/// banding stops being the first thing you notice at this size; the click map
/// is continuous regardless, so the picked colour is not quantised to a cell.
const CELLS: usize = 16;

/// Hue in degrees, saturation and value in `0..=1`, to a `Color`.
fn hsv_to_color(h: f32, s: f32, v: f32, a: f32) -> Color {
    let h = h.rem_euclid(360.0);
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match h as u32 / 60 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    Color::rgba(r + m, g + m, b + m, a)
}

/// A `Color` back to hue, saturation and value.
///
/// Needed because the two controls move one coordinate each: dragging the hue
/// strip must keep the saturation and value the swatch already has.
fn color_to_hsv(c: Color) -> (f32, f32, f32) {
    let max = c.r.max(c.g).max(c.b);
    let min = c.r.min(c.g).min(c.b);
    let d = max - min;
    let h = if d <= f32::EPSILON {
        0.0
    } else if max == c.r {
        60.0 * (((c.g - c.b) / d) % 6.0)
    } else if max == c.g {
        60.0 * ((c.b - c.r) / d + 2.0)
    } else {
        60.0 * ((c.r - c.g) / d + 4.0)
    };
    let s = if max <= f32::EPSILON { 0.0 } else { d / max };
    (h.rem_euclid(360.0), s, max)
}

/// The colour under a point in the saturation-value square, for hue `h`.
fn sv_at(square: Rect, at: Position, h: f32, a: f32) -> Color {
    let s = ((at.x - square.x) / square.width).clamp(0.0, 1.0);
    let v = 1.0 - ((at.y - square.y) / square.height).clamp(0.0, 1.0);
    hsv_to_color(h, s, v, a)
}

impl StatefulWidget for ColorPicker {
    type State = ColorPickerState;

    fn render(mut self, area: Rect, frame: &mut Frame<'_>, state: &mut ColorPickerState) {
        if !self.agent_id.is_empty() {
            if let Some(handler) = self.on_color.take() {
                frame.register_message(self.agent_id.clone(), "set_color", handler);
            }
            if let Some(handler) = self.on_open.take() {
                frame.register_message(self.agent_id.clone(), "toggle_open", handler);
            }

            // A click on the swatch opens or closes the picker; a click in the
            // square picks a saturation and a value; a click on the strip
            // picks a hue. Each names its action, since none of them is
            // "whichever handler was registered first".
            //
            // Until the square and the strip were drawn there was nothing on
            // screen to read a colour off, so a click supplied none and
            // `set_color` read its defaults: black.
            let swatch_size = area.height.min(area.width).min(SWATCH_MAX);
            let swatch = Rect::new(area.x, area.y, swatch_size, swatch_size);
            let square = Rect::new(area.x, area.y + swatch_size + 20.0, SV_SIZE, SV_SIZE);
            let strip = Rect::new(
                square.x,
                square.y + square.height + GAP,
                SV_SIZE,
                HUE_HEIGHT,
            );
            let open = self.open;
            let colour = state.color;
            frame.register_click(
                self.agent_id.clone(),
                crate::runtime::ClickParams::from_position(move |at| {
                    if swatch.contains(at) {
                        return Some(crate::runtime::Click::action(
                            "toggle_open",
                            serde_json::Value::Null,
                        ));
                    }
                    if !open {
                        return None;
                    }
                    let (h, s_now, v_now) = color_to_hsv(colour);
                    let picked = if square.contains(at) {
                        sv_at(square, at, h, colour.a)
                    } else if strip.contains(at) {
                        // The strip moves one coordinate. Keeping the other two
                        // is why `color_to_hsv` exists: read back as `r`, `g`,
                        // `b`, a new hue alone cannot be expressed.
                        let hue = ((at.x - strip.x) / strip.width).clamp(0.0, 1.0) * 360.0;
                        hsv_to_color(hue, s_now, v_now, colour.a)
                    } else {
                        return None;
                    };
                    Some(crate::runtime::Click::action(
                        "set_color",
                        serde_json::json!({
                            "r": (picked.r * 255.0).round() as u8,
                            "g": (picked.g * 255.0).round() as u8,
                            "b": (picked.b * 255.0).round() as u8,
                        }),
                    ))
                }),
            );
        }

        if !self.agent_id.is_empty() {
            if frame.describes(area) {
                let node = UiNode::new("ColorPicker", SemanticRole::Input)
                    .with_id(self.agent_id.clone())
                    .with_bounds(area.into())
                    .with_label(&self.label)
                    .with_property("r", serde_json::json!((state.color.r * 255.0) as u8))
                    .with_property("g", serde_json::json!((state.color.g * 255.0) as u8))
                    .with_property("b", serde_json::json!((state.color.b * 255.0) as u8))
                    .with_property("a", serde_json::json!((state.color.a * 255.0) as u8))
                    .with_property("open", serde_json::json!(self.open));
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

        // The picker itself, over whatever comes after it in the view. Drawn
        // as a grid of flat fills because `Painter` has no gradient: nine
        // shapes, none of them a ramp.
        if self.open {
            let (hue, sat, val) = color_to_hsv(state.color);
            let alpha = state.color.a;
            let square = Rect::new(area.x, area.y + swatch_size + 20.0, SV_SIZE, SV_SIZE);
            let strip = Rect::new(
                square.x,
                square.y + square.height + GAP,
                SV_SIZE,
                HUE_HEIGHT,
            );
            frame.overlay(move |frame| {
                let cell = SV_SIZE / CELLS as f32;
                for row in 0..CELLS {
                    for col in 0..CELLS {
                        // The centre of the cell, so the swatch shown is the
                        // colour the middle of it would pick.
                        let s = (col as f32 + 0.5) / CELLS as f32;
                        let v = 1.0 - (row as f32 + 0.5) / CELLS as f32;
                        frame.painter().fill_rect(
                            Rect::new(
                                square.x + col as f32 * cell,
                                square.y + row as f32 * cell,
                                cell + 0.5,
                                cell + 0.5,
                            ),
                            hsv_to_color(hue, s, v, 1.0),
                            0.0,
                        );
                    }
                }
                frame.painter().stroke_rect(square, Color::GRAY, 1.0, 2.0);
                // Where the current colour sits in the square.
                frame.painter().stroke_circle(
                    Position::new(
                        square.x + sat * square.width,
                        square.y + (1.0 - val) * square.height,
                    ),
                    5.0,
                    Color::WHITE,
                    2.0,
                );

                let hue_cell = SV_SIZE / CELLS as f32;
                for col in 0..CELLS {
                    let h = (col as f32 + 0.5) / CELLS as f32 * 360.0;
                    frame.painter().fill_rect(
                        Rect::new(
                            strip.x + col as f32 * hue_cell,
                            strip.y,
                            hue_cell + 0.5,
                            strip.height,
                        ),
                        hsv_to_color(h, 1.0, 1.0, 1.0),
                        0.0,
                    );
                }
                frame.painter().stroke_rect(strip, Color::GRAY, 1.0, 2.0);
                frame.painter().stroke_rect(
                    Rect::new(
                        strip.x + hue / 360.0 * strip.width - 2.0,
                        strip.y,
                        4.0,
                        strip.height,
                    ),
                    Color::WHITE,
                    2.0,
                    0.0,
                );
                let _ = alpha;
            });
        }
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

/// The two conversions have to agree, because the hue strip reads a colour
/// apart and puts it back together with one coordinate changed. If they
/// disagree, dragging the strip quietly shifts the saturation too.
#[test]
fn hsv_survives_a_round_trip() {
    for (h, s, v) in [
        (0.0, 1.0, 1.0),
        (120.0, 1.0, 1.0),
        (240.0, 0.5, 0.25),
        (300.0, 0.75, 0.9),
        (59.0, 0.1, 1.0),
    ] {
        let c = hsv_to_color(h, s, v, 1.0);
        let (h2, s2, v2) = color_to_hsv(c);
        assert!((h2 - h).abs() < 1.0, "hue {h} came back as {h2}");
        assert!((s2 - s).abs() < 0.01, "saturation {s} came back as {s2}");
        assert!((v2 - v).abs() < 0.01, "value {v} came back as {v2}");
    }
}

/// Grey has no hue to speak of, and asking for one must not divide by the
/// zero difference between its channels.
#[test]
fn grey_has_no_hue_and_does_not_divide_by_zero() {
    let (h, s, v) = color_to_hsv(Color::rgba(0.5, 0.5, 0.5, 1.0));
    assert_eq!(h, 0.0);
    assert_eq!(s, 0.0);
    assert!((v - 0.5).abs() < 0.001);

    let (h, s, v) = color_to_hsv(Color::rgba(0.0, 0.0, 0.0, 1.0));
    assert_eq!((h, s, v), (0.0, 0.0, 0.0));
}

/// The corners of the square are the colours a person expects to find
/// there, which is what makes the control readable at all.
#[test]
fn the_corners_of_the_square_are_what_they_look_like() {
    let square = Rect::new(0.0, 0.0, 100.0, 100.0);
    let top_right = sv_at(square, Position::new(100.0, 0.0), 0.0, 1.0);
    assert!(top_right.r > 0.99 && top_right.g < 0.01, "{top_right:?}");

    let bottom_left = sv_at(square, Position::new(0.0, 100.0), 0.0, 1.0);
    assert!(
        bottom_left.r < 0.01 && bottom_left.b < 0.01,
        "{bottom_left:?}"
    );

    let top_left = sv_at(square, Position::new(0.0, 0.0), 0.0, 1.0);
    assert!(
        top_left.r > 0.99 && top_left.g > 0.99 && top_left.b > 0.99,
        "the top left is white, got {top_left:?}"
    );
}

/// A point outside the square still names a colour inside it, so a drag
/// that slips off the edge holds at the edge rather than wrapping.
#[test]
fn a_point_outside_the_square_is_clamped_to_it() {
    let square = Rect::new(10.0, 10.0, 100.0, 100.0);
    let far = sv_at(square, Position::new(999.0, -999.0), 0.0, 1.0);
    let corner = sv_at(square, Position::new(110.0, 10.0), 0.0, 1.0);
    assert_eq!((far.r, far.g, far.b), (corner.r, corner.g, corner.b));
}
