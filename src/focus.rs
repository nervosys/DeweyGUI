//! A focus ring, for an application to drive.
//!
//! [`FocusManager`] keeps an ordered list of widget ids and moves a cursor
//! around it, which is the bookkeeping Tab/Shift+Tab navigation needs.
//!
//! **Nothing in this crate drives it.** No backend registers a widget here, no
//! key handler routes Tab to [`focus_next`](FocusManager::focus_next), and no
//! widget draws a focus indicator or activates on Enter. Pressing Tab in a
//! Dewey application therefore does nothing unless the application does all of
//! that itself: build the ring from the ids it rendered, move it from
//! `handle_event`, and style the focused widget.
//!
//! An agent is unaffected — it addresses a widget by id rather than by
//! tabbing to it — but a keyboard user is not, and a screen reader reads the
//! AccessKit tree without a focus ring behind it.

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
