//! Command palette widget — a searchable command launcher.

use crate::core::{Color, Position, Rect, Style};
use crate::ontology::*;
use crate::runtime::Frame;
use crate::widget::StatefulWidget;

/// A command palette entry.
#[derive(Debug, Clone)]
pub struct PaletteCommand {
    /// Unique command identifier.
    pub id: String,
    /// Display label.
    pub label: String,
    /// Optional keyboard shortcut description.
    pub shortcut: Option<String>,
    /// Optional category for grouping.
    pub category: Option<String>,
}

impl PaletteCommand {
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            shortcut: None,
            category: None,
        }
    }

    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    pub fn category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into());
        self
    }
}

/// Persistent state for the command palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandPaletteState {
    /// Current search query.
    pub query: String,
    /// Whether the palette is open.
    pub open: bool,
    /// Index of the currently highlighted entry.
    pub selected_index: usize,
}

impl CommandPaletteState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            query: String::new(),
            open: false,
            selected_index: 0,
        }
    }
}

impl Default for CommandPaletteState {
    fn default() -> Self {
        Self::new()
    }
}

/// A searchable command launcher (Ctrl+Shift+P style).
///
/// # Examples
///
/// ```
/// # use dewey::prelude::*;
/// let commands = vec![
///     PaletteCommand::new("save", "Save File").shortcut("Ctrl+S"),
///     PaletteCommand::new("open", "Open File").shortcut("Ctrl+O"),
/// ];
/// CommandPalette::new(commands).bg(Color::DARK_GRAY).fg(Color::WHITE);
/// ```
///
/// Agents can list commands, search, and execute them.
/// What an agent asked a [`CommandPalette`] to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteChange<'a> {
    /// Run the command with this id.
    Execute(&'a str),
    /// Filter the command list by this query.
    Search(&'a str),
    /// Open the palette.
    Open,
    /// Close the palette.
    Close,
}

pub struct CommandPalette {
    commands: Vec<PaletteCommand>,
    placeholder: String,
    style: Style,
    agent_id: std::borrow::Cow<'static, str>,
    /// Changes to apply, one per action this widget advertises.
    handlers: Vec<(&'static str, Box<dyn std::any::Any + Send>)>,
}

impl CommandPalette {
    #[must_use]
    pub fn new(commands: Vec<PaletteCommand>) -> Self {
        Self {
            commands,
            placeholder: "Type a command...".into(),
            style: Style::default(),
            agent_id: std::borrow::Cow::Borrowed(""),
            handlers: Vec::new(),
        }
    }

    pub fn placeholder(mut self, ph: impl Into<String>) -> Self {
        self.placeholder = ph.into();
        self
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

    /// Name this palette and give it the change to apply for every action it
    /// advertises: `execute`, `search`, `open` and `close`.
    ///
    /// A command palette is the shape of interface an agent most wants to
    /// drive — one name per capability — and without a handler it accepts
    /// none of them, because its query and open state live in
    /// [`CommandPaletteState`].
    #[must_use]
    pub fn on_change<M: 'static>(
        mut self,
        id: impl Into<std::borrow::Cow<'static, str>>,
        f: impl Fn(&mut M, PaletteChange<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.agent_id = id.into();
        let f = std::sync::Arc::new(f);
        for action in ["execute", "search", "open", "close"] {
            let f = f.clone();
            let handler: crate::runtime::ValueMutation<M> =
                Box::new(move |m: &mut M, v: &serde_json::Value| {
                    let text = |k: &str| {
                        v.get(k)
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or_default()
                    };
                    let change = match action {
                        "execute" => PaletteChange::Execute(text("command_id")),
                        "search" => PaletteChange::Search(text("query")),
                        "open" => PaletteChange::Open,
                        _ => PaletteChange::Close,
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

    /// Filter commands by the current query using case-insensitive substring matching.
    fn filtered_commands(&self, query: &str) -> Vec<&PaletteCommand> {
        if query.is_empty() {
            return self.commands.iter().collect();
        }
        let q = query.to_lowercase();
        self.commands
            .iter()
            .filter(|c| {
                c.label.to_lowercase().contains(&q)
                    || c.id.to_lowercase().contains(&q)
                    || c.category
                        .as_deref()
                        .map(|cat| cat.to_lowercase().contains(&q))
                        .unwrap_or(false)
            })
            .collect()
    }
}

impl Discoverable for CommandPalette {
    fn schema(&self) -> WidgetSchema {
        let mut schema = WidgetSchema::new(
            "CommandPalette",
            "A searchable command launcher",
            SemanticRole::Navigation,
        );
        schema.usage_hint =
            Some("CommandPalette::new(commands).placeholder(\"Type a command...\")".into());
        schema.tags = vec![
            "command".into(),
            "palette".into(),
            "search".into(),
            "launcher".into(),
        ];
        schema
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![
            AgentCapability::Searchable,
            AgentCapability::Selectable {
                multi_select: false,
                item_count: 0,
            },
            AgentCapability::Focusable,
        ]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![
            AgentAction::with_params(
                "execute",
                "Execute a command by ID",
                vec![ActionParam::required(
                    "command_id",
                    "ID of the command to execute",
                    ActionParamType::String,
                )],
                false,
            ),
            AgentAction::with_params(
                "search",
                "Search commands by query",
                vec![ActionParam::required(
                    "query",
                    "Search query",
                    ActionParamType::String,
                )],
                false,
            ),
            AgentAction::simple("open", "Open the command palette", true),
            AgentAction::simple("close", "Close the command palette", true),
            AgentAction::simple("list", "List all available commands", false),
        ]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::Navigation
    }

    fn agent_state(&self) -> serde_json::Value {
        let cmds: Vec<_> = self
            .commands
            .iter()
            .map(|c| {
                serde_json::json!({
                    "id": c.id,
                    "label": c.label,
                    "shortcut": c.shortcut,
                    "category": c.category,
                })
            })
            .collect();
        serde_json::json!({ "commands": cmds })
    }

    fn execute_action(
        &mut self,
        action: &str,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        match action {
            "execute" => {
                let cmd_id = params
                    .get("command_id")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing command_id")?;
                match self.commands.iter().find(|c| c.id == cmd_id) {
                    Some(cmd) => Ok(serde_json::json!({
                        "executed": cmd.id,
                        "label": cmd.label,
                    })),
                    None => Err(format!("Unknown command: {cmd_id}")),
                }
            }
            "search" => {
                let query = params.get("query").and_then(|v| v.as_str()).unwrap_or("");
                let results: Vec<_> = self
                    .filtered_commands(query)
                    .iter()
                    .map(|c| {
                        serde_json::json!({
                            "id": c.id,
                            "label": c.label,
                        })
                    })
                    .collect();
                Ok(serde_json::json!({ "results": results }))
            }
            "list" => Ok(self.agent_state()),
            "open" | "close" => Ok(serde_json::json!({ "status": action })),
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
        if self.placeholder.is_empty() {
            None
        } else {
            Some(self.placeholder.clone())
        }
    }
}

impl StatefulWidget for CommandPalette {
    type State = CommandPaletteState;

    fn render(mut self, area: Rect, frame: &mut Frame<'_>, state: &mut CommandPaletteState) {
        if !self.agent_id.is_empty() {
            for (action, handler) in self.handlers.drain(..) {
                frame.register_message(self.agent_id.clone(), action, handler);
            }
        }

        if !state.open {
            return;
        }

        let filtered = self.filtered_commands(&state.query);
        if state.selected_index >= filtered.len() {
            state.selected_index = 0;
        }

        if !self.agent_id.is_empty() {
            if frame.describes(area) {
                let results: Vec<_> = filtered
                    .iter()
                    .map(|c| serde_json::json!({"id": c.id, "label": c.label}))
                    .collect();
                let node = UiNode::new("CommandPalette", SemanticRole::Navigation)
                    .with_id(self.agent_id.clone())
                    .with_bounds(area.into())
                    .with_property("query", serde_json::json!(state.query))
                    .with_property("open", serde_json::json!(state.open))
                    .with_property("selected_index", serde_json::json!(state.selected_index))
                    .with_property("filtered_results", serde_json::json!(results));
                frame.register_widget(node);
            }
            frame.register_hitbox(self.agent_id.clone(), area, 10);
        }

        // Palette window dimensions
        let pw = area.width * 0.7;
        let ph = area.height * 0.5;
        let px = area.x + area.width * 0.15;
        let py = area.y + 40.0;
        let palette_rect = Rect::new(px, py, pw, ph);

        let palette_bg = self.style.background.unwrap_or(Color::DARK_GRAY);
        frame.painter().fill_rect(palette_rect, palette_bg, 8.0);
        frame
            .painter()
            .stroke_rect(palette_rect, Color::GRAY, 1.0, 8.0);

        // Title
        let mut title_ts = self.style.resolved_text();
        if title_ts.font_size == 14.0 {
            title_ts.font_size = 16.0;
        }
        title_ts.weight = crate::core::style::FontWeight::Bold;
        frame.painter().text(
            Position::new(px + 8.0, py + 8.0),
            "Command Palette",
            &title_ts,
        );

        // Query
        let query_ts = self.style.resolved_text();
        let query_display = if state.query.is_empty() {
            "Type to search..."
        } else {
            &state.query
        };
        frame
            .painter()
            .text(Position::new(px + 8.0, py + 32.0), query_display, &query_ts);

        // Separator
        frame.painter().line(
            Position::new(px + 4.0, py + 52.0),
            Position::new(px + pw - 4.0, py + 52.0),
            Color::GRAY,
            1.0,
        );

        // Filtered items
        frame
            .painter()
            .push_clip(Rect::new(px, py + 56.0, pw, ph - 60.0));
        let item_ts = self.style.resolved_text();
        for (i, cmd) in filtered.iter().enumerate() {
            let iy = py + 56.0 + i as f32 * 24.0;
            if i == state.selected_index {
                frame.painter().fill_rect(
                    Rect::new(px + 2.0, iy, pw - 4.0, 22.0),
                    Color::from_rgba8(60, 60, 120, 255),
                    4.0,
                );
            }
            let text = if let Some(ref sc) = cmd.shortcut {
                format!("{} ({})", cmd.label, sc)
            } else {
                cmd.label.clone()
            };
            frame
                .painter()
                .text(Position::new(px + 12.0, iy + 3.0), &text, &item_ts);
        }
        frame.painter().pop_clip();
    }
}
