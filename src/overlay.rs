//! An overlay stack, for an application to drive.
//!
//! [`OverlayStack`] orders layers by z-index and answers which one owns a
//! point, which is the bookkeeping floating content needs.
//!
//! **Nothing in this crate drives it.** No frame renders a stack and no
//! backend consults one for hit-testing, so an overlay pushed here appears
//! nowhere and blocks nothing. The `Modal` widget draws its own backdrop and
//! is rendered by the application's `view` like any other widget; it does not
//! go through this module.

use crate::core::Rect;

/// A single overlay layer.
pub struct Overlay {
    /// Unique identifier for this overlay.
    pub id: String,
    /// The area this overlay occupies.
    pub bounds: Rect,
    /// Whether this overlay blocks input to layers below.
    pub modal: bool,
    /// Z-order (higher = on top).
    pub z_order: u32,
}

impl Overlay {
    /// Create a new non-modal overlay.
    pub fn new(id: impl Into<String>, bounds: Rect) -> Self {
        Self {
            id: id.into(),
            bounds,
            modal: false,
            z_order: 0,
        }
    }

    /// Make this overlay modal (blocks input below).
    pub fn modal(mut self) -> Self {
        self.modal = true;
        self
    }

    /// Set the z-order.
    pub fn with_z_order(mut self, z: u32) -> Self {
        self.z_order = z;
        self
    }
}

/// A stack of overlays, rendering from bottom to top.
pub struct OverlayStack {
    overlays: Vec<Overlay>,
    next_z: u32,
}

impl OverlayStack {
    /// Create an empty overlay stack.
    pub fn new() -> Self {
        Self {
            overlays: Vec::new(),
            next_z: 100,
        }
    }

    /// Push an overlay onto the stack.
    pub fn push(&mut self, mut overlay: Overlay) {
        overlay.z_order = self.next_z;
        self.next_z += 1;
        self.overlays.push(overlay);
    }

    /// Remove an overlay by ID.
    pub fn remove(&mut self, id: &str) -> Option<Overlay> {
        if let Some(pos) = self.overlays.iter().position(|o| o.id == id) {
            Some(self.overlays.remove(pos))
        } else {
            None
        }
    }

    /// Remove the topmost overlay.
    pub fn pop(&mut self) -> Option<Overlay> {
        self.overlays.pop()
    }

    /// Get all overlays in rendering order (bottom to top).
    pub fn iter(&self) -> impl Iterator<Item = &Overlay> {
        self.overlays.iter()
    }

    /// Whether the topmost overlay is modal.
    pub fn is_modal_active(&self) -> bool {
        self.overlays.last().is_some_and(|o| o.modal)
    }

    /// Whether any overlay with the given ID exists.
    pub fn contains(&self, id: &str) -> bool {
        self.overlays.iter().any(|o| o.id == id)
    }

    /// Number of active overlays.
    pub fn len(&self) -> usize {
        self.overlays.len()
    }

    /// Whether the stack is empty.
    pub fn is_empty(&self) -> bool {
        self.overlays.is_empty()
    }

    /// Clear all overlays.
    pub fn clear(&mut self) {
        self.overlays.clear();
        self.next_z = 100;
    }
}

impl Default for OverlayStack {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience for creating a centered modal dialog box.
pub struct ModalBox {
    /// Title of the modal.
    pub title: String,
    /// Width as fraction of viewport (0.0..1.0).
    pub width_frac: f32,
    /// Height as fraction of viewport (0.0..1.0).
    pub height_frac: f32,
}

impl ModalBox {
    /// Create a modal box with the given title.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            width_frac: 0.5,
            height_frac: 0.4,
        }
    }

    /// Set the relative width.
    pub fn width(mut self, frac: f32) -> Self {
        self.width_frac = frac.clamp(0.1, 1.0);
        self
    }

    /// Set the relative height.
    pub fn height(mut self, frac: f32) -> Self {
        self.height_frac = frac.clamp(0.1, 1.0);
        self
    }

    /// Calculate the centered bounds within the given viewport.
    pub fn bounds(&self, viewport: Rect) -> Rect {
        let w = viewport.width * self.width_frac;
        let h = viewport.height * self.height_frac;
        let x = viewport.x + (viewport.width - w) / 2.0;
        let y = viewport.y + (viewport.height - h) / 2.0;
        Rect::new(x, y, w, h)
    }

    /// Create a modal overlay for this box within the given viewport.
    pub fn as_overlay(&self, viewport: Rect) -> Overlay {
        Overlay::new(&self.title, self.bounds(viewport)).modal()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_stack_push_pop() {
        let mut stack = OverlayStack::new();
        stack.push(Overlay::new("a", Rect::new(0.0, 0.0, 100.0, 100.0)));
        stack.push(Overlay::new("b", Rect::new(0.0, 0.0, 200.0, 200.0)).modal());
        assert_eq!(stack.len(), 2);
        assert!(stack.is_modal_active());

        let top = stack.pop().unwrap();
        assert_eq!(top.id, "b");
        assert!(!stack.is_modal_active());
    }

    #[test]
    fn modal_box_centering() {
        let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
        let mb = ModalBox::new("test").width(0.5).height(0.5);
        let bounds = mb.bounds(viewport);
        assert!((bounds.x - 200.0).abs() < 0.01);
        assert!((bounds.y - 150.0).abs() < 0.01);
        assert!((bounds.width - 400.0).abs() < 0.01);
        assert!((bounds.height - 300.0).abs() < 0.01);
    }
}
