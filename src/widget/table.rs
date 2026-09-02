//! Table widget — a data grid with columns, sorting, filtering, and pagination.

use crate::core::style::TextStyle;
use crate::core::{Color, Position, Rect, Style};
use crate::ontology::*;
use crate::runtime::Frame;
use crate::widget::StatefulWidget;

/// Sort direction for table columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

/// Table state with selection, scroll offset, sorting, filtering, and pagination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableState {
    pub selected_row: Option<usize>,
    pub offset: usize,
    /// Column index + direction currently being sorted by.
    pub sort_column: Option<(usize, SortDirection)>,
    /// Active filter text applied across all columns.
    pub filter: String,
    /// Number of rows per page (0 = no pagination).
    pub page_size: usize,
    /// Current page index (0-based).
    pub current_page: usize,
}

impl TableState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            selected_row: None,
            offset: 0,
            sort_column: None,
            filter: String::new(),
            page_size: 0,
            current_page: 0,
        }
    }

    /// Set the sort column and direction.
    pub fn sort_by(&mut self, column: usize, direction: SortDirection) {
        self.sort_column = Some((column, direction));
    }

    /// Toggle sort on a column (cycles: Ascending → Descending → None).
    pub fn toggle_sort(&mut self, column: usize) {
        self.sort_column = match self.sort_column {
            Some((col, SortDirection::Ascending)) if col == column => {
                Some((column, SortDirection::Descending))
            }
            Some((col, SortDirection::Descending)) if col == column => None,
            _ => Some((column, SortDirection::Ascending)),
        };
    }

    /// Set the filter string.
    pub fn set_filter(&mut self, filter: impl Into<String>) {
        self.filter = filter.into();
        self.current_page = 0;
    }

    /// Enable pagination with the given page size.
    pub fn set_page_size(&mut self, size: usize) {
        self.page_size = size;
        self.current_page = 0;
    }

    /// Compute total pages for a given row count.
    pub fn total_pages(&self, row_count: usize) -> usize {
        if self.page_size == 0 || row_count == 0 {
            1
        } else {
            row_count.div_ceil(self.page_size)
        }
    }

    /// Go to next page.
    pub fn next_page(&mut self, row_count: usize) {
        let total = self.total_pages(row_count);
        if self.current_page + 1 < total {
            self.current_page += 1;
        }
    }

    /// Go to previous page.
    pub fn prev_page(&mut self) {
        self.current_page = self.current_page.saturating_sub(1);
    }
}

impl Default for TableState {
    fn default() -> Self {
        Self::new()
    }
}

/// A data table with headers, rows, sorting, filtering, and pagination.
/// What an agent asked a [`Table`] to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableChange<'a> {
    /// Select the row at this index.
    SelectRow(usize),
    /// Sort by this column.
    Sort { column: usize, descending: bool },
    /// Keep only rows matching this text.
    Filter(&'a str),
    /// Show this page.
    Page(usize),
}

pub struct Table {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    style: Style,
    agent_id: std::borrow::Cow<'static, str>,
    handlers: Vec<(&'static str, Box<dyn std::any::Any + Send>)>,
}

impl Table {
    #[must_use]
    pub fn new(headers: Vec<String>, rows: Vec<Vec<String>>) -> Self {
        Self {
            headers,
            rows,
            style: Style::default(),
            agent_id: std::borrow::Cow::Borrowed(""),
            handlers: Vec::new(),
        }
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn bg(mut self, color: Color) -> Self {
        self.style.background = Some(color);
        self
    }

    pub fn fg(mut self, color: Color) -> Self {
        self.style.foreground = Some(color);
        self
    }

    /// Name this widget and give it the change to apply on `select_row`.
    ///
    /// Bound to the action `Table` advertises, so an agent following the
    /// ontology reaches it and the application writes no
    /// `execute_action` handler.
    #[must_use]
    pub fn on_select<M: 'static>(
        mut self,
        id: impl Into<std::borrow::Cow<'static, str>>,
        f: impl FnOnce(&mut M, usize) + Send + 'static,
    ) -> Self {
        let wrapped: crate::runtime::ValueMutation<M> =
            Box::new(move |m: &mut M, v: &serde_json::Value| {
                f(
                    m,
                    v.get("index")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0) as usize,
                )
            });
        self.agent_id = id.into();
        self.handlers.push(("select_row", Box::new(wrapped)));
        self
    }

    /// Name this table and give it the change to apply for every action it
    /// advertises: `select_row`, `sort`, `filter` and `page`.
    ///
    /// [`on_select`](Self::on_select) covers only the first. The others are
    /// left to the application because only it knows what its rows mean —
    /// but a table wired for selection alone accepts `sort` and silently
    /// ignores it, so `validate` reports the gap.
    #[must_use]
    pub fn on_change<M: 'static>(
        mut self,
        id: impl Into<std::borrow::Cow<'static, str>>,
        f: impl Fn(&mut M, TableChange<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.agent_id = id.into();
        let f = std::sync::Arc::new(f);
        for action in ["select_row", "sort", "filter", "page"] {
            let f = f.clone();
            let handler: crate::runtime::ValueMutation<M> =
                Box::new(move |m: &mut M, v: &serde_json::Value| {
                    let index = |k: &str| {
                        v.get(k).and_then(serde_json::Value::as_u64).unwrap_or(0) as usize
                    };
                    let change = match action {
                        "select_row" => TableChange::SelectRow(index("index")),
                        "sort" => TableChange::Sort {
                            column: index("column"),
                            descending: v
                                .get("direction")
                                .and_then(serde_json::Value::as_str)
                                .is_some_and(|d| d.eq_ignore_ascii_case("desc")),
                        },
                        "filter" => TableChange::Filter(
                            v.get("text")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or_default(),
                        ),
                        _ => TableChange::Page(index("page")),
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

    /// Apply filtering and sorting to produce the display-ready row indices.
    fn compute_visible_rows(&self, state: &TableState) -> Vec<usize> {
        let filter_lower = state.filter.to_lowercase();

        // Filter
        let mut indices: Vec<usize> = (0..self.rows.len())
            .filter(|&i| {
                if filter_lower.is_empty() {
                    return true;
                }
                self.rows[i]
                    .iter()
                    .any(|cell| cell.to_lowercase().contains(&filter_lower))
            })
            .collect();

        // Sort
        if let Some((col, direction)) = &state.sort_column {
            let col = *col;
            indices.sort_by(|&a, &b| {
                let va = self.rows[a].get(col).map(String::as_str).unwrap_or("");
                let vb = self.rows[b].get(col).map(String::as_str).unwrap_or("");
                // Try numeric sort, fall back to string
                let cmp = match (va.parse::<f64>(), vb.parse::<f64>()) {
                    (Ok(fa), Ok(fb)) => fa.partial_cmp(&fb).unwrap_or(std::cmp::Ordering::Equal),
                    _ => va.cmp(vb),
                };
                match direction {
                    SortDirection::Ascending => cmp,
                    SortDirection::Descending => cmp.reverse(),
                }
            });
        }

        indices
    }
}

impl Discoverable for Table {
    fn schema(&self) -> WidgetSchema {
        let mut schema = WidgetSchema::new(
            "Table",
            "A data table with rows and columns",
            SemanticRole::DataVisualization,
        );
        schema.usage_hint = Some("Table::new(headers, rows).page_size(20)".into());
        schema.tags = vec!["table".into(), "data".into(), "grid".into(), "rows".into()];
        schema
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![
            AgentCapability::Selectable {
                multi_select: false,
                item_count: self.rows.len(),
            },
            AgentCapability::Scrollable {
                vertical: true,
                horizontal: true,
            },
            AgentCapability::Sortable {
                columns: self.headers.clone(),
            },
            AgentCapability::Filterable,
        ]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![
            AgentAction::with_params(
                "select_row",
                "Select a row by index",
                vec![ActionParam::required(
                    "index",
                    "Row index",
                    ActionParamType::Index,
                )],
                true,
            ),
            AgentAction::with_params(
                "sort",
                "Sort by column",
                vec![
                    ActionParam::required("column", "Column index", ActionParamType::Index),
                    ActionParam::required("direction", "asc or desc", ActionParamType::String),
                ],
                true,
            ),
            AgentAction::with_params(
                "filter",
                "Filter rows by text",
                vec![ActionParam::required(
                    "text",
                    "Filter text",
                    ActionParamType::String,
                )],
                true,
            ),
            AgentAction::with_params(
                "page",
                "Go to page",
                vec![ActionParam::required(
                    "page",
                    "Page number (0-based)",
                    ActionParamType::Index,
                )],
                true,
            ),
        ]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::DataVisualization
    }

    fn agent_state(&self) -> serde_json::Value {
        serde_json::json!({
            "headers": self.headers,
            "row_count": self.rows.len(),
        })
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
        Some(format!(
            "Table ({} columns, {} rows)",
            self.headers.len(),
            self.rows.len()
        ))
    }
}

impl StatefulWidget for Table {
    type State = TableState;

    fn render(mut self, area: Rect, frame: &mut Frame<'_>, state: &mut TableState) {
        let visible = self.compute_visible_rows(state);
        let total_visible = visible.len();

        // Pagination
        let page_rows = if state.page_size > 0 {
            let start = state.current_page * state.page_size;
            let end = (start + state.page_size).min(total_visible);
            if start < total_visible {
                &visible[start..end]
            } else {
                &[]
            }
        } else {
            &visible[..]
        };

        if !self.agent_id.is_empty() {
            if frame.describes(area) {
                let node = UiNode::new("Table", SemanticRole::DataVisualization)
                    .with_id(self.agent_id.clone())
                    .with_bounds(area.into())
                    .with_property("headers", serde_json::json!(self.headers))
                    .with_property("row_count", serde_json::json!(self.rows.len()))
                    .with_property("visible_count", serde_json::json!(total_visible))
                    .with_property("selected_row", serde_json::json!(state.selected_row))
                    .with_property(
                        "sort_column",
                        serde_json::json!(state.sort_column.map(|(c, d)| {
                            serde_json::json!({"column": c, "direction": match d {
                                SortDirection::Ascending => "asc",
                                SortDirection::Descending => "desc",
                            }})
                        })),
                    )
                    .with_property("filter", serde_json::json!(state.filter))
                    .with_property("page", serde_json::json!(state.current_page))
                    .with_property(
                        "total_pages",
                        serde_json::json!(state.total_pages(total_visible)),
                    );
                frame.register_widget(node);
            }
            frame.register_hitbox(self.agent_id.clone(), area, 1);
            for (action, handler) in self.handlers.drain(..) {
                frame.register_message(self.agent_id.clone(), action, handler);
            }
        }

        frame.painter().push_clip(area);
        let col_count = self.headers.len().max(1);
        let col_w = area.width / col_count as f32;
        let row_h = 24.0;
        let mut header_ts = self.style.resolved_text();
        header_ts.weight = crate::core::style::FontWeight::Bold;
        let cell_ts = self.style.resolved_text();

        // Headers with sort indicator
        for (j, header) in self.headers.iter().enumerate() {
            let x = area.x + j as f32 * col_w;
            frame
                .painter()
                .fill_rect(Rect::new(x, area.y, col_w, row_h), Color::DARK_GRAY, 0.0);
            let indicator = match state.sort_column {
                Some((col, SortDirection::Ascending)) if col == j => " ▲",
                Some((col, SortDirection::Descending)) if col == j => " ▼",
                _ => "",
            };
            let label = format!("{header}{indicator}");
            frame
                .painter()
                .text(Position::new(x + 4.0, area.y + 4.0), &label, &header_ts);
        }

        // Rows
        for (display_i, &row_idx) in page_rows.iter().enumerate() {
            let y = area.y + (display_i + 1) as f32 * row_h;
            if state.selected_row == Some(row_idx) {
                frame.painter().fill_rect(
                    Rect::new(area.x, y, area.width, row_h),
                    Color::BLUE.with_alpha(0.3),
                    0.0,
                );
            } else if display_i % 2 == 1 {
                frame.painter().fill_rect(
                    Rect::new(area.x, y, area.width, row_h),
                    Color::WHITE.with_alpha(0.05),
                    0.0,
                );
            }
            if let Some(row) = self.rows.get(row_idx) {
                for (j, cell) in row.iter().enumerate() {
                    let x = area.x + j as f32 * col_w;
                    frame
                        .painter()
                        .text(Position::new(x + 4.0, y + 4.0), cell, &cell_ts);
                }
            }
        }

        // Pagination footer
        if state.page_size > 0 {
            let total_pages = state.total_pages(total_visible);
            let footer_y = area.y + (page_rows.len() + 1) as f32 * row_h;
            let footer_ts = TextStyle {
                font_size: 12.0,
                color: Color::LIGHT_GRAY,
                ..Default::default()
            };
            let footer_text = format!(
                "Page {} of {} ({} rows)",
                state.current_page + 1,
                total_pages,
                total_visible,
            );
            frame.painter().text(
                Position::new(area.x + 4.0, footer_y + 4.0),
                &footer_text,
                &footer_ts,
            );
        }

        frame.painter().pop_clip();
    }
}
