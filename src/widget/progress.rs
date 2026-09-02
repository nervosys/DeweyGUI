//! Progress bar widget.

use crate::core::{Color, Position, Rect, Style};
use crate::ontology::*;
use crate::runtime::Frame;
use crate::widget::Widget;

/// A progress bar showing completion percentage.
pub struct ProgressBar {
    progress: f32,
    label: Option<String>,
    style: Style,
    agent_id: std::borrow::Cow<'static, str>,
}

impl ProgressBar {
    /// Create a progress bar. `progress` is clamped to `[0.0, 1.0]`.
    #[must_use]
    pub fn new(progress: f32) -> Self {
        Self {
            progress: progress.clamp(0.0, 1.0),
            label: None,
            style: Style::default(),
            agent_id: std::borrow::Cow::Borrowed(""),
        }
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
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

    pub fn rounded(mut self, radius: f32) -> Self {
        self.style.border_radius = Some(radius);
        self
    }

    pub fn agent_id(mut self, id: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        self.agent_id = id.into();
        self
    }
}

impl Discoverable for ProgressBar {
    fn schema(&self) -> WidgetSchema {
        let mut schema = WidgetSchema::new(
            "ProgressBar",
            "A progress indicator",
            SemanticRole::Progress,
        );
        schema.usage_hint = Some("ProgressBar::new(0.75).label(\"Loading...\")".into());
        schema.tags = vec!["progress".into(), "loading".into(), "bar".into()];
        schema
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::Progress
    }

    fn agent_state(&self) -> serde_json::Value {
        serde_json::json!({ "progress": self.progress })
    }

    fn execute_action(
        &mut self,
        _action: &str,
        _params: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        Err("ProgressBar has no actions".to_string())
    }

    fn agent_id(&self) -> Option<&str> {
        if self.agent_id.is_empty() {
            None
        } else {
            Some(&self.agent_id)
        }
    }

    fn accessibility_label(&self) -> Option<String> {
        self.label.clone()
    }
}

impl Widget for ProgressBar {
    fn render(self, area: Rect, frame: &mut Frame<'_>) {
        if frame.describes(area) && !self.agent_id.is_empty() {
            let node = UiNode::new("ProgressBar", SemanticRole::Progress)
                .with_id(self.agent_id.clone())
                .with_bounds(area.into())
                .with_property("progress", serde_json::json!(self.progress));
            frame.register_widget(node);
        }

        // Background track
        let track_bg = self.style.background.unwrap_or(Color::DARK_GRAY);
        let radius = self.style.border_radius.unwrap_or(4.0);
        frame.painter().fill_rect(area, track_bg, radius);
        // Filled portion
        let fill_color = self.style.foreground.unwrap_or(Color::BLUE);
        let fill_w = area.width * self.progress;
        if fill_w > 0.0 {
            let fill = Rect::new(area.x, area.y, fill_w, area.height);
            frame.painter().fill_rect(fill, fill_color, radius);
        }
        // Label
        if let Some(label) = &self.label {
            let mut ts = self.style.resolved_text();
            if ts.font_size == 14.0 {
                ts.font_size = 12.0; // Default smaller for progress labels
            }
            let text_size = frame.painter().measure_text(label, &ts);
            let tx = area.x + (area.width - text_size.width) * 0.5;
            let ty = area.y + (area.height - text_size.height) * 0.5;
            frame.painter().text(Position::new(tx, ty), label, &ts);
        }
    }
}
