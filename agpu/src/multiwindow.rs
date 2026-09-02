//! Window bookkeeping.
//!
//! **This tracks windows; it does not open them.** There is no `winit`
//! reference in this module: `create` allocates an id and pushes a record,
//! and `close` removes one. An embedder that wants a second real window
//! creates it itself and uses this to keep track.
//!
//! Nothing in this crate drives it.

use crate::core::{Rect, Size};

/// Unique identifier for a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(u64);

impl WindowId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn raw(&self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for WindowId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Window({})", self.0)
    }
}

/// Configuration for creating a new window.
#[derive(Debug, Clone)]
pub struct WindowConfig {
    pub title: String,
    pub size: Size,
    pub resizable: bool,
    pub decorations: bool,
    pub transparent: bool,
    pub always_on_top: bool,
}

impl WindowConfig {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            size: Size::new(800.0, 600.0),
            resizable: true,
            decorations: true,
            transparent: false,
            always_on_top: false,
        }
    }

    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.size = Size::new(width, height);
        self
    }

    pub fn resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    pub fn decorations(mut self, decorations: bool) -> Self {
        self.decorations = decorations;
        self
    }

    pub fn transparent(mut self, transparent: bool) -> Self {
        self.transparent = transparent;
        self
    }

    pub fn always_on_top(mut self, on_top: bool) -> Self {
        self.always_on_top = on_top;
        self
    }
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self::new("Untitled")
    }
}

/// State of a managed window.
#[derive(Debug, Clone)]
pub struct WindowState {
    pub id: WindowId,
    pub config: WindowConfig,
    pub area: Rect,
    pub focused: bool,
    pub visible: bool,
}

/// Manages multiple windows and their lifecycle.
pub struct WindowManager {
    windows: Vec<WindowState>,
    next_id: u64,
    focused: Option<WindowId>,
}

impl WindowManager {
    pub fn new() -> Self {
        Self {
            windows: Vec::new(),
            next_id: 1,
            focused: None,
        }
    }

    /// Create a new window and return its ID.
    pub fn create(&mut self, config: WindowConfig) -> WindowId {
        let id = WindowId::new(self.next_id);
        self.next_id += 1;
        let area = Rect::new(0.0, 0.0, config.size.width, config.size.height);
        self.windows.push(WindowState {
            id,
            config,
            area,
            focused: false,
            visible: true,
        });
        id
    }

    /// Close a window by ID. Returns true if the window existed.
    pub fn close(&mut self, id: WindowId) -> bool {
        let len_before = self.windows.len();
        self.windows.retain(|w| w.id != id);
        if self.focused == Some(id) {
            self.focused = self.windows.last().map(|w| w.id);
        }
        self.windows.len() < len_before
    }

    /// Focus a window by ID.
    pub fn focus(&mut self, id: WindowId) {
        for w in &mut self.windows {
            w.focused = w.id == id;
        }
        self.focused = Some(id);
    }

    /// Get the currently focused window ID.
    pub fn focused(&self) -> Option<WindowId> {
        self.focused
    }

    /// Get all window states.
    pub fn windows(&self) -> &[WindowState] {
        &self.windows
    }

    /// Get a window state by ID.
    pub fn get(&self, id: WindowId) -> Option<&WindowState> {
        self.windows.iter().find(|w| w.id == id)
    }

    /// Get a mutable window state by ID.
    pub fn get_mut(&mut self, id: WindowId) -> Option<&mut WindowState> {
        self.windows.iter_mut().find(|w| w.id == id)
    }

    /// Number of open windows.
    pub fn count(&self) -> usize {
        self.windows.len()
    }
}

impl Default for WindowManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_id_display() {
        let id = WindowId::new(42);
        assert_eq!(format!("{id}"), "Window(42)");
    }

    #[test]
    fn window_config_defaults() {
        let cfg = WindowConfig::default();
        assert!(cfg.resizable);
        assert!(cfg.decorations);
        assert!(!cfg.transparent);
    }

    #[test]
    fn window_config_builder() {
        let cfg = WindowConfig::new("Test")
            .size(1024.0, 768.0)
            .resizable(false)
            .always_on_top(true);
        assert_eq!(cfg.title, "Test");
        assert!(!cfg.resizable);
        assert!(cfg.always_on_top);
    }

    #[test]
    fn window_manager_create() {
        let mut wm = WindowManager::new();
        let id = wm.create(WindowConfig::new("Main"));
        assert_eq!(wm.count(), 1);
        assert!(wm.get(id).is_some());
    }

    #[test]
    fn window_manager_multiple() {
        let mut wm = WindowManager::new();
        let id1 = wm.create(WindowConfig::new("One"));
        let id2 = wm.create(WindowConfig::new("Two"));
        assert_ne!(id1, id2);
        assert_eq!(wm.count(), 2);
    }

    #[test]
    fn window_manager_close() {
        let mut wm = WindowManager::new();
        let id = wm.create(WindowConfig::new("X"));
        assert!(wm.close(id));
        assert_eq!(wm.count(), 0);
        assert!(!wm.close(id)); // already closed
    }

    #[test]
    fn window_manager_focus() {
        let mut wm = WindowManager::new();
        let id1 = wm.create(WindowConfig::new("A"));
        let id2 = wm.create(WindowConfig::new("B"));
        wm.focus(id1);
        assert_eq!(wm.focused(), Some(id1));
        wm.focus(id2);
        assert_eq!(wm.focused(), Some(id2));
        assert!(!wm.get(id1).unwrap().focused);
        assert!(wm.get(id2).unwrap().focused);
    }

    #[test]
    fn window_manager_close_focused_shifts() {
        let mut wm = WindowManager::new();
        let id1 = wm.create(WindowConfig::new("A"));
        let id2 = wm.create(WindowConfig::new("B"));
        wm.focus(id1);
        wm.close(id1);
        assert_eq!(wm.focused(), Some(id2));
    }
}
