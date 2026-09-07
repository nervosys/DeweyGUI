//! Profiling instrumentation for Dewey.
//!
//! Tracks frame timing, widget render counts, and performance metrics
//! to help diagnose bottlenecks.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::ontology::*;

/// Per-frame timing data.
#[derive(Debug, Clone)]
pub struct FrameProfile {
    /// Frame number (monotonically increasing).
    pub frame_number: u64,
    /// Total frame time, wall clock from `begin_frame` to `end_frame`.
    pub total: Duration,
    /// Time spent in `Model::update`, for the messages this frame delivered.
    pub update: Duration,
    /// Time spent in layout computation.
    ///
    /// **Always zero.** Dewey has no separate layout pass: a widget lays
    /// itself out inside `Model::view`, so layout time is part of
    /// [`render`](Self::render) and no host can separate the two. The field is
    /// kept because it is public API at 1.0; it is not reported to agents,
    /// because publishing a constant zero as a measurement is how the rest of
    /// this crate's dead instrumentation looked from the outside.
    pub layout: Duration,
    /// Time spent in rendering (view + paint).
    pub render: Duration,
    /// Number of widgets rendered this frame.
    pub widget_count: usize,
}

/// A profiling timer — call `start(label)` / `stop(label)` around
/// code sections and then inspect results.
pub struct Profiler {
    frame_number: u64,
    timers: HashMap<String, Instant>,
    durations: HashMap<String, Duration>,
    widget_count: usize,
    frame_start: Instant,
    history: Vec<FrameProfile>,
    max_history: usize,
}

impl Profiler {
    /// Create a profiler that retains the last `max_history` frames.
    pub fn new(max_history: usize) -> Self {
        Self {
            frame_number: 0,
            timers: HashMap::new(),
            durations: HashMap::new(),
            widget_count: 0,
            frame_start: Instant::now(),
            history: Vec::new(),
            max_history,
        }
    }

    /// Begin a new frame. Call at the start of the frame loop.
    pub fn begin_frame(&mut self) {
        self.timers.clear();
        self.durations.clear();
        self.widget_count = 0;
        self.frame_start = Instant::now();
    }

    /// Start a named timer.
    pub fn start(&mut self, label: &str) {
        self.timers.insert(label.to_string(), Instant::now());
    }

    /// Record a duration measured somewhere else.
    ///
    /// The frame loop is not one function. A host times `Model::update` while
    /// answering an event and renders later, so the two halves of a frame
    /// cannot both be bracketed by `start`/`stop` around this profiler. The
    /// host measures its own half and hands the result over.
    pub fn record(&mut self, label: &str, duration: Duration) {
        self.durations.insert(label.to_string(), duration);
    }

    /// Stop a named timer and record the duration.
    pub fn stop(&mut self, label: &str) {
        if let Some(start) = self.timers.remove(label) {
            self.durations.insert(label.to_string(), start.elapsed());
        }
    }

    /// Increment the widget render count.
    pub fn count_widget(&mut self) {
        self.widget_count += 1;
    }

    /// Add N to the widget count.
    pub fn count_widgets(&mut self, n: usize) {
        self.widget_count += n;
    }

    /// End the frame and store the profile.
    pub fn end_frame(&mut self) {
        let total = self.frame_start.elapsed();
        let profile = FrameProfile {
            frame_number: self.frame_number,
            total,
            update: self.get_duration("update"),
            layout: self.get_duration("layout"),
            render: self.get_duration("render"),
            widget_count: self.widget_count,
        };

        self.history.push(profile);
        if self.history.len() > self.max_history {
            self.history.remove(0);
        }
        self.frame_number += 1;
    }

    fn get_duration(&self, label: &str) -> Duration {
        self.durations.get(label).copied().unwrap_or(Duration::ZERO)
    }

    /// Get the most recent frame profile.
    pub fn last_frame(&self) -> Option<&FrameProfile> {
        self.history.last()
    }

    /// Get the history of frame profiles.
    pub fn history(&self) -> &[FrameProfile] {
        &self.history
    }

    /// Average frame time over the stored history.
    pub fn avg_frame_time(&self) -> Duration {
        if self.history.is_empty() {
            return Duration::ZERO;
        }
        let total: Duration = self.history.iter().map(|f| f.total).sum();
        total / self.history.len() as u32
    }

    /// Average FPS over the stored history.
    pub fn avg_fps(&self) -> f64 {
        let avg = self.avg_frame_time();
        if avg.is_zero() {
            return 0.0;
        }
        1.0 / avg.as_secs_f64()
    }

    /// Get the maximum frame time in history.
    pub fn max_frame_time(&self) -> Duration {
        self.history
            .iter()
            .map(|f| f.total)
            .max()
            .unwrap_or(Duration::ZERO)
    }

    /// Get a named duration for a custom label.
    pub fn duration(&self, label: &str) -> Duration {
        self.get_duration(label)
    }

    /// Current frame number.
    pub fn frame_number(&self) -> u64 {
        self.frame_number
    }
}

impl Default for Profiler {
    fn default() -> Self {
        Self::new(120) // ~2 seconds at 60 FPS
    }
}

impl Discoverable for Profiler {
    fn schema(&self) -> WidgetSchema {
        let mut schema = WidgetSchema::new(
            "Profiler",
            "Frame profiling and performance metrics tracker",
            SemanticRole::Diagnostic,
        );
        schema.usage_hint = Some("profiler.begin_frame(); ... profiler.end_frame();".into());
        schema.tags = vec![
            "profiling".into(),
            "performance".into(),
            "fps".into(),
            "timing".into(),
            "metrics".into(),
        ];
        schema
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![
            AgentAction::simple("reset", "Clear profiling history", true),
            AgentAction::simple("snapshot", "Get current performance snapshot", false),
        ]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::Diagnostic
    }

    fn agent_state(&self) -> serde_json::Value {
        let last = self.last_frame().map(|f| {
            serde_json::json!({
                "frame_number": f.frame_number,
                "total_ms": f.total.as_secs_f64() * 1000.0,
                "update_ms": f.update.as_secs_f64() * 1000.0,
                "render_ms": f.render.as_secs_f64() * 1000.0,
                "widget_count": f.widget_count,
            })
        });
        serde_json::json!({
            "frame_number": self.frame_number,
            "avg_fps": self.avg_fps(),
            "avg_frame_time_ms": self.avg_frame_time().as_secs_f64() * 1000.0,
            "max_frame_time_ms": self.max_frame_time().as_secs_f64() * 1000.0,
            "history_length": self.history.len(),
            "max_history": self.max_history,
            "last_frame": last,
        })
    }

    fn execute_action(
        &mut self,
        action: &str,
        _params: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        match action {
            "reset" => {
                self.history.clear();
                self.frame_number = 0;
                Ok(serde_json::json!({ "reset": true }))
            }
            "snapshot" => Ok(self.agent_state()),
            _ => Err(format!("Unknown action: {action}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiler_basics() {
        let mut p = Profiler::new(10);
        p.begin_frame();
        p.start("update");
        // Simulate work
        std::thread::sleep(Duration::from_millis(1));
        p.stop("update");
        p.count_widgets(5);
        p.end_frame();

        let frame = p.last_frame().unwrap();
        assert_eq!(frame.frame_number, 0);
        assert_eq!(frame.widget_count, 5);
        assert!(frame.update > Duration::ZERO);
        assert!(p.avg_fps() > 0.0);
    }

    #[test]
    fn max_history_cap() {
        let mut p = Profiler::new(3);
        for _ in 0..5 {
            p.begin_frame();
            p.end_frame();
        }
        assert_eq!(p.history().len(), 3);
        assert_eq!(p.frame_number(), 5);
    }
}
