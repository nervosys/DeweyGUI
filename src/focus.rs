//! Keyboard focus: Tab between widgets, Enter to press one.
//!
//! Every host drives this — the default backend, agpu and the headless driver
//! — through [`handle_key`] and [`draw_ring`], so Tab means the same thing
//! wherever an application runs.
//!
//! The ring is rebuilt from the hit map after every frame, so **render order
//! is tab order** and a widget is focusable exactly when it is interactive and
//! has an id. There is no second registration to fall out of step with, and a
//! `Label` is not a stop.
//!
//! [`draw_ring`] paints the indicator centrally rather than each widget
//! rendering its own focused state: 29 widgets would otherwise be 29 chances
//! to forget, and a widget that forgot would be invisibly unreachable.
//!
//! This shipped as "Focus Management — Ring-buffer tab navigation" with
//! [`FocusManager`] complete and nothing calling it, so pressing Tab did
//! nothing at all. An agent never noticed, because it addresses a widget by
//! id.

/// Manages focus state across focusable widgets.
pub struct FocusManager {
    /// Ordered list of focusable widget agent IDs.
    focusable: Vec<String>,
    /// Currently focused index (None if nothing focused).
    focused: Option<usize>,
}

impl FocusManager {
    /// Create an empty focus manager.
    pub fn new() -> Self {
        Self {
            focusable: Vec::new(),
            focused: None,
        }
    }

    /// Register a focusable widget by its agent ID.
    pub fn register(&mut self, agent_id: impl Into<String>) {
        let id = agent_id.into();
        if !self.focusable.contains(&id) {
            self.focusable.push(id);
        }
    }

    /// Clear all registrations (called at the start of each frame).
    pub fn clear(&mut self) {
        self.focusable.clear();
        self.focused = None;
    }

    /// Rebuild focus targets but keep the currently focused widget if it's still present.
    pub fn rebuild(&mut self, ids: Vec<String>) {
        let current = self.focused_id().map(|s| s.to_string());
        self.focusable = ids;
        self.focused = current.and_then(|id| self.focusable.iter().position(|f| f == &id));
    }

    /// Move focus to the next widget (Tab).
    pub fn focus_next(&mut self) {
        if self.focusable.is_empty() {
            self.focused = None;
            return;
        }
        self.focused = Some(match self.focused {
            Some(i) => (i + 1) % self.focusable.len(),
            None => 0,
        });
    }

    /// Move focus to the previous widget (Shift+Tab).
    pub fn focus_prev(&mut self) {
        if self.focusable.is_empty() {
            self.focused = None;
            return;
        }
        self.focused = Some(match self.focused {
            Some(0) => self.focusable.len() - 1,
            Some(i) => i - 1,
            None => self.focusable.len() - 1,
        });
    }

    /// Set focus to a specific widget by agent ID.
    pub fn focus_on(&mut self, agent_id: &str) {
        self.focused = self.focusable.iter().position(|f| f == agent_id);
    }

    /// Remove focus.
    pub fn blur(&mut self) {
        self.focused = None;
    }

    /// The currently focused widget's agent ID, if any.
    pub fn focused_id(&self) -> Option<&str> {
        self.focused
            .and_then(|i| self.focusable.get(i))
            .map(|s| s.as_str())
    }

    /// Whether a specific widget is focused.
    pub fn is_focused(&self, agent_id: &str) -> bool {
        self.focused_id() == Some(agent_id)
    }

    /// Total number of registered focusable widgets.
    pub fn len(&self) -> usize {
        self.focusable.len()
    }

    /// Whether there are no focusable widgets.
    pub fn is_empty(&self) -> bool {
        self.focusable.is_empty()
    }
}

impl Default for FocusManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_ring_navigation() {
        let mut fm = FocusManager::new();
        fm.register("a");
        fm.register("b");
        fm.register("c");

        assert_eq!(fm.focused_id(), None);

        fm.focus_next();
        assert_eq!(fm.focused_id(), Some("a"));

        fm.focus_next();
        assert_eq!(fm.focused_id(), Some("b"));

        fm.focus_next();
        assert_eq!(fm.focused_id(), Some("c"));

        // Wraps around
        fm.focus_next();
        assert_eq!(fm.focused_id(), Some("a"));

        // Go back
        fm.focus_prev();
        assert_eq!(fm.focused_id(), Some("c"));
    }

    #[test]
    fn focus_by_id() {
        let mut fm = FocusManager::new();
        fm.register("x");
        fm.register("y");
        fm.focus_on("y");
        assert!(fm.is_focused("y"));
        assert!(!fm.is_focused("x"));
    }
}

/// What a key press did to the focus ring.
#[derive(Debug, PartialEq, Eq)]
pub enum FocusAction {
    /// Focus moved. Redraw; nothing else to do.
    Moved,
    /// Activate this widget, exactly as a click on it would.
    Activate(String),
    /// Not a focus key.
    Ignored,
}

/// The colour of the focus indicator.
///
/// A ring the runtime draws, rather than a state each widget renders itself:
/// every widget would otherwise need to know about focus, and the ones that
/// forgot would be invisibly unreachable — which is the failure this whole
/// module was in before it was driven at all.
pub const RING_COLOUR: crate::core::Color = crate::core::Color::rgba(0.35, 0.6, 1.0, 1.0);

/// How thick the focus indicator is drawn.
pub const RING_WIDTH: f32 = 2.0;

/// Apply a key press to the ring.
///
/// Every host calls this, so Tab means the same thing headless, under the
/// default backend and under agpu. `Space` is `Char(' ')`, which is what a
/// text field also receives — a widget that consumes typing is why activation
/// is offered on `Enter` as well.
///
/// Shift+Tab goes backwards whether it arrives as `BackTab` or as `Tab` with
/// the shift modifier. Both happen: winit and egui report the first, and the
/// protocol's `inject_event` produces the second, because an agent writes
/// `{"code": "tab", "modifiers": ["shift"]}`.
pub fn handle_key(key: &crate::event::KeyEvent, focus: &mut FocusManager) -> FocusAction {
    use crate::event::{KeyCode, KeyModifiers};
    let shifted = key.modifiers.contains(KeyModifiers::SHIFT);
    match key.code {
        KeyCode::Tab if shifted => {
            focus.focus_prev();
            FocusAction::Moved
        }
        KeyCode::Tab => {
            focus.focus_next();
            FocusAction::Moved
        }
        KeyCode::BackTab => {
            focus.focus_prev();
            FocusAction::Moved
        }
        KeyCode::Enter | KeyCode::Char(' ') => match focus.focused_id() {
            Some(id) => FocusAction::Activate(id.to_string()),
            None => FocusAction::Ignored,
        },
        _ => FocusAction::Ignored,
    }
}

/// Draw the focus indicator around the focused widget.
///
/// Call after `Model::view`, while the frame still holds the hit map the view
/// filled in. Draws nothing when nothing is focused.
pub fn draw_ring(frame: &mut crate::runtime::Frame<'_>, focus: &FocusManager) {
    let Some(id) = focus.focused_id() else { return };
    let Some(bounds) = frame.hit_map.bounds_of(id) else {
        return;
    };
    frame
        .painter()
        .stroke_rect(bounds, RING_COLOUR, RING_WIDTH, 2.0);
}
