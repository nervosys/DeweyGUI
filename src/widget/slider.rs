//! Slider widget — a draggable value selector.

use crate::core::{Color, Position, Rect, Style};
use crate::ontology::*;
use crate::runtime::Frame;
use crate::widget::StatefulWidget;

/// Slider state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SliderState {
    pub value: f64,
}

impl SliderState {
    #[must_use]
    pub fn new(value: f64) -> Self {
        Self { value }
    }
}

impl Default for SliderState {
    fn default() -> Self {
        Self { value: 0.0 }
    }
}

/// A draggable slider.
pub struct Slider {
    min: f64,
    max: f64,
    step: f64,
    label: String,
    style: Style,
    agent_id: std::borrow::Cow<'static, str>,
    on_value: Option<Box<dyn std::any::Any + Send>>,
}

impl Slider {
    #[must_use]
    pub fn new(min: f64, max: f64) -> Self {
        Self {
            min,
            max,
            step: 1.0,
            label: String::new(),
            style: Style::default(),
            agent_id: std::borrow::Cow::Borrowed(""),
            on_value: None,
        }
    }

    pub fn step(mut self, step: f64) -> Self {
        self.step = step;
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
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

    pub fn bg(mut self, color: Color) -> Self {
        self.style.background = Some(color);
        self
    }

    /// Name this widget and give it the change to apply when its value moves.
    ///
    /// The value arrives the same way whether a person edited the widget or an
    /// agent sent `execute_action(id, "set_value", {"value": ...})`, so the
    /// application writes no `execute_action` handler for it.
    ///
    /// ```no_run
    /// # use dewey::prelude::*;
    /// # use dewey::widget::Slider;
    /// # struct App { volume: f64 }
    /// Slider::new(0.0, 1.0).on_change("vol", |app: &mut App, v: f64| app.volume = v);
    /// ```
    #[must_use]
    pub fn on_change<M: 'static>(
        mut self,
        id: impl Into<std::borrow::Cow<'static, str>>,
        f: impl FnOnce(&mut M, f64) + Send + 'static,
    ) -> Self {
        let wrapped: crate::runtime::ValueMutation<M> =
            Box::new(move |m: &mut M, v: &serde_json::Value| {
                let value = v
                    .get("value")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(0.0);
                f(m, value)
            });
        self.agent_id = id.into();
        self.on_value = Some(Box::new(wrapped));
        self
    }

    pub fn agent_id(mut self, id: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        self.agent_id = id.into();
        self
    }
}

impl Discoverable for Slider {
    fn schema(&self) -> WidgetSchema {
        let mut schema =
            WidgetSchema::new("Slider", "A draggable value slider", SemanticRole::Input);
        schema.usage_hint = Some("Slider::new(0.0, 100.0).step(1.0).label(\"Volume\")".into());
        schema.tags = vec!["slider".into(), "range".into(), "value".into()];
        schema
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![
            AgentCapability::RangeEditable {
                min: self.min,
                max: self.max,
                step: Some(self.step),
            },
            AgentCapability::Focusable,
        ]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![AgentAction::with_params(
            "set_value",
            "Set the slider value",
            vec![ActionParam::required(
                "value",
                "The value to set",
                ActionParamType::Float,
            )],
            true,
        )]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::Input
    }

    fn agent_state(&self) -> serde_json::Value {
        serde_json::json!({ "min": self.min, "max": self.max, "step": self.step })
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
        if self.label.is_empty() {
            None
        } else {
            Some(self.label.clone())
        }
    }
}

impl StatefulWidget for Slider {
    type State = SliderState;

    fn render(mut self, area: Rect, frame: &mut Frame<'_>, state: &mut SliderState) {
        if !self.agent_id.is_empty() {
            if frame.ontology_enabled() {
                let node = UiNode::new("Slider", SemanticRole::Input)
                    .with_id(self.agent_id.clone())
                    .with_bounds(area.into())
                    .with_property("value", serde_json::json!(state.value))
                    .with_property("min", serde_json::json!(self.min))
                    .with_property("max", serde_json::json!(self.max));
                frame.register_widget(node);
            }
            frame.register_hitbox(self.agent_id.clone(), area, 1);
            if let Some(handler) = self.on_value.take() {
                frame.register_message(self.agent_id.clone(), "set_value", handler);
            }
        }

        // Track
        let track_h = 4.0;
        let track_y = area.y + (area.height - track_h) * 0.5;
        let track = Rect::new(area.x, track_y, area.width, track_h);
        let track_bg = self.style.background.unwrap_or(Color::DARK_GRAY);
        frame.painter().fill_rect(track, track_bg, 2.0);
        // Filled portion
        let frac = if self.max > self.min {
            ((state.value - self.min) / (self.max - self.min)).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        let fill_color = self.style.foreground.unwrap_or(Color::BLUE);
        let fill = Rect::new(area.x, track_y, area.width * frac, track_h);
        frame.painter().fill_rect(fill, fill_color, 2.0);
        // Thumb
        let thumb_x = area.x + area.width * frac;
        let thumb_center = Position::new(thumb_x, area.y + area.height * 0.5);
        frame.painter().fill_circle(thumb_center, 7.0, Color::WHITE);
        // Label
        if !self.label.is_empty() {
            let mut ts = self.style.resolved_text();
            if ts.font_size == 14.0 {
                ts.font_size = 12.0;
            }
            let val_text = format!("{}: {:.1}", self.label, state.value);
            frame
                .painter()
                .text(Position::new(area.x, area.y), &val_text, &ts);
        }
    }
}
