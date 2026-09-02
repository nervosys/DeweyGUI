//! Date/time picker widget — a calendar-style date selector.

use crate::core::style::{Color, FontWeight, Style, TextStyle};
use crate::core::{Position, Rect};
use crate::ontology::*;
use crate::runtime::Frame;
use crate::widget::StatefulWidget;

/// A date value (year, month 1-12, day 1-31).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateValue {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl DateValue {
    #[must_use]
    pub fn new(year: i32, month: u32, day: u32) -> Self {
        Self { year, month, day }
    }

    /// Number of days in this date's month.
    pub fn days_in_month(&self) -> u32 {
        match self.month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => {
                if self.is_leap_year() {
                    29
                } else {
                    28
                }
            }
            _ => 30,
        }
    }

    fn is_leap_year(&self) -> bool {
        (self.year % 4 == 0 && self.year % 100 != 0) || self.year % 400 == 0
    }

    /// Day of week for the 1st of this month (0 = Monday, 6 = Sunday).
    /// Uses Tomohiko Sakamoto's algorithm.
    pub fn first_weekday(&self) -> u32 {
        let y = if self.month <= 2 {
            self.year - 1
        } else {
            self.year
        };
        let m = self.month as i32;
        let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
        let idx = (m - 1) as usize;
        let dow = (y + y / 4 - y / 100 + y / 400 + t[idx] + 1) % 7;
        // Convert: 0=Sun → Mon-based: 0=Mon
        ((dow + 6) % 7) as u32
    }

    /// Month name.
    pub fn month_name(&self) -> &'static str {
        match self.month {
            1 => "January",
            2 => "February",
            3 => "March",
            4 => "April",
            5 => "May",
            6 => "June",
            7 => "July",
            8 => "August",
            9 => "September",
            10 => "October",
            11 => "November",
            12 => "December",
            _ => "Unknown",
        }
    }

    /// Format as YYYY-MM-DD.
    pub fn to_iso(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

impl std::fmt::Display for DateValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_iso())
    }
}

/// State for the date picker widget.
pub struct DatePickerState {
    /// The currently selected date.
    pub selected: DateValue,
    /// The month being viewed (may differ from selected).
    pub view_year: i32,
    pub view_month: u32,
    /// Whether the calendar popup is open.
    pub open: bool,
}

impl DatePickerState {
    #[must_use]
    pub fn new(year: i32, month: u32, day: u32) -> Self {
        Self {
            selected: DateValue::new(year, month, day),
            view_year: year,
            view_month: month,
            open: false,
        }
    }

    /// Navigate to the previous month.
    pub fn prev_month(&mut self) {
        if self.view_month == 1 {
            self.view_month = 12;
            self.view_year -= 1;
        } else {
            self.view_month -= 1;
        }
    }

    /// Navigate to the next month.
    pub fn next_month(&mut self) {
        if self.view_month == 12 {
            self.view_month = 1;
            self.view_year += 1;
        } else {
            self.view_month += 1;
        }
    }

    /// Select a specific day in the current view month.
    pub fn select_day(&mut self, day: u32) {
        self.selected = DateValue::new(self.view_year, self.view_month, day);
    }

    /// Toggle the open state.
    pub fn toggle(&mut self) {
        self.open = !self.open;
    }
}

impl Default for DatePickerState {
    fn default() -> Self {
        Self::new(2025, 1, 1)
    }
}

/// A date picker widget showing a calendar grid.
/// What an agent asked a [`DatePicker`] to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateChange {
    /// Select this year/month/day.
    Set { year: i32, month: u32, day: u32 },
    /// Show the previous month.
    PrevMonth,
    /// Show the next month.
    NextMonth,
    /// Open or close the calendar.
    Toggle,
}

pub struct DatePicker {
    style: Style,
    agent_id: std::borrow::Cow<'static, str>,
    label: String,
    /// Changes to apply, one per action this widget advertises.
    handlers: Vec<(&'static str, Box<dyn std::any::Any + Send>)>,
}

impl DatePicker {
    pub fn new() -> Self {
        Self {
            style: Style::default(),
            agent_id: std::borrow::Cow::Borrowed(""),
            label: "Select date".to_string(),
            handlers: Vec::new(),
        }
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

    /// Name this picker and give it the change to apply for every action it
    /// advertises: `set_date`, `prev_month`, `next_month` and `toggle`.
    ///
    /// The selected date lives in [`DatePickerState`], outside the widget, so
    /// without a handler the picker rejects every action an agent sends it.
    #[must_use]
    pub fn on_change<M: 'static>(
        mut self,
        id: impl Into<std::borrow::Cow<'static, str>>,
        f: impl Fn(&mut M, DateChange) + Send + Sync + 'static,
    ) -> Self {
        self.agent_id = id.into();
        let f = std::sync::Arc::new(f);
        for action in ["set_date", "prev_month", "next_month", "toggle"] {
            let f = f.clone();
            let handler: crate::runtime::ValueMutation<M> =
                Box::new(move |m: &mut M, v: &serde_json::Value| {
                    let int = |k: &str| v.get(k).and_then(serde_json::Value::as_i64);
                    let change = match action {
                        "set_date" => DateChange::Set {
                            year: int("year").unwrap_or(1970) as i32,
                            month: int("month").unwrap_or(1) as u32,
                            day: int("day").unwrap_or(1) as u32,
                        },
                        "prev_month" => DateChange::PrevMonth,
                        "next_month" => DateChange::NextMonth,
                        _ => DateChange::Toggle,
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

impl Default for DatePicker {
    fn default() -> Self {
        Self::new()
    }
}

impl Discoverable for DatePicker {
    fn schema(&self) -> WidgetSchema {
        let mut schema = WidgetSchema::new(
            "DatePicker",
            "A calendar-style date picker",
            SemanticRole::Input,
        );
        schema.usage_hint = Some("DatePicker::new(\"Birthday\").agent_id(\"bday\")".into());
        schema.tags = vec!["date".into(), "picker".into(), "calendar".into()];
        schema
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![AgentCapability::Focusable, AgentCapability::Clickable]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![
            AgentAction::with_params(
                "set_date",
                "Set the selected date",
                vec![
                    ActionParam::required("year", "Year", ActionParamType::Integer),
                    ActionParam::required("month", "Month (1-12)", ActionParamType::Integer),
                    ActionParam::required("day", "Day (1-31)", ActionParamType::Integer),
                ],
                true,
            ),
            AgentAction::simple("prev_month", "Navigate to previous month", true),
            AgentAction::simple("next_month", "Navigate to next month", true),
            AgentAction::simple("toggle", "Toggle calendar open/closed", true),
        ]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::Input
    }

    fn agent_state(&self) -> serde_json::Value {
        serde_json::json!({
            "label": self.label,
            "note": "Use DatePickerState for selected date; see UiTree for live state",
        })
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

impl Discoverable for DatePickerState {
    fn schema(&self) -> WidgetSchema {
        WidgetSchema::new(
            "DatePickerState",
            "The mutable state of a DatePicker, holding selected date and calendar view",
            SemanticRole::Input,
        )
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![AgentCapability::Focusable, AgentCapability::Clickable]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![
            AgentAction::with_params(
                "set_date",
                "Set the selected date",
                vec![
                    ActionParam::required("year", "Year", ActionParamType::Integer),
                    ActionParam::required("month", "Month (1-12)", ActionParamType::Integer),
                    ActionParam::required("day", "Day (1-31)", ActionParamType::Integer),
                ],
                true,
            ),
            AgentAction::simple("prev_month", "Navigate to previous month", true),
            AgentAction::simple("next_month", "Navigate to next month", true),
            AgentAction::simple("toggle", "Toggle calendar open/closed", true),
        ]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::Input
    }

    fn agent_state(&self) -> serde_json::Value {
        serde_json::json!({
            "selected": self.selected.to_iso(),
            "view_year": self.view_year,
            "view_month": self.view_month,
            "open": self.open,
        })
    }

    fn execute_action(
        &mut self,
        action: &str,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        match action {
            "set_date" => {
                let year = params["year"].as_i64().ok_or("missing year")? as i32;
                let month = params["month"].as_u64().ok_or("missing month")? as u32;
                let day = params["day"].as_u64().ok_or("missing day")? as u32;
                if !(1..=12).contains(&month) {
                    return Err("month must be 1-12".to_string());
                }
                if !(1..=31).contains(&day) {
                    return Err("day must be 1-31".to_string());
                }
                self.selected = DateValue::new(year, month, day);
                Ok(serde_json::json!({ "selected": self.selected.to_iso() }))
            }
            "prev_month" => {
                self.prev_month();
                Ok(serde_json::json!({
                    "view_year": self.view_year,
                    "view_month": self.view_month,
                }))
            }
            "next_month" => {
                self.next_month();
                Ok(serde_json::json!({
                    "view_year": self.view_year,
                    "view_month": self.view_month,
                }))
            }
            "toggle" => {
                self.toggle();
                Ok(serde_json::json!({ "open": self.open }))
            }
            _ => Err(format!("Unknown action: {action}")),
        }
    }
}

impl StatefulWidget for DatePicker {
    type State = DatePickerState;

    fn render(mut self, area: Rect, frame: &mut Frame<'_>, state: &mut DatePickerState) {
        if !self.agent_id.is_empty() {
            for (action, handler) in self.handlers.drain(..) {
                frame.register_message(self.agent_id.clone(), action, handler);
            }
        }

        let ts = self.style.resolved_text();

        if !self.agent_id.is_empty() {
            if frame.describes(area) {
                let node = UiNode::new("DatePicker", SemanticRole::Input)
                    .with_id(self.agent_id.clone())
                    .with_bounds(area.into())
                    .with_property("selected", serde_json::json!(state.selected.to_iso()))
                    .with_property("open", serde_json::json!(state.open))
                    .with_property(
                        "view_month",
                        serde_json::json!(format!(
                            "{} {}",
                            DateValue::new(state.view_year, state.view_month, 1).month_name(),
                            state.view_year
                        )),
                    );
                frame.register_widget(node);
            }
            frame.register_hitbox(self.agent_id.clone(), area, 1);
        }

        // Date display row
        let display_text = format!("{}: {}", self.label, state.selected.to_iso());
        frame.painter().fill_rect(
            Rect::new(area.x, area.y, area.width, 28.0),
            Color::DARK_GRAY,
            4.0,
        );
        frame.painter().text(
            Position::new(area.x + 8.0, area.y + 6.0),
            &display_text,
            &ts,
        );

        if !state.open {
            return;
        }

        // Calendar grid
        let cal_y = area.y + 32.0;
        let cell_w = area.width / 7.0;
        let cell_h = 24.0;

        // Month/year header
        let view_date = DateValue::new(state.view_year, state.view_month, 1);
        let header = format!("◀  {} {}  ▶", view_date.month_name(), state.view_year);
        let header_ts = TextStyle {
            font_size: 14.0,
            color: Color::WHITE,
            weight: FontWeight::Bold,
            ..Default::default()
        };
        frame.painter().fill_rect(
            Rect::new(area.x, cal_y, area.width, cell_h),
            Color::rgba(0.15, 0.15, 0.2, 1.0),
            0.0,
        );
        frame.painter().text(
            Position::new(area.x + 8.0, cal_y + 4.0),
            &header,
            &header_ts,
        );

        // Day-of-week headers
        let dow_y = cal_y + cell_h;
        let dow_ts = TextStyle {
            font_size: 12.0,
            color: Color::LIGHT_GRAY,
            weight: FontWeight::Bold,
            ..Default::default()
        };
        for (i, name) in ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"]
            .iter()
            .enumerate()
        {
            let x = area.x + i as f32 * cell_w;
            frame.painter().text(
                Position::new(x + cell_w / 2.0 - 6.0, dow_y + 4.0),
                name,
                &dow_ts,
            );
        }

        // Day grid
        let first_dow = view_date.first_weekday();
        let days_in_month = view_date.days_in_month();
        let grid_y = dow_y + cell_h;

        let day_ts = TextStyle {
            font_size: 13.0,
            color: Color::WHITE,
            ..Default::default()
        };
        let selected_ts = TextStyle {
            font_size: 13.0,
            color: Color::WHITE,
            weight: FontWeight::Bold,
            ..Default::default()
        };

        for day in 1..=days_in_month {
            let cell_idx = (first_dow + day - 1) as f32;
            let col = cell_idx % 7.0;
            let row = (cell_idx / 7.0).floor();
            let cx = area.x + col * cell_w;
            let cy = grid_y + row * cell_h;

            let is_selected = state.selected.year == state.view_year
                && state.selected.month == state.view_month
                && state.selected.day == day;

            if is_selected {
                frame.painter().fill_circle(
                    Position::new(cx + cell_w / 2.0, cy + cell_h / 2.0),
                    10.0,
                    Color::rgba(0.35, 0.55, 0.95, 1.0),
                );
                frame.painter().text(
                    Position::new(cx + cell_w / 2.0 - 6.0, cy + 4.0),
                    &day.to_string(),
                    &selected_ts,
                );
            } else {
                frame.painter().text(
                    Position::new(cx + cell_w / 2.0 - 6.0, cy + 4.0),
                    &day.to_string(),
                    &day_ts,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_value_days_in_month() {
        assert_eq!(DateValue::new(2024, 2, 1).days_in_month(), 29); // leap
        assert_eq!(DateValue::new(2023, 2, 1).days_in_month(), 28);
        assert_eq!(DateValue::new(2024, 1, 1).days_in_month(), 31);
        assert_eq!(DateValue::new(2024, 4, 1).days_in_month(), 30);
    }

    #[test]
    fn date_value_iso() {
        assert_eq!(DateValue::new(2025, 6, 15).to_iso(), "2025-06-15");
    }

    #[test]
    fn state_navigation() {
        let mut state = DatePickerState::new(2025, 1, 15);
        state.prev_month();
        assert_eq!(state.view_month, 12);
        assert_eq!(state.view_year, 2024);
        state.next_month();
        assert_eq!(state.view_month, 1);
        assert_eq!(state.view_year, 2025);
    }
}
