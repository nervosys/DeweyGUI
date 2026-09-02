//! Structural checks an agent can run against a rendered interface.
//!
//! An agent that scaffolds a GUI needs to know whether what it built actually
//! works, and some of the ways it can fail are invisible: a button with no id
//! renders perfectly and is simply dead — not hit-testable, not addressable.
//! That mistake was made while writing this project's own benchmarks. These
//! checks catch that class of fault without a window or a screenshot.

use serde::{Deserialize, Serialize};

/// How badly wrong a finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// The interface is broken: something cannot be seen, clicked, or addressed.
    Error,
    /// Suspicious but possibly deliberate.
    Warning,
}

/// One structural problem found in a rendered interface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    /// Stable machine-readable code, e.g. `unaddressable_widget`.
    pub code: &'static str,
    /// Human- and agent-readable explanation.
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub widget_type: Option<String>,
}

impl Diagnostic {
    fn new(severity: Severity, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            severity,
            code,
            message: message.into(),
            agent_id: None,
            widget_type: None,
        }
    }

    #[must_use]
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.agent_id = Some(id.into());
        self
    }

    #[must_use]
    pub fn with_type(mut self, ty: impl Into<String>) -> Self {
        self.widget_type = Some(ty.into());
        self
    }
}

/// Relative luminance, per WCAG 2.1.
///
/// The sRGB channels are linearised before weighting, which is why this is not
/// a plain average: a mid grey and a mid green look nothing alike to an eye.
fn luminance(color: crate::core::Color) -> f32 {
    fn channel(c: f32) -> f32 {
        let c = c.clamp(0.0, 1.0);
        if c <= 0.039_28 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * channel(color.r) + 0.7152 * channel(color.g) + 0.0722 * channel(color.b)
}

/// WCAG contrast ratio between two colours, from 1.0 (identical) to 21.0.
#[must_use]
pub fn contrast_ratio(a: crate::core::Color, b: crate::core::Color) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// The threshold below which text is treated as unreadable.
///
/// WCAG AA asks for 4.5 for body text and 3.0 for large text. This reports
/// well below either, because the point is to catch text that cannot be read
/// at all rather than to grade a palette — a framework that argued with every
/// deliberate design choice would be turned off.
const UNREADABLE_BELOW: f32 = 1.6;

/// Check what was painted for text nobody can read.
///
/// The one seeded fault `check` misses is a label painted white on white:
/// correct id, real bounds, on screen, fully wired, and invisible. Structure
/// cannot show it, so this reads the draw commands instead — for each piece of
/// text, the last filled rectangle underneath it is the ground it sits on.
///
/// This is not a substitute for looking at the interface. It catches text
/// against a flat fill, which is the case that happens by accident.
#[must_use]
pub fn check_contrast(ops: &[crate::backend::test::RenderOp]) -> Vec<Diagnostic> {
    use crate::backend::test::RenderOp;

    let mut out = Vec::new();
    for (index, op) in ops.iter().enumerate() {
        let RenderOp::Text {
            position,
            text,
            color,
            ..
        } = op
        else {
            continue;
        };
        if text.trim().is_empty() {
            continue;
        }

        // The ground is the last fill drawn before this text that covers it.
        let ground = ops[..index].iter().rev().find_map(|earlier| match earlier {
            RenderOp::FillRect { rect, color, .. }
                if rect.x <= position.x
                    && position.x <= rect.x + rect.width
                    && rect.y <= position.y
                    && position.y <= rect.y + rect.height =>
            {
                Some(*color)
            }
            _ => None,
        });
        let Some(ground) = ground else {
            continue;
        };
        // Text drawn over something translucent is not a flat comparison.
        if ground.a < 0.99 || color.a < 0.99 {
            continue;
        }

        let ratio = contrast_ratio(*color, ground);
        if ratio < UNREADABLE_BELOW {
            out.push(
                Diagnostic::new(
                    Severity::Error,
                    "unreadable_text",
                    format!(
                        "\"{}\" is drawn at contrast {ratio:.2} against what is behind it; \
                         below {UNREADABLE_BELOW:.1} it cannot be read at all",
                        text.chars().take(40).collect::<String>()
                    ),
                )
                .with_type("Text"),
            );
        }
    }
    out
}

/// Check a rendered tree for structural faults.
///
/// `unaddressable` lists widget types that rendered while declaring actions but
/// carrying no id — the runtime collects these during the frame, because a
/// widget with no id never reaches the tree to be inspected afterwards.
///
/// `strict` is for an application that means to be driven unattended. It
/// promotes every warning to an error and adds one check that is otherwise
/// silent: a widget publishing actions with nothing wired to any of them. That
/// is legitimate when the application answers through `Model::execute_action`,
/// which is why it is not reported by default — and it is also exactly how
/// `Canvas`, `Chart` and `RichText` came to accept `clear` and do nothing.
#[must_use]
pub fn check(
    tree: &super::UiTree,
    unaddressable: &[&'static str],
    window: crate::core::Size,
    handlers: &[(String, &'static str)],
    registry: &super::OntologyRegistry,
    strict: bool,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    let mut stack = vec![&tree.root];
    while let Some(node) = stack.pop() {
        if let Some(id) = node.agent_id.as_deref() {
            *seen.entry(id.to_string()).or_insert(0) += 1;

            if let Some(b) = node.bounds {
                if b.width <= 0.0 || b.height <= 0.0 {
                    out.push(
                        Diagnostic::new(
                            Severity::Error,
                            "zero_size_widget",
                            format!(
                                "`{id}` has no area ({}x{}), so it cannot be seen or clicked",
                                b.width, b.height
                            ),
                        )
                        .with_id(id)
                        .with_type(node.widget_type.as_ref()),
                    );
                } else if b.x >= window.width
                    || b.y >= window.height
                    || b.x + b.width <= 0.0
                    || b.y + b.height <= 0.0
                {
                    out.push(
                        Diagnostic::new(
                            Severity::Warning,
                            "offscreen_widget",
                            format!(
                                "`{id}` lies outside the {}x{} window and cannot be reached",
                                window.width, window.height
                            ),
                        )
                        .with_id(id)
                        .with_type(node.widget_type.as_ref()),
                    );
                }
            }
        }
        for child in &node.children {
            stack.push(child);
        }
    }

    for (id, count) in seen {
        if count > 1 {
            out.push(
                Diagnostic::new(
                    Severity::Error,
                    "duplicate_agent_id",
                    format!("`{id}` is used by {count} widgets; an action naming it is ambiguous"),
                )
                .with_id(id),
            );
        }
    }

    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for ty in unaddressable {
        *counts.entry(ty).or_insert(0) += 1;
    }
    let mut counts: Vec<_> = counts.into_iter().collect();
    counts.sort_unstable();
    for (ty, count) in counts {
        out.push(
            Diagnostic::new(
                Severity::Error,
                "unaddressable_widget",
                format!(
                    "{count} `{ty}` widget(s) rendered without an id, so they are not \
                     hit-testable and no agent can act on them; give each one `.action(id, msg)`"
                ),
            )
            .with_type(ty),
        );
    }

    // A handler bound to an action the widget's own ontology does not
    // advertise is unreachable: the agent is told to call one name and the
    // application answers another. `Checkbox` advertises `toggle`, and a
    // handler registered under `click` looked correct and could not be fired.
    let by_id: std::collections::HashMap<&str, &super::UiNode> = collect_ids(&tree.root);
    for (id, action) in handlers {
        let Some(node) = by_id.get(id.as_str()) else {
            continue;
        };
        let Some(schema) = registry.get_schema(&node.widget_type) else {
            continue;
        };
        if !schema.actions.is_empty() && !schema.actions.iter().any(|a| a.name == *action) {
            let advertised: Vec<_> = schema.actions.iter().map(|a| a.name.as_str()).collect();
            out.push(
                Diagnostic::new(
                    Severity::Error,
                    "unadvertised_action",
                    format!(
                        "`{id}` handles `{action}`, which `{}` does not advertise; an agent following the ontology would call one of: {}",
                        node.widget_type,
                        advertised.join(", ")
                    ),
                )
                .with_id(id.clone())
                .with_type(node.widget_type.as_ref()),
            );
        }
    }

    // A widget that publishes an action nothing is wired to accepts the call
    // and does nothing, which is worse than refusing it: the agent has no way
    // to tell the difference and moves on believing the interface changed.
    //
    // Only widgets the application has already wired at least once are
    // checked. Wiring none of a widget's actions means the application drives
    // it through `execute_action` instead, which is a different style, not a
    // fault.
    let mut wired: std::collections::HashMap<&str, Vec<&str>> = std::collections::HashMap::new();
    for (id, action) in handlers {
        wired.entry(id.as_str()).or_default().push(action);
    }
    let mut partial: Vec<Diagnostic> = Vec::new();
    for (id, actions) in &wired {
        let Some(node) = by_id.get(id) else { continue };
        let Some(schema) = registry.get_schema(&node.widget_type) else {
            continue;
        };
        let missing: Vec<&str> = schema
            .actions
            .iter()
            .filter(|a| a.mutates && !actions.contains(&a.name.as_str()))
            .map(|a| a.name.as_str())
            .collect();
        if !missing.is_empty() {
            partial.push(
                Diagnostic::new(
                    Severity::Warning,
                    "unhandled_action",
                    format!(
                        "`{id}` is wired for {}, but `{}` also advertises {}; calling those succeeds and changes nothing",
                        actions.join(", "),
                        node.widget_type,
                        missing.join(", ")
                    ),
                )
                .with_id((*id).to_string())
                .with_type(node.widget_type.as_ref()),
            );
        }
    }
    out.append(&mut partial);

    // Strict only: a widget that advertises actions and wires none of them.
    // Silent by default because answering through `Model::execute_action` is a
    // different style rather than a fault; an application that opts into strict
    // has said it does not use that style.
    if strict {
        let mut stack = vec![&tree.root];
        let mut unwired: Vec<Diagnostic> = Vec::new();
        while let Some(node) = stack.pop() {
            for child in &node.children {
                stack.push(child);
            }
            let Some(id) = node.agent_id.as_deref() else {
                continue;
            };
            if wired.contains_key(id) {
                continue;
            }
            let Some(schema) = registry.get_schema(&node.widget_type) else {
                continue;
            };
            let mutating: Vec<&str> = schema
                .actions
                .iter()
                .filter(|a| a.mutates)
                .map(|a| a.name.as_str())
                .collect();
            if mutating.is_empty() {
                continue;
            }
            unwired.push(
                Diagnostic::new(
                    Severity::Error,
                    "unwired_widget",
                    format!(
                        "`{id}` publishes {} but has no handler for any of them; \
                         under strict validation every advertised action must be \
                         reachable",
                        mutating.join(", ")
                    ),
                )
                .with_id(id)
                .with_type(node.widget_type.as_ref()),
            );
        }
        unwired.sort_by(|a, b| a.agent_id.cmp(&b.agent_id));
        out.append(&mut unwired);

        for diagnostic in &mut out {
            diagnostic.severity = Severity::Error;
        }
    }

    out.sort_by(|a, b| (a.code, &a.agent_id).cmp(&(b.code, &b.agent_id)));
    out
}

fn collect_ids(root: &super::UiNode) -> std::collections::HashMap<&str, &super::UiNode> {
    let mut map = std::collections::HashMap::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if let Some(id) = node.agent_id.as_deref() {
            map.insert(id, node);
        }
        for child in &node.children {
            stack.push(child);
        }
    }
    map
}
