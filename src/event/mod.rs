//! Event system for GUI input handling.
//!
//! Provides a unified event type for the runtime, with keyboard, mouse,
//! touch, window, and custom events.

use crate::core::rect::{Position, Size};
use serde::{Deserialize, Serialize};

/// A GUI event.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Keyboard event.
    Key(KeyEvent),
    /// Mouse event.
    Mouse(MouseEvent),
    /// Text input (already composed characters).
    TextInput(String),
    /// Window was resized to the given logical size.
    Resize(Size),
    /// Window gained focus.
    FocusGained,
    /// Window lost focus.
    FocusLost,
    /// Window close requested.
    CloseRequested,
    /// A scheduled tick from the runtime (for animations).
    Tick,
    /// File(s) dropped onto the window.
    FileDrop(Vec<String>),
    /// File(s) hovering over the window.
    FileHover(Vec<String>),
    /// File hover cancelled.
    FileHoverCancelled,
    /// A widget-to-widget drag-and-drop event.
    ///
    /// **No backend emits one.** The agpu backend converts an
    /// `agpu::Event::DragDrop` into this, and nothing in the agpu crate ever
    /// constructs one; winit reports file drops, which arrive as
    /// [`FileDrop`](Event::FileDrop) and are a different thing. The default
    /// backend has no drag-drop path at all, and the agent protocol cannot
    /// inject one.
    ///
    /// So this variant, [`DragDropEvent`] and [`DragPayload`] are the vocabulary
    /// for an application that tracks a drag itself out of
    /// [`Mouse`](Event::Mouse) events. They are not a drag-and-drop
    /// implementation, and `handle_event` will not be called with one.
    DragDrop(DragDropEvent),
}

/// A keyboard event.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
    pub kind: KeyEventKind,
}

impl KeyEvent {
    pub fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        Self {
            code,
            modifiers,
            kind: KeyEventKind::Press,
        }
    }

    pub fn is_ctrl(&self, code: KeyCode) -> bool {
        self.code == code && self.modifiers.contains(KeyModifiers::CONTROL)
    }

    pub fn is_key(&self, code: KeyCode) -> bool {
        self.code == code && self.modifiers.is_empty()
    }
}

/// The kind of key event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyEventKind {
    Press,
    Release,
    Repeat,
}

/// Key code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyCode {
    Char(char),
    F(u8),
    Backspace,
    Enter,
    Tab,
    BackTab,
    Esc,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    Null,
}

/// Key modifier flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct KeyModifiers(u8);

impl KeyModifiers {
    pub const NONE: Self = Self(0);
    pub const SHIFT: Self = Self(1 << 0);
    pub const CONTROL: Self = Self(1 << 1);
    pub const ALT: Self = Self(1 << 2);
    pub const SUPER: Self = Self(1 << 3);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl std::ops::BitOr for KeyModifiers {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for KeyModifiers {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// A mouse event.
#[derive(Debug, Clone, PartialEq)]
pub struct MouseEvent {
    pub kind: MouseEventKind,
    pub position: Position,
    pub modifiers: KeyModifiers,
}

impl MouseEvent {
    pub fn is_click(&self) -> bool {
        matches!(self.kind, MouseEventKind::Click(_))
    }

    pub fn is_drag(&self) -> bool {
        matches!(self.kind, MouseEventKind::Drag(_))
    }

    pub fn is_scroll(&self) -> bool {
        matches!(self.kind, MouseEventKind::Scroll { .. })
    }

    /// Whether the given rect was clicked.
    pub fn clicked_in(&self, area: crate::core::rect::Rect) -> bool {
        self.is_click() && area.contains(self.position)
    }
}

/// The kind of mouse event.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MouseEventKind {
    Click(MouseButton),
    Release(MouseButton),
    Drag(MouseButton),
    Move,
    Scroll { delta_x: f32, delta_y: f32 },
}

/// Mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// A hit-test map that resolves positions to widget IDs.
///
/// Widgets register their bounds during rendering; the runtime uses this
/// to route mouse events to the correct widget.
#[derive(Debug, Default)]
pub struct HitMap {
    entries: Vec<HitEntry>,
}

#[derive(Debug)]
struct HitEntry {
    agent_id: std::borrow::Cow<'static, str>,
    bounds: crate::core::rect::Rect,
    z_order: u32,
}

impl HitMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear all entries (called at the start of each frame).
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Register a widget's clickable bounds.
    pub fn register(
        &mut self,
        agent_id: impl Into<std::borrow::Cow<'static, str>>,
        bounds: crate::core::rect::Rect,
        z_order: u32,
    ) {
        self.entries.push(HitEntry {
            agent_id: agent_id.into(),
            bounds,
            z_order,
        });
    }

    /// The widgets that can take keyboard focus, in the order they rendered.
    ///
    /// A widget registers here when it is interactive and has an id, which is
    /// exactly the condition for being focusable — so the ring needs no second
    /// registration to fall out of step with. Render order is tab order.
    ///
    /// Duplicates are dropped: a widget that registers twice is one stop.
    pub fn focusables(&self) -> Vec<String> {
        let mut seen: Vec<String> = Vec::with_capacity(self.entries.len());
        for entry in &self.entries {
            let id = entry.agent_id.as_ref();
            if !seen.iter().any(|s| s == id) {
                seen.push(id.to_string());
            }
        }
        seen
    }

    /// Where a widget rendered, if it did.
    pub fn bounds_of(&self, agent_id: &str) -> Option<crate::core::rect::Rect> {
        self.entries
            .iter()
            .find(|e| e.agent_id.as_ref() == agent_id)
            .map(|e| e.bounds)
    }

    /// Find the widget at the given position (highest z-order wins).
    pub fn hit_test(&self, pos: Position) -> Option<&str> {
        self.entries
            .iter()
            .filter(|e| e.bounds.contains(pos))
            .max_by_key(|e| e.z_order)
            .map(|e| e.agent_id.as_ref())
    }
}

/// A drag-and-drop event.
#[derive(Debug, Clone, PartialEq)]
pub struct DragDropEvent {
    /// The kind of drag-drop event.
    pub kind: DragDropKind,
    /// Current pointer position during drag.
    pub position: Position,
}

/// The kind of drag-and-drop event.
#[derive(Debug, Clone, PartialEq)]
pub enum DragDropKind {
    /// A drag operation started from a widget.
    DragStart {
        /// Agent ID of the source widget.
        source_id: String,
        /// Payload being dragged (serialised data).
        payload: DragPayload,
    },
    /// The dragged item is hovering over a potential drop target.
    DragOver {
        /// Agent ID of the widget being hovered over.
        target_id: String,
    },
    /// The dragged item left a potential drop target.
    DragLeave {
        /// Agent ID of the widget the drag left.
        target_id: String,
    },
    /// The dragged item was dropped on a target.
    Drop {
        /// Agent ID of the source widget.
        source_id: String,
        /// Agent ID of the target widget.
        target_id: String,
        /// The payload that was dropped.
        payload: DragPayload,
    },
    /// The drag operation was cancelled.
    DragCancel,
}

/// Data carried during a drag-and-drop operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DragPayload {
    /// Plain text content.
    Text(String),
    /// An item index (e.g. from a list or table).
    Index(usize),
    /// A path in a tree widget.
    Path(Vec<String>),
    /// Arbitrary JSON data.
    Json(serde_json::Value),
}
