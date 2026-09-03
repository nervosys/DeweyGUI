//! Memory utilities, offered to applications.
//!
//! An [`Arena`] for per-frame temporaries, a [`VecPool`] for reusable buffers,
//! and an [`InlineString`] that keeps short strings off the heap.
//!
//! **Nothing in this crate drives it.** This module was listed on the roadmap
//! as "memory optimization" and no allocation in Dewey goes through any of it:
//! the frame path allocates with `Vec` and `String` like anything else, so
//! these types optimise nothing until an application reaches for them. The
//! per-frame allocation counts that did come down — 18.0 to 4.0 per row — came
//! from not building the nodes in the first place, and from borrowing the keys
//! that were being copied, which is a different technique entirely.

use std::cell::RefCell;

/// A simple bump allocator for per-frame temporary data.
///
/// Allocates from a pre-allocated byte buffer. Call [`reset()`](Arena::reset)
/// each frame to reclaim all memory without individual frees.
pub struct Arena {
    buf: Vec<u8>,
    offset: usize,
}

impl Arena {
    /// Create an arena with the given initial capacity in bytes.
    pub fn new(capacity: usize) -> Self {
        Self {
            buf: vec![0u8; capacity],
            offset: 0,
        }
    }

    /// Allocate `size` bytes, returning a mutable slice.
    /// Returns `None` if the arena is full.
    pub fn alloc(&mut self, size: usize) -> Option<&mut [u8]> {
        // Align to 8 bytes
        let aligned = (self.offset + 7) & !7;
        if aligned + size > self.buf.len() {
            return None;
        }
        self.offset = aligned + size;
        Some(&mut self.buf[aligned..aligned + size])
    }

    /// Reset the arena, making all previously allocated memory available again.
    pub fn reset(&mut self) {
        self.offset = 0;
    }

    /// Number of bytes currently allocated.
    pub fn used(&self) -> usize {
        self.offset
    }

    /// Total capacity in bytes.
    pub fn capacity(&self) -> usize {
        self.buf.len()
    }

    /// Remaining free bytes.
    pub fn remaining(&self) -> usize {
        self.buf.len().saturating_sub(self.offset)
    }
}

impl Default for Arena {
    fn default() -> Self {
        Self::new(64 * 1024) // 64 KB default
    }
}

/// A pool of reusable `Vec<T>` buffers.
///
/// Instead of allocating new vectors each frame, borrow from the pool
/// and return them when done. Returned vectors are cleared but retain
/// their capacity.
pub struct VecPool<T> {
    pool: RefCell<Vec<Vec<T>>>,
}

impl<T> VecPool<T> {
    /// Create an empty pool.
    pub fn new() -> Self {
        Self {
            pool: RefCell::new(Vec::new()),
        }
    }

    /// Borrow a vector from the pool (or create a new one if empty).
    pub fn get(&self) -> Vec<T> {
        self.pool.borrow_mut().pop().unwrap_or_default()
    }

    /// Return a vector to the pool. It is cleared but keeps its capacity.
    pub fn put(&self, mut v: Vec<T>) {
        v.clear();
        self.pool.borrow_mut().push(v);
    }

    /// Number of recycled vectors available.
    pub fn available(&self) -> usize {
        self.pool.borrow().len()
    }
}

impl<T> Default for VecPool<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// A compact inline string (up to 22 bytes without heap allocation).
///
/// Uses `compact_str::CompactString` under the hood but provides a
/// more ergonomic API for widget IDs and short labels.
pub type InlineString = compact_str::CompactString;

/// Utility: reuse a `String` buffer by clearing it instead of dropping.
pub fn reuse_string(s: &mut String) {
    s.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arena_alloc_and_reset() {
        let mut arena = Arena::new(256);
        let a = arena.alloc(32).unwrap();
        assert_eq!(a.len(), 32);
        assert!(arena.used() > 0);

        let b = arena.alloc(64).unwrap();
        assert_eq!(b.len(), 64);

        arena.reset();
        assert_eq!(arena.used(), 0);
        assert_eq!(arena.remaining(), 256);
    }

    #[test]
    fn arena_overflow() {
        let mut arena = Arena::new(16);
        assert!(arena.alloc(8).is_some());
        assert!(arena.alloc(100).is_none());
    }

    #[test]
    fn vec_pool_reuse() {
        let pool: VecPool<i32> = VecPool::new();
        let mut v = pool.get();
        v.push(1);
        v.push(2);
        v.push(3);
        let cap = v.capacity();
        pool.put(v);
        assert_eq!(pool.available(), 1);

        let v2 = pool.get();
        assert!(v2.is_empty());
        assert!(v2.capacity() >= cap); // Capacity is retained
        assert_eq!(pool.available(), 0);
    }
}
