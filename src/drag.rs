//! Turning pointer events into a drag.
//!
//! [`Event::DragDrop`](crate::event::Event::DragDrop) has existed since v1.1
//! and **no host could deliver one**. The vocabulary was complete — five kinds,
//! four payload types, a `source_id` and a `target_id` — and nothing produced
//! any of it: the agpu backend converted an `agpu::Event::DragDrop` that the
//! agpu crate never constructs, the default backend had no drag path at all,
//! and the agent protocol could not inject a release, so `handle_event` was
//! never called with one on any host, by anybody.
//!
//! A drag is not an event a backend reports. It is a reading of three ordinary
//! ones — a press, some movement, a release — and that reading is the part
//! worth having in one place: which widget it started on, which it is over
//! now, and whether letting go means a drop or a cancel.
//!
//! Every host feeds its mouse events to a [`DragTracker`] and delivers what
//! comes back. `tests/backend_parity.rs` fails for one that stops.

use crate::core::Position;
use crate::event::{DragDropEvent, DragDropKind, DragPayload, MouseEvent, MouseEventKind};

/// What a widget offers when a drag starts on it.
///
/// Registered with [`Frame::register_drag`](crate::runtime::Frame::register_drag)
/// and given the point the press landed on, because what is being dragged
/// usually depends on where: a list offers the row under the pointer, not the
/// list.
pub type DragSource = Box<dyn Fn(Position) -> Option<DragPayload> + Send>;

/// A drag in progress.
#[derive(Debug, Clone, PartialEq)]
struct Active {
    source_id: String,
    payload: DragPayload,
    /// The widget the pointer is over, if any. Kept so `DragOver` is sent when
    /// it changes rather than on every mouse move.
    over: Option<String>,
    /// Whether the pointer has actually moved since the press.
    ///
    /// A press and release without movement is a click, and reporting it as a
    /// drag as well would make every click on a draggable row a zero-distance
    /// drag ending in a drop on itself.
    moved: bool,
}

/// Reads a drag out of ordinary mouse events.
///
/// Feed it every mouse event with the result of hit-testing that position.
/// It answers with the drag events that happened, which may be none.
#[derive(Debug, Default)]
pub struct DragTracker {
    /// Where a press landed, until it is known whether it becomes a drag.
    pending: Option<(String, Position)>,
    active: Option<Active>,
}

impl DragTracker {
    /// A new tracker with no drag in progress.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether a drag is under way.
    #[must_use]
    pub fn is_dragging(&self) -> bool {
        self.active.is_some()
    }

    /// The widget a drag started on, if one is under way.
    #[must_use]
    pub fn source(&self) -> Option<&str> {
        self.active.as_ref().map(|a| a.source_id.as_str())
    }

    /// Read one mouse event.
    ///
    /// `hit` is the widget under `event.position`, and `payload_of` is asked
    /// what a widget offers when a press lands on it — `None` for a widget
    /// that is not draggable, which is most of them.
    pub fn handle(
        &mut self,
        event: &MouseEvent,
        hit: Option<&str>,
        payload_of: impl Fn(&str, Position) -> Option<DragPayload>,
    ) -> Vec<DragDropEvent> {
        let at = event.position;
        match event.kind {
            MouseEventKind::Click(_) => {
                // Remembered, not started. A press that never moves is a
                // click, and a widget that answers both would fire its action
                // and report a drag for the same gesture.
                self.pending = hit.and_then(|id| payload_of(id, at).map(|_| (id.to_string(), at)));
                Vec::new()
            }
            MouseEventKind::Move | MouseEventKind::Drag(_) => {
                let mut out = Vec::new();
                if let Some((source_id, from)) = self.pending.take() {
                    if let Some(payload) = payload_of(&source_id, from) {
                        out.push(DragDropEvent {
                            kind: DragDropKind::DragStart {
                                source_id: source_id.clone(),
                                payload: payload.clone(),
                            },
                            position: at,
                        });
                        self.active = Some(Active {
                            source_id,
                            payload,
                            over: None,
                            moved: true,
                        });
                    }
                }
                let Some(active) = self.active.as_mut() else {
                    return out;
                };
                active.moved = true;
                let now = hit.map(str::to_owned);
                if now != active.over {
                    // Leaving is reported before arriving, so a target that
                    // highlights itself is never told it has two.
                    if let Some(left) = active.over.take() {
                        out.push(DragDropEvent {
                            kind: DragDropKind::DragLeave { target_id: left },
                            position: at,
                        });
                    }
                    if let Some(entered) = now.clone() {
                        out.push(DragDropEvent {
                            kind: DragDropKind::DragOver { target_id: entered },
                            position: at,
                        });
                    }
                    active.over = now;
                }
                out
            }
            MouseEventKind::Release(_) => {
                self.pending = None;
                let Some(active) = self.active.take() else {
                    return Vec::new();
                };
                // Released over something, having moved: a drop. Released over
                // nothing, or without moving: a cancel. The difference matters
                // to an application, which is why both are in the vocabulary.
                match (active.moved, hit) {
                    (true, Some(target_id)) => vec![DragDropEvent {
                        kind: DragDropKind::Drop {
                            source_id: active.source_id,
                            target_id: target_id.to_string(),
                            payload: active.payload,
                        },
                        position: at,
                    }],
                    _ => vec![DragDropEvent {
                        kind: DragDropKind::DragCancel,
                        position: at,
                    }],
                }
            }
            MouseEventKind::Scroll { .. } => Vec::new(),
        }
    }

    /// Abandon any drag in progress, reporting the cancel.
    ///
    /// For a host that loses the pointer — the window losing focus, say —
    /// where no release will ever arrive.
    pub fn cancel(&mut self, at: Position) -> Vec<DragDropEvent> {
        self.pending = None;
        if self.active.take().is_some() {
            vec![DragDropEvent {
                kind: DragDropKind::DragCancel,
                position: at,
            }]
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::MouseButton;

    fn at(x: f32, y: f32, kind: MouseEventKind) -> MouseEvent {
        MouseEvent {
            kind,
            position: Position::new(x, y),
            modifiers: crate::event::KeyModifiers::default(),
        }
    }

    fn rows(_id: &str, _p: Position) -> Option<DragPayload> {
        Some(DragPayload::Index(3))
    }

    fn nothing(_id: &str, _p: Position) -> Option<DragPayload> {
        None
    }

    #[test]
    fn a_press_and_release_without_moving_is_not_a_drag() {
        let mut t = DragTracker::new();
        assert!(
            t.handle(
                &at(0.0, 0.0, MouseEventKind::Click(MouseButton::Left)),
                Some("a"),
                rows
            )
            .is_empty()
        );
        let out = t.handle(
            &at(0.0, 0.0, MouseEventKind::Release(MouseButton::Left)),
            Some("a"),
            rows,
        );
        assert!(
            out.is_empty(),
            "a click on a draggable widget reported a drag: {out:?}"
        );
    }

    #[test]
    fn press_move_release_is_a_drop() {
        let mut t = DragTracker::new();
        t.handle(
            &at(0.0, 0.0, MouseEventKind::Click(MouseButton::Left)),
            Some("from"),
            rows,
        );
        let started = t.handle(&at(5.0, 5.0, MouseEventKind::Move), Some("to"), rows);
        assert!(matches!(
            started.first().map(|e| &e.kind),
            Some(DragDropKind::DragStart { .. })
        ));
        assert!(started.iter().any(|e| matches!(
            &e.kind,
            DragDropKind::DragOver { target_id } if target_id == "to"
        )));
        let dropped = t.handle(
            &at(5.0, 5.0, MouseEventKind::Release(MouseButton::Left)),
            Some("to"),
            rows,
        );
        match dropped.first().map(|e| &e.kind) {
            Some(DragDropKind::Drop {
                source_id,
                target_id,
                payload,
            }) => {
                assert_eq!(source_id, "from");
                assert_eq!(target_id, "to");
                assert_eq!(payload, &DragPayload::Index(3));
            }
            other => panic!("expected a drop, got {other:?}"),
        }
        assert!(!t.is_dragging());
    }

    #[test]
    fn releasing_over_nothing_cancels() {
        let mut t = DragTracker::new();
        t.handle(
            &at(0.0, 0.0, MouseEventKind::Click(MouseButton::Left)),
            Some("from"),
            rows,
        );
        t.handle(&at(5.0, 5.0, MouseEventKind::Move), None, rows);
        let out = t.handle(
            &at(5.0, 5.0, MouseEventKind::Release(MouseButton::Left)),
            None,
            rows,
        );
        assert!(matches!(
            out.first().map(|e| &e.kind),
            Some(DragDropKind::DragCancel)
        ));
    }

    #[test]
    fn a_widget_that_offers_nothing_starts_no_drag() {
        let mut t = DragTracker::new();
        t.handle(
            &at(0.0, 0.0, MouseEventKind::Click(MouseButton::Left)),
            Some("plain"),
            nothing,
        );
        let out = t.handle(&at(9.0, 9.0, MouseEventKind::Move), Some("other"), nothing);
        assert!(out.is_empty(), "{out:?}");
        assert!(!t.is_dragging());
    }

    #[test]
    fn moving_between_targets_leaves_before_it_arrives() {
        let mut t = DragTracker::new();
        t.handle(
            &at(0.0, 0.0, MouseEventKind::Click(MouseButton::Left)),
            Some("from"),
            rows,
        );
        t.handle(&at(1.0, 1.0, MouseEventKind::Move), Some("a"), rows);
        let out = t.handle(&at(2.0, 2.0, MouseEventKind::Move), Some("b"), rows);
        let kinds: Vec<&DragDropKind> = out.iter().map(|e| &e.kind).collect();
        assert!(
            matches!(kinds.first(), Some(DragDropKind::DragLeave { target_id }) if target_id == "a"),
            "{kinds:?}"
        );
        assert!(
            matches!(kinds.get(1), Some(DragDropKind::DragOver { target_id }) if target_id == "b"),
            "{kinds:?}"
        );
    }
}
