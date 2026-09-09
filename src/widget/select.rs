//! Select / combo box widget.

use crate::core::style::TextStyle;
use crate::core::{Color, Position, Rect, Style};
use crate::ontology::*;
use crate::runtime::Frame;
use crate::widget::StatefulWidget;

/// Select state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SelectState {
    pub selected: usize,
}

impl SelectState {
    #[must_use]
    pub fn new() -> Self {
        Self { selected: 0 }
    }
}

/// A dropdown select / combo box.
///
/// # Agent view
///
/// What an agent sees of this widget, and what it can call on it.
/// Every action changes state unless it says otherwise.
/// Generated from the widget's own answers; `tests/agent_view.rs`
/// fails the build if this block and the code disagree.
///
/// - role: `Selection`
/// - actions: `select`(index), `toggle_open`
/// - state it publishes: `label`, `open`, `options`
/// - capabilities: `Focusable`, `Selectable`
/// - live value: held by the application in `SelectState`, not by the widget
pub struct Select {
    options: Vec<String>,
    label: String,
    style: Style,
    agent_id: std::borrow::Cow<'static, str>,
    on_value: Option<Box<dyn std::any::Any + Send>>,
    open: bool,
    on_open: Option<Box<dyn std::any::Any + Send>>,
}

/// The height of one row of the open option list.
const OPTION_HEIGHT: f32 = 24.0;

impl Select {
    #[must_use]
    pub fn new(label: impl Into<String>, options: Vec<String>) -> Self {
        Self {
            options,
            label: label.into(),
            style: Style::default(),
            agent_id: std::borrow::Cow::Borrowed(""),
            on_value: None,
            open: false,
            on_open: None,
        }
    }

    /// Whether the option list is showing.
    ///
    /// The application owns this, the way it owns a `Modal`'s. `SelectState`
    /// holds only the selection, and giving it a second field would break
    /// every `SelectState { selected }` already written.
    #[must_use]
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Give this select the change to apply when the list is opened or closed.
    ///
    /// The bool is the state it should move to. Without this a `Select` cannot
    /// be opened by a person at all: it drew a value and an arrow, the list
    /// was never painted, and clicking the arrow had nothing to fire.
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

    pub fn rounded(mut self, radius: f32) -> Self {
        self.style.border_radius = Some(radius);
        self
    }

    /// Name this widget and give it the change to apply on `select`.
    ///
    /// Bound to the action `Select` advertises, so an agent following the
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
        self.on_value = Some(Box::new(wrapped));
        self
    }

    pub fn agent_id(mut self, id: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        self.agent_id = id.into();
        self
    }
}

impl Discoverable for Select {
    fn schema(&self) -> WidgetSchema {
        let mut schema = WidgetSchema::new("Select", "A dropdown select", SemanticRole::Selection);
        schema.usage_hint =
            Some("Select::new(\"Color\", vec![\"Red\".into(), \"Blue\".into()])".into());
        schema.tags = vec!["select".into(), "dropdown".into(), "combo".into()];
        schema
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![
            AgentCapability::Selectable {
                multi_select: false,
                item_count: self.options.len(),
            },
            AgentCapability::Focusable,
        ]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![
            AgentAction::with_params(
                "select",
                "Select an option by index",
                vec![ActionParam::required(
                    "index",
                    "Option index",
                    ActionParamType::Index,
                )],
                true,
            ),
            AgentAction::simple("toggle_open", "Show or hide the option list", true),
        ]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::Selection
    }

    fn agent_state(&self) -> serde_json::Value {
        serde_json::json!({
            "options": self.options,
            "label": self.label,
            "open": self.open,
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

impl StatefulWidget for Select {
    type State = SelectState;

    fn render(mut self, area: Rect, frame: &mut Frame<'_>, state: &mut SelectState) {
        let list = Rect::new(
            area.x,
            area.y + area.height,
            area.width,
            self.options.len() as f32 * OPTION_HEIGHT,
        );

        if !self.agent_id.is_empty() {
            frame.register_hitbox(self.agent_id.clone(), area, 1);
            if let Some(handler) = self.on_value.take() {
                frame.register_message(self.agent_id.clone(), "select", handler);
            }
            if let Some(handler) = self.on_open.take() {
                frame.register_message(self.agent_id.clone(), "toggle_open", handler);
            }

            // A click on the field opens or closes the list; a click on an
            // option picks it. Both name their action, because the primary one
            // is whichever handler was registered first and neither of these
            // is "whichever".
            //
            // Until the list was drawn there was nothing to aim at, so a click
            // could say nothing about which option was meant, and said `0`.
            let open = self.open;
            frame.register_click(
                self.agent_id.clone(),
                crate::runtime::ClickParams::from_position(move |at| {
                    if area.contains(at) {
                        return Some(crate::runtime::Click::action(
                            "toggle_open",
                            serde_json::Value::Null,
                        ));
                    }
                    if !open || !list.contains(at) {
                        return None;
                    }
                    let row = ((at.y - list.y) / OPTION_HEIGHT).floor();
                    if row < 0.0 {
                        return None;
                    }
                    Some(crate::runtime::Click::action(
                        "select",
                        serde_json::json!({ "index": row as usize }),
                    ))
                }),
            );
        }

        // Draw select box
        let bg = self.style.background.unwrap_or(Color::DARK_GRAY);
        let radius = self.style.border_radius.unwrap_or(4.0);
        frame.painter().fill_rect(area, bg, radius);
        frame.painter().stroke_rect(area, Color::GRAY, 1.0, radius);
        let current = self
            .options
            .get(state.selected)
            .cloned()
            .unwrap_or_default();
        let ts = self.style.resolved_text();
        // Label
        if !self.label.is_empty() {
            let label_ts = TextStyle {
                font_size: 12.0,
                color: Color::GRAY,
                ..Default::default()
            };
            frame
                .painter()
                .text(Position::new(area.x, area.y - 16.0), &self.label, &label_ts);
        }
        frame
            .painter()
            .text(Position::new(area.x + 4.0, area.y + 4.0), &current, &ts);
        // Dropdown arrow
        let arrow_x = area.x + area.width - 16.0;
        let arrow_y = area.y + area.height * 0.5;
        frame
            .painter()
            .text(Position::new(arrow_x, arrow_y - 7.0), "\u{25BC}", &ts);

        // The open list is drawn after every other widget, not here: painted
        // inline it would be covered by whatever the view renders next, which
        // for a form is the field underneath. The hitbox it registers is later
        // than theirs too, and a later hitbox wins a tie, so an option takes
        // the click rather than the control it is covering.
        if self.open && !self.options.is_empty() {
            let options = std::mem::take(&mut self.options);
            let selected = state.selected;
            let id = self.agent_id.clone();
            let bg = self.style.background.unwrap_or(Color::DARK_GRAY);
            frame.overlay(move |frame| {
                frame.painter().fill_rect(list, bg, 4.0);
                frame.painter().stroke_rect(list, Color::GRAY, 1.0, 4.0);
                let ts = TextStyle::default();
                for (i, option) in options.iter().enumerate() {
                    let row = Rect::new(
                        list.x,
                        list.y + i as f32 * OPTION_HEIGHT,
                        list.width,
                        OPTION_HEIGHT,
                    );
                    if i == selected {
                        frame
                            .painter()
                            .fill_rect(row, Color::BLUE.with_alpha(0.3), 0.0);
                    }
                    frame
                        .painter()
                        .text(Position::new(row.x + 4.0, row.y + 4.0), option, &ts);
                }
                if !id.is_empty() {
                    frame.register_hitbox(id, list, 1);
                }
            });
        }

        // Built last so owned fields move into the state instead of being
        // cloned; painting above only borrows them.
        if frame.describes(area) && !self.agent_id.is_empty() {
            let selected_text = self
                .options
                .get(state.selected)
                .cloned()
                .unwrap_or_default();
            let node = UiNode::new("Select", SemanticRole::Selection)
                .with_id(self.agent_id.clone())
                .with_bounds(area.into())
                .with_property("options", serde_json::Value::from(self.options))
                .with_property("selected", serde_json::json!(state.selected))
                .with_property("selected_text", serde_json::json!(selected_text))
                .with_property("open", serde_json::json!(self.open));
            frame.register_widget(node);
        }
    }
}
