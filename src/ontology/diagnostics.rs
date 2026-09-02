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

/// Check a rendered tree for structural faults.
///
/// `unaddressable` lists widget types that rendered while declaring actions but
/// carrying no id — the runtime collects these during the frame, because a
/// widget with no id never reaches the tree to be inspected afterwards.
#[must_use]
pub fn check(
    tree: &super::UiTree,
    unaddressable: &[&'static str],
    window: crate::core::Size,
    handlers: &[(String, &'static str)],
    registry: &super::OntologyRegistry,
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
                        "`{id}` handles `{action}`, which `{}` does not advertise; an agent                          following the ontology would call one of: {}",
                        node.widget_type,
                        advertised.join(", ")
                    ),
                )
                .with_id(id.clone())
                .with_type(node.widget_type.as_ref()),
            );
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
