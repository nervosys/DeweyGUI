//! Agent session: processes agent requests against the ontology and model.

use std::collections::{HashMap, HashSet};

use crate::ontology::registry::OntologyRegistry;

use super::protocol::{
    AgentEvent, AgentRequest, AgentResponse, InjectedEvent, MIN_PROTOCOL_VERSION, PROTOCOL_VERSION,
    SERVER_CAPABILITIES,
};

/// Maximum subscriptions per agent session.
const MAX_SUBSCRIPTIONS: usize = 100;

/// Manages an agent's connection to a Dewey application.
#[derive(Default)]
pub struct AgentSession {
    subscriptions: HashSet<String>,
    /// Previous state snapshots keyed by agent_id for diffing.
    prev_states: HashMap<String, crate::ontology::Properties>,
}

impl AgentSession {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if the agent is subscribed to a specific event type.
    pub fn is_subscribed(&self, event_type: &str) -> bool {
        self.subscriptions.contains(event_type) || self.subscriptions.contains("*")
    }

    /// Compute state-change events by comparing current ontology tree to
    /// previously seen snapshots. Returns a list of [`AgentEvent::StateChanged`]
    /// for any widget whose serialized state differs from last time.
    pub fn compute_state_diffs(&mut self, registry: &OntologyRegistry) -> Vec<AgentEvent> {
        if !self.is_subscribed("state_changed") {
            return Vec::new();
        }

        let Some(tree) = registry.tree() else {
            return Vec::new();
        };

        let mut events = Vec::new();
        let mut stack: Vec<&crate::ontology::UiNode> = vec![&tree.root];

        while let Some(node) = stack.pop() {
            if let Some(ref id) = node.agent_id {
                let changed = match self.prev_states.get(id.as_ref()) {
                    Some(prev) => prev != &node.state,
                    None => true,
                };
                if changed {
                    events.push(AgentEvent::StateChanged {
                        agent_id: id.to_string(),
                        state: node.state.to_value(),
                    });
                    self.prev_states.insert(id.to_string(), node.state.clone());
                }
            }
            for child in &node.children {
                stack.push(child);
            }
        }

        events
    }

    /// Process an incoming agent request and return a response.
    ///
    /// Returns `(response, should_quit)`.
    pub fn process_request(
        &mut self,
        request: &AgentRequest,
        registry: &OntologyRegistry,
    ) -> (AgentResponse, bool) {
        match request {
            AgentRequest::QueryOntology { query, role } => {
                let schemas = if let Some(q) = query {
                    registry.search(q)
                } else {
                    registry
                        .list_types()
                        .iter()
                        .filter_map(|name| registry.get_schema(name))
                        .collect()
                };

                let schemas: Vec<_> = if let Some(role_str) = role {
                    schemas
                        .into_iter()
                        .filter(|s| s.default_role.to_string() == *role_str)
                        .collect()
                } else {
                    schemas
                };

                let data = serde_json::to_value(&schemas).unwrap_or_default();
                (AgentResponse::ok(data), false)
            }

            AgentRequest::GetSchema { widget_type } => match registry.get_schema(widget_type) {
                Some(schema) => {
                    let data = serde_json::to_value(schema).unwrap_or_default();
                    (AgentResponse::ok(data), false)
                }
                None => (
                    AgentResponse::err(format!("Unknown widget type: {widget_type}")),
                    false,
                ),
            },

            AgentRequest::GetTree => {
                let tree = registry.export_tree();
                (AgentResponse::ok(tree), false)
            }

            AgentRequest::GetState { agent_id } => match registry.find_node(agent_id) {
                Some(node) => {
                    let data = serde_json::json!({
                        "agent_id": node.agent_id,
                        "widget_type": node.widget_type,
                        "role": node.role,
                        "state": node.state.to_value(),
                        "label": node.label,
                        "bounds": node.bounds,
                        "capabilities": node.capabilities,
                    });
                    (AgentResponse::ok(data), false)
                }
                None => (
                    AgentResponse::err(format!("Widget not found: {agent_id}")),
                    false,
                ),
            },

            AgentRequest::ExecuteAction {
                agent_id,
                action,
                params,
            } => match registry.find_node(agent_id) {
                Some(node) => {
                    if let Err(e) =
                        registry.validate_action_params(&node.widget_type, action, params)
                    {
                        return (
                            AgentResponse::err(format!(
                                "Invalid params for {}.{}: {e}",
                                node.widget_type, action
                            )),
                            false,
                        );
                    }
                    let data = serde_json::json!({
                        "status": "dispatched",
                        "agent_id": agent_id,
                        "action": action,
                        "params": params,
                    });
                    (AgentResponse::ok(data), false)
                }
                None => (
                    AgentResponse::err(format!("Widget not found: {agent_id}")),
                    false,
                ),
            },

            AgentRequest::InjectEvent { .. } => (
                AgentResponse::ok(serde_json::json!({"status": "injected"})),
                false,
            ),

            AgentRequest::Subscribe { events } => {
                let remaining = MAX_SUBSCRIPTIONS.saturating_sub(self.subscriptions.len());
                let to_add: Vec<_> = events.iter().take(remaining).cloned().collect();
                self.subscriptions.extend(to_add);
                (
                    AgentResponse::ok(serde_json::json!({
                        "subscriptions": self.subscriptions.iter().collect::<Vec<_>>()
                    })),
                    false,
                )
            }

            AgentRequest::Unsubscribe { events } => {
                for e in events {
                    self.subscriptions.remove(e);
                }
                (
                    AgentResponse::ok(serde_json::json!({
                        "subscriptions": self.subscriptions.iter().collect::<Vec<_>>()
                    })),
                    false,
                )
            }

            AgentRequest::Ping => (
                AgentResponse::ok(serde_json::json!({
                    "status": "pong",
                    "protocol_version": PROTOCOL_VERSION,
                    "framework": "dewey",
                })),
                false,
            ),

            AgentRequest::Quit => (
                AgentResponse::ok(serde_json::json!({"status": "quitting"})),
                true,
            ),

            AgentRequest::Screenshot { format } => {
                // In headless mode, return the UI tree as a structured JSON "screenshot".
                // A real GPU-backed screenshot would require eframe's
                // `Frame::screenshot()`, which is only available in the egui backend.
                let tree = registry.export_tree();
                (
                    AgentResponse::ok(serde_json::json!({
                        "format": format,
                        "kind": "ui_tree",
                        "tree": tree,
                    })),
                    false,
                )
            }

            AgentRequest::BatchActions { actions } => {
                let mut results = Vec::new();
                for entry in actions {
                    match registry.find_node(&entry.agent_id) {
                        Some(node) => {
                            if let Err(e) = registry.validate_action_params(
                                &node.widget_type,
                                &entry.action,
                                &entry.params,
                            ) {
                                results.push(serde_json::json!({
                                    "agent_id": entry.agent_id,
                                    "action": entry.action,
                                    "status": "error",
                                    "error": format!("{}.{}: {e}", node.widget_type, entry.action),
                                }));
                            } else {
                                results.push(serde_json::json!({
                                    "agent_id": entry.agent_id,
                                    "action": entry.action,
                                    "status": "dispatched",
                                    "params": entry.params,
                                }));
                            }
                        }
                        None => {
                            results.push(serde_json::json!({
                                "agent_id": entry.agent_id,
                                "action": entry.action,
                                "status": "error",
                                "error": format!("Widget not found: {}", entry.agent_id),
                            }));
                        }
                    }
                }
                (
                    AgentResponse::ok(serde_json::json!({ "results": results })),
                    false,
                )
            }

            AgentRequest::Negotiate {
                client_version,
                capabilities,
            } => {
                let compatible =
                    *client_version >= MIN_PROTOCOL_VERSION && *client_version <= PROTOCOL_VERSION;

                let supported_caps: Vec<&str> = capabilities
                    .iter()
                    .filter_map(|c| {
                        if SERVER_CAPABILITIES.contains(&c.as_str()) {
                            Some(c.as_str())
                        } else {
                            None
                        }
                    })
                    .collect();

                (
                    AgentResponse::ok(serde_json::json!({
                        "protocol_version": PROTOCOL_VERSION,
                        "min_protocol_version": MIN_PROTOCOL_VERSION,
                        "client_version": client_version,
                        "compatible": compatible,
                        "supported_capabilities": supported_caps,
                        "server_capabilities": SERVER_CAPABILITIES,
                        "framework": "dewey",
                    })),
                    false,
                )
            }
        }
    }

    /// Convert an injected event to a framework Event.
    pub fn convert_injected_event(event: &InjectedEvent) -> Option<crate::event::Event> {
        use crate::event::*;
        match event {
            InjectedEvent::Key { code, modifiers } => {
                let key_code = parse_key_code(code)?;
                let mut mods = KeyModifiers::empty();
                for m in modifiers {
                    match m.to_lowercase().as_str() {
                        "shift" => mods |= KeyModifiers::SHIFT,
                        "ctrl" | "control" => mods |= KeyModifiers::CONTROL,
                        "alt" => mods |= KeyModifiers::ALT,
                        "super" | "meta" | "command" => mods |= KeyModifiers::SUPER,
                        _ => {}
                    }
                }
                Some(Event::Key(KeyEvent::new(key_code, mods)))
            }
            InjectedEvent::MouseClick { x, y, button } => {
                let btn = match button.to_lowercase().as_str() {
                    "left" => MouseButton::Left,
                    "right" => MouseButton::Right,
                    "middle" => MouseButton::Middle,
                    _ => MouseButton::Left,
                };
                Some(Event::Mouse(MouseEvent {
                    kind: MouseEventKind::Click(btn),
                    position: crate::core::Position::new(*x, *y),
                    modifiers: KeyModifiers::empty(),
                }))
            }
            InjectedEvent::MouseMove { x, y } => Some(Event::Mouse(MouseEvent {
                kind: MouseEventKind::Move,
                position: crate::core::Position::new(*x, *y),
                modifiers: KeyModifiers::empty(),
            })),
            InjectedEvent::MouseScroll {
                x,
                y,
                delta_x,
                delta_y,
            } => Some(Event::Mouse(MouseEvent {
                kind: MouseEventKind::Scroll {
                    delta_x: *delta_x,
                    delta_y: *delta_y,
                },
                position: crate::core::Position::new(*x, *y),
                modifiers: KeyModifiers::empty(),
            })),
            InjectedEvent::TextInput { text } => Some(Event::TextInput(text.clone())),
            InjectedEvent::Resize { width, height } => {
                Some(Event::Resize(crate::core::Size::new(*width, *height)))
            }
        }
    }
}

fn parse_key_code(code: &str) -> Option<crate::event::KeyCode> {
    use crate::event::KeyCode;
    match code.to_lowercase().as_str() {
        "enter" | "return" => Some(KeyCode::Enter),
        "tab" => Some(KeyCode::Tab),
        "backspace" => Some(KeyCode::Backspace),
        "escape" | "esc" => Some(KeyCode::Esc),
        "space" | " " => Some(KeyCode::Char(' ')),
        "left" => Some(KeyCode::Left),
        "right" => Some(KeyCode::Right),
        "up" => Some(KeyCode::Up),
        "down" => Some(KeyCode::Down),
        "home" => Some(KeyCode::Home),
        "end" => Some(KeyCode::End),
        "pageup" => Some(KeyCode::PageUp),
        "pagedown" => Some(KeyCode::PageDown),
        "delete" | "del" => Some(KeyCode::Delete),
        "insert" => Some(KeyCode::Insert),
        s if s.len() == 1 => s.chars().next().map(KeyCode::Char),
        s if s.starts_with('f') => s[1..].parse::<u8>().ok().map(KeyCode::F),
        _ => None,
    }
}
