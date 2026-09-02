//! Chart widget — renders line, bar, and pie charts via the Painter API.

use crate::core::style::{Color, FontWeight, TextStyle};
use crate::core::{Position, Rect};
use crate::ontology::*;
use crate::runtime::Frame;
use crate::widget::Widget;

/// A data series for charting.
#[derive(Debug, Clone)]
pub struct Series {
    /// Series label.
    pub label: String,
    /// Data points (x, y) or (label_index, value) depending on chart type.
    pub values: Vec<f64>,
    /// Series color.
    pub color: Color,
}

impl Series {
    #[must_use]
    pub fn new(label: impl Into<String>, values: Vec<f64>, color: Color) -> Self {
        Self {
            label: label.into(),
            values,
            color,
        }
    }
}

/// The type of chart to render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartKind {
    Line,
    Bar,
    Pie,
}

/// A chart widget that renders data visualizations.
///
/// # Examples
///
/// ```
/// # use dewey::prelude::*;
/// Chart::line("Revenue")
///     .labels(vec!["Q1".into(), "Q2".into(), "Q3".into()])
///     .series(Series::new("2025", vec![100.0, 150.0, 200.0], Color::BLUE))
///     .series(Series::new("2026", vec![120.0, 180.0, 250.0], Color::GREEN));
/// ```
/// What an agent asked a [`Chart`] to do.
#[derive(Debug, Clone, PartialEq)]
pub enum ChartChange<'a> {
    /// Add a series under this label.
    AddSeries { label: &'a str, values: Vec<f64> },
    /// Remove the series at this index.
    RemoveSeries(usize),
    /// Remove every series.
    Clear,
}

pub struct Chart {
    kind: ChartKind,
    title: String,
    labels: Vec<String>,
    series: Vec<Series>,
    agent_id: std::borrow::Cow<'static, str>,
    handlers: Vec<(&'static str, Box<dyn std::any::Any + Send>)>,
}

impl Chart {
    /// Create a new line chart.
    #[must_use]
    pub fn line(title: impl Into<String>) -> Self {
        Self {
            kind: ChartKind::Line,
            title: title.into(),
            labels: Vec::new(),
            series: Vec::new(),
            agent_id: std::borrow::Cow::Borrowed(""),
            handlers: Vec::new(),
        }
    }

    /// Create a new bar chart.
    #[must_use]
    pub fn bar(title: impl Into<String>) -> Self {
        Self {
            kind: ChartKind::Bar,
            title: title.into(),
            labels: Vec::new(),
            series: Vec::new(),
            agent_id: std::borrow::Cow::Borrowed(""),
            handlers: Vec::new(),
        }
    }

    /// Create a new pie chart.
    #[must_use]
    pub fn pie(title: impl Into<String>) -> Self {
        Self {
            kind: ChartKind::Pie,
            title: title.into(),
            labels: Vec::new(),
            series: Vec::new(),
            agent_id: std::borrow::Cow::Borrowed(""),
            handlers: Vec::new(),
        }
    }

    /// Set the X-axis labels.
    pub fn labels(mut self, labels: Vec<String>) -> Self {
        self.labels = labels;
        self
    }

    /// Add a data series.
    pub fn series(mut self, series: Series) -> Self {
        self.series.push(series);
        self
    }

    /// Set the agent ID.
    /// Name this chart and give it the change to apply for every action it
    /// advertises: `add_series`, `remove_series` and `clear`.
    ///
    /// The series are rebuilt from the model each frame, so a change applied
    /// to the widget lasts until the next redraw and no longer.
    #[must_use]
    pub fn on_change<M: 'static>(
        mut self,
        id: impl Into<std::borrow::Cow<'static, str>>,
        f: impl Fn(&mut M, ChartChange<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.agent_id = id.into();
        let f = std::sync::Arc::new(f);
        for action in ["add_series", "remove_series", "clear"] {
            let f = f.clone();
            let handler: crate::runtime::ValueMutation<M> =
                Box::new(move |m: &mut M, v: &serde_json::Value| {
                    let change = match action {
                        "add_series" => ChartChange::AddSeries {
                            label: v
                                .get("label")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or_default(),
                            values: v
                                .get("values")
                                .and_then(serde_json::Value::as_array)
                                .map(|vs| {
                                    vs.iter()
                                        .filter_map(serde_json::Value::as_f64)
                                        .collect::<Vec<_>>()
                                })
                                .unwrap_or_default(),
                        },
                        "remove_series" => ChartChange::RemoveSeries(
                            v.get("index")
                                .and_then(serde_json::Value::as_u64)
                                .unwrap_or(0) as usize,
                        ),
                        _ => ChartChange::Clear,
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
}

impl Discoverable for Chart {
    fn schema(&self) -> WidgetSchema {
        let mut schema = WidgetSchema::new(
            "Chart",
            "A data visualization chart (line, bar, or pie)",
            SemanticRole::DataVisualization,
        );
        schema.usage_hint = Some(
            "Chart::line(\"Title\").series(Series::new(\"s1\", vec![1.0,2.0], Color::RED))".into(),
        );
        schema.tags = vec![
            "chart".into(),
            "graph".into(),
            "plot".into(),
            "data".into(),
            "visualization".into(),
        ];
        schema
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![AgentCapability::Focusable]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![
            AgentAction::with_params(
                "add_series",
                "Add a data series to the chart",
                vec![
                    ActionParam::required("label", "Series label", ActionParamType::String),
                    ActionParam::required("values", "Data values array", ActionParamType::Any),
                ],
                true,
            ),
            AgentAction::with_params(
                "remove_series",
                "Remove a series by index",
                vec![ActionParam::required(
                    "index",
                    "Series index",
                    ActionParamType::Index,
                )],
                true,
            ),
            AgentAction::simple("clear", "Remove all series", true),
        ]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::DataVisualization
    }

    fn agent_state(&self) -> serde_json::Value {
        let series: Vec<serde_json::Value> = self
            .series
            .iter()
            .map(|s| {
                serde_json::json!({
                    "label": s.label,
                    "values": s.values,
                    "color": format!("rgba({:.2},{:.2},{:.2},{:.2})", s.color.r, s.color.g, s.color.b, s.color.a),
                })
            })
            .collect();
        serde_json::json!({
            "title": self.title,
            "kind": format!("{:?}", self.kind),
            "series_count": self.series.len(),
            "series": series,
            "labels": self.labels,
        })
    }

    fn execute_action(
        &mut self,
        action: &str,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        match action {
            "add_series" => {
                let label = params["label"].as_str().ok_or("missing label")?.to_string();
                let values: Vec<f64> = params["values"]
                    .as_array()
                    .ok_or("missing values array")?
                    .iter()
                    .filter_map(|v| v.as_f64())
                    .collect();
                let color = PALETTE[self.series.len() % PALETTE.len()];
                self.series.push(Series::new(label, values, color));
                Ok(serde_json::json!({ "series_count": self.series.len() }))
            }
            "remove_series" => {
                let idx = params["index"].as_u64().ok_or("missing index")? as usize;
                if idx >= self.series.len() {
                    return Err(format!("index {idx} out of range ({})", self.series.len()));
                }
                self.series.remove(idx);
                Ok(serde_json::json!({ "series_count": self.series.len() }))
            }
            "clear" => {
                self.series.clear();
                Ok(serde_json::json!({ "series_count": 0 }))
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
        Some(format!("{} ({:?} chart)", self.title, self.kind))
    }
}

/// Default chart palette colors.
const PALETTE: &[Color] = &[
    Color {
        r: 0.35,
        g: 0.55,
        b: 0.95,
        a: 1.0,
    },
    Color {
        r: 0.90,
        g: 0.30,
        b: 0.30,
        a: 1.0,
    },
    Color {
        r: 0.25,
        g: 0.80,
        b: 0.40,
        a: 1.0,
    },
    Color {
        r: 0.95,
        g: 0.70,
        b: 0.20,
        a: 1.0,
    },
    Color {
        r: 0.60,
        g: 0.35,
        b: 0.85,
        a: 1.0,
    },
    Color {
        r: 0.0,
        g: 0.80,
        b: 0.80,
        a: 1.0,
    },
];

impl Widget for Chart {
    fn render(mut self, area: Rect, frame: &mut Frame<'_>) {
        if !self.agent_id.is_empty() {
            for (action, handler) in self.handlers.drain(..) {
                frame.register_message(self.agent_id.clone(), action, handler);
            }
        }

        if frame.ontology_enabled() && !self.agent_id.is_empty() {
            let node = UiNode::new("Chart", SemanticRole::DataVisualization)
                .with_id(self.agent_id.clone())
                .with_bounds(area.into())
                .with_property("title", serde_json::json!(self.title))
                .with_property("kind", serde_json::json!(format!("{:?}", self.kind)));
            frame.register_widget(node);
        }

        // Background
        frame
            .painter()
            .fill_rect(area, Color::rgba(0.08, 0.08, 0.10, 1.0), 4.0);
        frame
            .painter()
            .stroke_rect(area, Color::DARK_GRAY, 1.0, 4.0);

        // Title
        let title_ts = TextStyle {
            font_size: 15.0,
            color: Color::WHITE,
            weight: FontWeight::Bold,
            ..Default::default()
        };
        frame.painter().text(
            Position::new(area.x + 8.0, area.y + 6.0),
            &self.title,
            &title_ts,
        );

        let chart_area = Rect::new(
            area.x + 40.0,
            area.y + 30.0,
            area.width - 50.0,
            area.height - 50.0,
        );

        if chart_area.width <= 0.0 || chart_area.height <= 0.0 {
            return;
        }

        match self.kind {
            ChartKind::Line => self.render_line(chart_area, frame),
            ChartKind::Bar => self.render_bar(chart_area, frame),
            ChartKind::Pie => self.render_pie(chart_area, frame),
        }
    }
}

impl Chart {
    fn render_line(&self, area: Rect, frame: &mut Frame<'_>) {
        // Axes
        frame.painter().line(
            Position::new(area.x, area.y),
            Position::new(area.x, area.bottom()),
            Color::GRAY,
            1.0,
        );
        frame.painter().line(
            Position::new(area.x, area.bottom()),
            Position::new(area.right(), area.bottom()),
            Color::GRAY,
            1.0,
        );

        if self.series.is_empty() {
            return;
        }

        // Find data range
        let (min_val, max_val) = self.data_range();
        let range = if (max_val - min_val).abs() < f64::EPSILON {
            1.0
        } else {
            max_val - min_val
        };

        let label_ts = TextStyle {
            font_size: 10.0,
            color: Color::LIGHT_GRAY,
            ..Default::default()
        };

        // Draw X labels
        let max_points = self
            .series
            .iter()
            .map(|s| s.values.len())
            .max()
            .unwrap_or(0);
        if max_points > 1 {
            for (i, label) in self.labels.iter().enumerate().take(max_points) {
                let x = area.x + (i as f64 / (max_points - 1) as f64) as f32 * area.width;
                frame
                    .painter()
                    .text(Position::new(x, area.bottom() + 4.0), label, &label_ts);
            }
        }

        // Draw series
        for (si, s) in self.series.iter().enumerate() {
            let color = if s.color != Color::TRANSPARENT {
                s.color
            } else {
                PALETTE[si % PALETTE.len()]
            };

            let n = s.values.len();
            if n < 2 {
                continue;
            }

            for i in 0..n - 1 {
                let x1 = area.x + (i as f64 / (n - 1) as f64) as f32 * area.width;
                let x2 = area.x + ((i + 1) as f64 / (n - 1) as f64) as f32 * area.width;
                let y1 = area.bottom() - ((s.values[i] - min_val) / range) as f32 * area.height;
                let y2 = area.bottom() - ((s.values[i + 1] - min_val) / range) as f32 * area.height;

                frame
                    .painter()
                    .line(Position::new(x1, y1), Position::new(x2, y2), color, 2.0);
            }

            // Data points
            for i in 0..n {
                let x = area.x + (i as f64 / (n - 1) as f64) as f32 * area.width;
                let y = area.bottom() - ((s.values[i] - min_val) / range) as f32 * area.height;
                frame.painter().fill_circle(Position::new(x, y), 3.0, color);
            }
        }
    }

    fn render_bar(&self, area: Rect, frame: &mut Frame<'_>) {
        // Axes
        frame.painter().line(
            Position::new(area.x, area.y),
            Position::new(area.x, area.bottom()),
            Color::GRAY,
            1.0,
        );
        frame.painter().line(
            Position::new(area.x, area.bottom()),
            Position::new(area.right(), area.bottom()),
            Color::GRAY,
            1.0,
        );

        if self.series.is_empty() {
            return;
        }

        let (min_val, max_val) = self.data_range();
        let min_val = min_val.min(0.0);
        let range = if (max_val - min_val).abs() < f64::EPSILON {
            1.0
        } else {
            max_val - min_val
        };

        let max_points = self
            .series
            .iter()
            .map(|s| s.values.len())
            .max()
            .unwrap_or(0);
        if max_points == 0 {
            return;
        }

        let series_count = self.series.len();
        let group_w = area.width / max_points as f32;
        let bar_w = (group_w * 0.8) / series_count as f32;
        let gap = group_w * 0.1;

        let label_ts = TextStyle {
            font_size: 10.0,
            color: Color::LIGHT_GRAY,
            ..Default::default()
        };

        for (i, label) in self.labels.iter().enumerate().take(max_points) {
            let x = area.x + i as f32 * group_w + gap;
            frame
                .painter()
                .text(Position::new(x, area.bottom() + 4.0), label, &label_ts);
        }

        for (si, s) in self.series.iter().enumerate() {
            let color = if s.color != Color::TRANSPARENT {
                s.color
            } else {
                PALETTE[si % PALETTE.len()]
            };

            for (i, &val) in s.values.iter().enumerate() {
                let bar_h = ((val - min_val) / range) as f32 * area.height;
                let x = area.x + i as f32 * group_w + gap + si as f32 * bar_w;
                let y = area.bottom() - bar_h;
                frame
                    .painter()
                    .fill_rect(Rect::new(x, y, bar_w, bar_h), color, 2.0);
            }
        }
    }

    fn render_pie(&self, area: Rect, frame: &mut Frame<'_>) {
        let first = match self.series.first() {
            Some(s) => s,
            None => return,
        };

        let total: f64 = first.values.iter().sum();
        if total <= 0.0 {
            return;
        }

        let cx = area.x + area.width / 2.0;
        let cy = area.y + area.height / 2.0;
        let radius = area.width.min(area.height) / 2.0 - 10.0;

        if radius <= 0.0 {
            return;
        }

        // Draw filled segments as wedge approximations
        let mut start_angle = 0.0_f64;
        let label_ts = TextStyle {
            font_size: 11.0,
            color: Color::WHITE,
            ..Default::default()
        };

        for (i, &val) in first.values.iter().enumerate() {
            let sweep = (val / total) * std::f64::consts::TAU;
            let color = PALETTE[i % PALETTE.len()];

            // Draw wedge as filled triangles from center
            let steps = ((sweep * 30.0) as usize).max(4);
            for step in 0..steps {
                let a1 = start_angle + (step as f64 / steps as f64) * sweep;
                let a2 = start_angle + ((step + 1) as f64 / steps as f64) * sweep;

                let p1 = Position::new(
                    cx + (a1.cos() as f32) * radius,
                    cy + (a1.sin() as f32) * radius,
                );
                let p2 = Position::new(
                    cx + (a2.cos() as f32) * radius,
                    cy + (a2.sin() as f32) * radius,
                );

                // Draw as lines from center to arc (approximated)
                frame.painter().line(Position::new(cx, cy), p1, color, 2.0);
                frame.painter().line(p1, p2, color, 2.0);
            }

            // Label
            let mid_angle = start_angle + sweep / 2.0;
            let label_r = radius * 0.65;
            let lx = cx + (mid_angle.cos() as f32) * label_r;
            let ly = cy + (mid_angle.sin() as f32) * label_r;
            let label = self
                .labels
                .get(i)
                .cloned()
                .unwrap_or_else(|| format!("{:.0}%", val / total * 100.0));
            frame
                .painter()
                .text(Position::new(lx - 10.0, ly - 6.0), &label, &label_ts);

            start_angle += sweep;
        }

        // Border circle
        frame
            .painter()
            .stroke_circle(Position::new(cx, cy), radius, Color::GRAY, 1.0);
    }

    fn data_range(&self) -> (f64, f64) {
        let mut min = f64::MAX;
        let mut max = f64::MIN;
        for s in &self.series {
            for &v in &s.values {
                if v < min {
                    min = v;
                }
                if v > max {
                    max = v;
                }
            }
        }
        if min > max { (0.0, 1.0) } else { (min, max) }
    }
}
