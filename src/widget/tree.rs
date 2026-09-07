//! Tree widget — a hierarchical tree view.

use crate::core::{Color, Position, Rect, Style};
use crate::ontology::*;
use crate::runtime::Frame;
use crate::widget::Widget;

/// A tree node.
#[derive(Debug, Clone)]
pub struct TreeNode {
    pub label: String,
    pub children: Vec<TreeNode>,
    pub expanded: bool,
}

impl TreeNode {
    #[must_use]
    pub fn leaf(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            children: Vec::new(),
            expanded: false,
        }
    }

    #[must_use]
    pub fn branch(label: impl Into<String>, children: Vec<TreeNode>) -> Self {
        Self {
            label: label.into(),
            children,
            expanded: true,
        }
    }

    /// Find a mutable node by slash-separated path (e.g. "root/child/leaf").
    /// The node at `label/label/label` from this one, if there is one.
    ///
    /// Public because `on_change` hands an application a
    /// [`TreeChange::Expand`] carrying exactly such a path, and until this
    /// there was no way in the public API to do anything with it. A callback
    /// whose payload the crate gives you no means to apply is a callback that
    /// looks wired and is not.
    pub fn find_by_path_mut(&mut self, path: &str) -> Option<&mut TreeNode> {
        let mut parts = path.splitn(2, '/');
        let head = parts.next()?;
        if self.label != head {
            return None;
        }
        match parts.next() {
            None => Some(self),
            Some(rest) => self
                .children
                .iter_mut()
                .find_map(|child| child.find_by_path_mut(rest)),
        }
    }

    /// Serialize the tree node hierarchy for the agent state.
    /// The node at `label/label/label` from this one, without borrowing it
    /// mutably.
    #[must_use]
    pub fn find_by_path(&self, path: &str) -> Option<&TreeNode> {
        let mut parts = path.splitn(2, '/');
        let head = parts.next()?;
        if self.label != head {
            return None;
        }
        match parts.next() {
            None => Some(self),
            Some(rest) => self
                .children
                .iter()
                .find_map(|child| child.find_by_path(rest)),
        }
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "label": self.label,
            "expanded": self.expanded,
            "children": self.children.iter().map(|c| c.to_json()).collect::<Vec<_>>(),
        })
    }
}

/// What an agent asked a [`Tree`] to do.
///
/// A tree advertises four actions rather than one, so a single `on_click`-style
/// builder would misreport it. One handler covers them all, and the match arms
/// name the same strings the ontology publishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeChange<'a> {
    /// Expand the node at this slash-separated path.
    Expand(&'a str),
    /// Collapse the node at this slash-separated path.
    Collapse(&'a str),
    /// Expand every node.
    ExpandAll,
    /// Collapse every node.
    CollapseAll,
}

/// A hierarchical tree view.
pub struct Tree {
    root: TreeNode,
    style: Style,
    agent_id: std::borrow::Cow<'static, str>,
    /// Changes to apply, one per action this widget advertises.
    handlers: Vec<(&'static str, Box<dyn std::any::Any + Send>)>,
}

impl Tree {
    #[must_use]
    pub fn new(root: TreeNode) -> Self {
        Self {
            root,
            style: Style::default(),
            agent_id: std::borrow::Cow::Borrowed(""),
            handlers: Vec::new(),
        }
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn fg(mut self, color: Color) -> Self {
        self.style.foreground = Some(color);
        self
    }

    pub fn agent_id(mut self, id: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        self.agent_id = id.into();
        self
    }

    /// Name this tree and give it the change to apply for every action it
    /// advertises.
    ///
    /// The closure runs for `expand`, `collapse`, `expand_all` and
    /// `collapse_all` alike, so an agent reading the ontology reaches the
    /// application whichever it calls. Without this, a `Tree` rejects every
    /// action: its state lives outside the widget, so `execute_action` on the
    /// widget itself has nothing to change.
    #[must_use]
    pub fn on_change<M: 'static>(
        mut self,
        id: impl Into<std::borrow::Cow<'static, str>>,
        f: impl Fn(&mut M, TreeChange<'_>) + Send + Sync + 'static,
    ) -> Self {
        self.agent_id = id.into();
        let f = std::sync::Arc::new(f);
        for action in ["expand", "collapse", "expand_all", "collapse_all"] {
            let f = f.clone();
            let handler: crate::runtime::ValueMutation<M> =
                Box::new(move |m: &mut M, v: &serde_json::Value| {
                    let path = v
                        .get("path")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default();
                    let change = match action {
                        "expand" => TreeChange::Expand(path),
                        "collapse" => TreeChange::Collapse(path),
                        "expand_all" => TreeChange::ExpandAll,
                        _ => TreeChange::CollapseAll,
                    };
                    f(m, change);
                });
            self.handlers.push((action, Box::new(handler)));
        }
        self
    }
}

/// Actions on the widget value itself.
///
/// Not the path an agent takes: a widget is rebuilt inside `view` on
/// every frame, so a change made here lasts until the next redraw.
/// An agent's `execute_action` reaches a handler and then
/// [`Model::execute_action`](crate::runtime::Model::execute_action).
/// This stays because the logic is worth testing on its own.
impl Tree {
    pub fn execute_action(
        &mut self,
        action: &str,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        match action {
            "expand" => {
                let path = params
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing 'path' parameter")?;
                let node = self
                    .root
                    .find_by_path_mut(path)
                    .ok_or_else(|| format!("Node not found: {path}"))?;
                node.expanded = true;
                Ok(serde_json::json!({ "expanded": true, "path": path }))
            }
            "collapse" => {
                let path = params
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing 'path' parameter")?;
                let node = self
                    .root
                    .find_by_path_mut(path)
                    .ok_or_else(|| format!("Node not found: {path}"))?;
                node.expanded = false;
                Ok(serde_json::json!({ "expanded": false, "path": path }))
            }
            "expand_all" => {
                fn expand_all(node: &mut TreeNode) {
                    node.expanded = true;
                    for child in &mut node.children {
                        expand_all(child);
                    }
                }
                expand_all(&mut self.root);
                Ok(serde_json::json!({ "expanded_all": true }))
            }
            "collapse_all" => {
                fn collapse_all(node: &mut TreeNode) {
                    node.expanded = false;
                    for child in &mut node.children {
                        collapse_all(child);
                    }
                }
                collapse_all(&mut self.root);
                Ok(serde_json::json!({ "collapsed_all": true }))
            }
            _ => Err(format!("Unknown action: {action}")),
        }
    }
}

impl Discoverable for Tree {
    fn schema(&self) -> WidgetSchema {
        let mut schema =
            WidgetSchema::new("Tree", "A hierarchical tree view", SemanticRole::TreeNode);
        schema.usage_hint =
            Some("Tree::new(TreeNode::branch(\"root\", vec![TreeNode::leaf(\"item\")]))".into());
        schema.tags = vec![
            "tree".into(),
            "hierarchy".into(),
            "treeview".into(),
            "nodes".into(),
        ];
        schema
    }

    fn capabilities(&self) -> Vec<AgentCapability> {
        vec![
            AgentCapability::Expandable {
                expanded: self.root.expanded,
            },
            AgentCapability::Selectable {
                multi_select: false,
                item_count: 0,
            },
            AgentCapability::Focusable,
        ]
    }

    fn actions(&self) -> Vec<AgentAction> {
        vec![
            AgentAction::with_params(
                "expand",
                "Expand a tree node by path (e.g. 'root/child')",
                vec![ActionParam::required(
                    "path",
                    "Slash-separated node path",
                    ActionParamType::String,
                )],
                true,
            ),
            AgentAction::with_params(
                "collapse",
                "Collapse a tree node by path (e.g. 'root/child')",
                vec![ActionParam::required(
                    "path",
                    "Slash-separated node path",
                    ActionParamType::String,
                )],
                true,
            ),
            AgentAction::simple("expand_all", "Expand all tree nodes", true),
            AgentAction::simple("collapse_all", "Collapse all tree nodes", true),
        ]
    }

    fn semantic_role(&self) -> SemanticRole {
        SemanticRole::TreeNode
    }

    fn agent_state(&self) -> serde_json::Value {
        self.root.to_json()
    }

    fn agent_id(&self) -> Option<&str> {
        if self.agent_id.is_empty() {
            None
        } else {
            Some(&self.agent_id)
        }
    }

    fn accessibility_label(&self) -> Option<String> {
        Some(self.root.label.clone())
    }
}

impl Widget for Tree {
    fn render(mut self, area: Rect, frame: &mut Frame<'_>) {
        if !self.agent_id.is_empty() {
            for (action, handler) in self.handlers.drain(..) {
                frame.register_message(self.agent_id.clone(), action, handler);
            }
        }
        if frame.describes(area) && !self.agent_id.is_empty() {
            let node = UiNode::new("Tree", SemanticRole::TreeNode)
                .with_id(self.agent_id.clone())
                .with_bounds(area.into())
                .with_property("root", self.root.to_json());
            frame.register_widget(node);
        }

        // One flatten, used to paint and to answer a click. Two traversals of
        // the same tree is how a click comes to land one row off the row it
        // was drawn on.
        let mut rows = Vec::new();
        flatten(&self.root, 0, String::new(), &mut rows);

        if !self.agent_id.is_empty() {
            // Without a hitbox this widget could be operated by an agent and
            // by nobody else: no click reached it, and the focus ring is built
            // from the hit map, so Tab could not reach it either. A `Tree`
            // that advertises `expand` and cannot be expanded by a person is
            // the same defect as a `Button` whose action goes nowhere.
            frame.register_hitbox(self.agent_id.clone(), area, 1);

            // Clicking a branch toggles it, which needs the action chosen from
            // the row rather than fixed: a click that could only ever fire the
            // first handler registered would expand a tree and never collapse
            // one.
            let targets: Vec<(String, bool, bool)> = rows
                .iter()
                .map(|r| (r.path.clone(), r.leaf, r.expanded))
                .collect();
            frame.register_click(
                self.agent_id.clone(),
                crate::runtime::ClickParams::from_position(move |at| {
                    let row = ((at.y - area.y - TOP_PAD) / ROW_HEIGHT).floor();
                    if row < 0.0 {
                        return None;
                    }
                    let (path, leaf, expanded) = targets.get(row as usize)?;
                    // A leaf has nothing to expand. Firing an expand that does
                    // nothing is the same lie as clamping to the nearest row.
                    if *leaf {
                        return None;
                    }
                    let action = if *expanded { "collapse" } else { "expand" };
                    Some(crate::runtime::Click::action(
                        action,
                        serde_json::json!({ "path": path }),
                    ))
                }),
            );
        }

        frame.painter().push_clip(area);
        let ts = self.style.resolved_text();
        for (i, row) in rows.iter().enumerate() {
            let prefix = if row.leaf {
                "  "
            } else if row.expanded {
                "▼ "
            } else {
                "▶ "
            };
            let x = area.x + row.depth as f32 * INDENT + 4.0;
            let y = area.y + TOP_PAD + i as f32 * ROW_HEIGHT;
            frame
                .painter()
                .text(Position::new(x, y), &format!("{prefix}{}", row.label), &ts);
        }
        frame.painter().pop_clip();
    }
}

/// The height of one row, where the first one starts, and how far a level of
/// nesting moves it right.
///
/// Named because the painting and the click map have to agree, and a constant
/// written twice is how they stop agreeing.
const ROW_HEIGHT: f32 = 20.0;
const TOP_PAD: f32 = 4.0;
const INDENT: f32 = 16.0;

/// One visible row: what is painted, and what a click on it means.
struct Row {
    label: String,
    /// `label/label/label` from the root, which is what `expand` takes.
    path: String,
    depth: usize,
    expanded: bool,
    leaf: bool,
}

/// The rows a tree shows, in the order they are drawn.
///
/// A collapsed branch hides its children, so this is not the whole tree — it
/// is what a person can see, and therefore what a click can mean.
fn flatten(node: &TreeNode, depth: usize, prefix: String, out: &mut Vec<Row>) {
    let path = if prefix.is_empty() {
        node.label.clone()
    } else {
        format!("{prefix}/{}", node.label)
    };
    out.push(Row {
        label: node.label.clone(),
        path: path.clone(),
        depth,
        expanded: node.expanded,
        leaf: node.children.is_empty(),
    });
    if node.expanded {
        for child in &node.children {
            flatten(child, depth + 1, path.clone(), out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_expand_collapse() {
        let root = TreeNode::branch(
            "root",
            vec![
                TreeNode::branch("a", vec![TreeNode::leaf("a1"), TreeNode::leaf("a2")]),
                TreeNode::leaf("b"),
            ],
        );
        let mut tree = Tree::new(root);

        // Collapse root/a
        let result = tree
            .execute_action("collapse", &serde_json::json!({"path": "root/a"}))
            .unwrap();
        assert_eq!(result["expanded"], false);

        // Verify it's collapsed
        assert!(!tree.root.children[0].expanded);

        // Expand it back
        let result = tree
            .execute_action("expand", &serde_json::json!({"path": "root/a"}))
            .unwrap();
        assert_eq!(result["expanded"], true);
        assert!(tree.root.children[0].expanded);
    }

    #[test]
    fn tree_expand_collapse_all() {
        let root = TreeNode::branch(
            "root",
            vec![TreeNode::branch(
                "a",
                vec![TreeNode::branch("b", vec![TreeNode::leaf("c")])],
            )],
        );
        let mut tree = Tree::new(root);

        tree.execute_action("collapse_all", &serde_json::json!({}))
            .unwrap();
        assert!(!tree.root.expanded);
        assert!(!tree.root.children[0].expanded);

        tree.execute_action("expand_all", &serde_json::json!({}))
            .unwrap();
        assert!(tree.root.expanded);
        assert!(tree.root.children[0].expanded);
    }

    #[test]
    fn tree_invalid_path() {
        let root = TreeNode::leaf("root");
        let mut tree = Tree::new(root);
        let result = tree.execute_action("expand", &serde_json::json!({"path": "nonexistent"}));
        assert!(result.is_err());
    }
}
