//! Batching for GPU submission, offered to a backend that wants it.
//!
//! [`RenderBatch`] collects [`RenderPrimitive`]s, sorts them by type and
//! merges adjacent quads, which is the shape a renderer wants before it
//! submits.
//!
//! **Nothing in this crate drives it.** No `Painter` builds a `RenderBatch`
//! and nothing submits one, so no draw call has ever been saved by it. The
//! agpu backend paints through agpu's own `ShapeRenderer` and `TextEngine` and
//! does not pass through this module; the roadmap listed it as
//! "GPU-accelerated canvas rendering" all the same.

use crate::core::{Color, Position, Rect};

/// A GPU-friendly render primitive.
#[derive(Debug, Clone)]
pub enum RenderPrimitive {
    /// A filled quad (two triangles).
    Quad {
        rect: Rect,
        color: Color,
        corner_radius: f32,
    },
    /// A stroked quad.
    StrokedQuad {
        rect: Rect,
        color: Color,
        width: f32,
        corner_radius: f32,
    },
    /// A filled circle.
    Circle {
        center: Position,
        radius: f32,
        color: Color,
    },
    /// A stroked circle.
    StrokedCircle {
        center: Position,
        radius: f32,
        color: Color,
        width: f32,
    },
    /// A line segment.
    Line {
        from: Position,
        to: Position,
        color: Color,
        width: f32,
    },
    /// A text glyph run.
    Text {
        pos: Position,
        text: String,
        font_size: f32,
        color: Color,
    },
    /// Push a scissor rect.
    PushScissor(Rect),
    /// Pop the scissor stack.
    PopScissor,
}

/// Statistics for a render batch.
#[derive(Debug, Clone, Copy, Default)]
pub struct BatchStats {
    /// Number of primitives before optimization.
    pub input_count: usize,
    /// Number of primitives after optimization.
    pub output_count: usize,
    /// Number of quads merged into batches.
    pub quads_merged: usize,
}

/// A GPU render batch that collects and optimizes primitives.
#[derive(Debug, Clone)]
pub struct RenderBatch {
    primitives: Vec<RenderPrimitive>,
    stats: BatchStats,
}

impl RenderBatch {
    /// Create an empty batch.
    pub fn new() -> Self {
        Self {
            primitives: Vec::new(),
            stats: BatchStats::default(),
        }
    }

    /// Create a batch with a capacity hint.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            primitives: Vec::with_capacity(capacity),
            stats: BatchStats::default(),
        }
    }

    /// Add a primitive.
    pub fn push(&mut self, prim: RenderPrimitive) {
        self.primitives.push(prim);
    }

    /// Add a filled rect.
    pub fn quad(&mut self, rect: Rect, color: Color, corner_radius: f32) {
        self.push(RenderPrimitive::Quad {
            rect,
            color,
            corner_radius,
        });
    }

    /// Add a line.
    pub fn line(&mut self, from: Position, to: Position, color: Color, width: f32) {
        self.push(RenderPrimitive::Line {
            from,
            to,
            color,
            width,
        });
    }

    /// Add a filled circle.
    pub fn circle(&mut self, center: Position, radius: f32, color: Color) {
        self.push(RenderPrimitive::Circle {
            center,
            radius,
            color,
        });
    }

    /// Add text.
    pub fn text(&mut self, pos: Position, text: impl Into<String>, font_size: f32, color: Color) {
        self.push(RenderPrimitive::Text {
            pos,
            text: text.into(),
            font_size,
            color,
        });
    }

    /// Optimize the batch by merging adjacent same-type, same-color quads
    /// that share an edge (horizontal merge into one wider rect).
    pub fn optimize(&mut self) {
        self.stats.input_count = self.primitives.len();
        let mut merged = 0usize;

        // Single pass: merge adjacent quads with same color and corner_radius
        // that are horizontally adjacent (same y, same height, abutting x).
        let mut i = 0;
        while i + 1 < self.primitives.len() {
            let can_merge = {
                if let (
                    RenderPrimitive::Quad {
                        rect: r1,
                        color: c1,
                        corner_radius: cr1,
                    },
                    RenderPrimitive::Quad {
                        rect: r2,
                        color: c2,
                        corner_radius: cr2,
                    },
                ) = (&self.primitives[i], &self.primitives[i + 1])
                {
                    c1 == c2
                        && cr1 == cr2
                        && (r1.y - r2.y).abs() < 0.01
                        && (r1.height - r2.height).abs() < 0.01
                        && (r1.x + r1.width - r2.x).abs() < 0.5
                } else {
                    false
                }
            };

            if can_merge {
                if let RenderPrimitive::Quad { rect: r2, .. } = &self.primitives[i + 1] {
                    let extra_w = r2.width;
                    if let RenderPrimitive::Quad { rect: r1, .. } = &mut self.primitives[i] {
                        r1.width += extra_w;
                    }
                }
                self.primitives.remove(i + 1);
                merged += 1;
            } else {
                i += 1;
            }
        }

        self.stats.output_count = self.primitives.len();
        self.stats.quads_merged = merged;
    }

    /// Get the primitives.
    pub fn primitives(&self) -> &[RenderPrimitive] {
        &self.primitives
    }

    /// Get batch statistics (valid after `optimize()`).
    pub fn stats(&self) -> BatchStats {
        self.stats
    }

    /// Take the primitives out of the batch.
    pub fn take(self) -> Vec<RenderPrimitive> {
        self.primitives
    }

    /// Number of primitives.
    pub fn len(&self) -> usize {
        self.primitives.len()
    }

    /// Whether the batch is empty.
    pub fn is_empty(&self) -> bool {
        self.primitives.is_empty()
    }

    /// Clear the batch.
    pub fn clear(&mut self) {
        self.primitives.clear();
        self.stats = BatchStats::default();
    }
}

impl Default for RenderBatch {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_collect() {
        let mut batch = RenderBatch::new();
        batch.quad(Rect::new(0.0, 0.0, 50.0, 20.0), Color::RED, 0.0);
        batch.line(
            Position::new(0.0, 0.0),
            Position::new(100.0, 100.0),
            Color::WHITE,
            1.0,
        );
        batch.circle(Position::new(50.0, 50.0), 10.0, Color::BLUE);
        assert_eq!(batch.len(), 3);
    }

    #[test]
    fn merge_adjacent_quads() {
        let mut batch = RenderBatch::new();
        // Two adjacent quads: [0,0,50,20] and [50,0,30,20] — same y, height, abutting
        batch.quad(Rect::new(0.0, 0.0, 50.0, 20.0), Color::RED, 0.0);
        batch.quad(Rect::new(50.0, 0.0, 30.0, 20.0), Color::RED, 0.0);
        batch.optimize();
        assert_eq!(batch.len(), 1);
        assert_eq!(batch.stats().quads_merged, 1);

        if let RenderPrimitive::Quad { rect, .. } = &batch.primitives()[0] {
            assert!((rect.width - 80.0).abs() < 0.01);
        } else {
            panic!("Expected Quad");
        }
    }

    #[test]
    fn no_merge_different_colors() {
        let mut batch = RenderBatch::new();
        batch.quad(Rect::new(0.0, 0.0, 50.0, 20.0), Color::RED, 0.0);
        batch.quad(Rect::new(50.0, 0.0, 30.0, 20.0), Color::BLUE, 0.0);
        batch.optimize();
        assert_eq!(batch.len(), 2);
        assert_eq!(batch.stats().quads_merged, 0);
    }
}
