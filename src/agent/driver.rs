//! Headless agent driver: run a Dewey app without a visible window.
//!
//! Uses an offscreen renderer for programmatic control. This enables:
//! - Automated testing of Dewey apps
//! - Agent-only operation (no human at the screen)
//! - CI/CD pipeline integration

use crate::ontology::OntologyRegistry;
use crate::runtime::{Command, Frame, Model};

use super::protocol::{AgentRequest, AgentResponse, RequestEnvelope};
use super::session::AgentSession;

/// Run a Dewey application headlessly, driven entirely by agent protocol messages.
pub struct HeadlessDriver<M: Model> {
    model: M,
    session: AgentSession,
    ontology: OntologyRegistry,
    running: bool,
    window_size: crate::core::Size,
    hit_map: crate::event::HitMap,
    /// Messages registered by widgets during the last render, keyed by agent id.
    messages: std::collections::HashMap<String, Box<dyn std::any::Any + Send>>,
}

impl<M: Model> HeadlessDriver<M> {
    /// Create a new headless driver with the given model and virtual window size.
    pub fn new(model: M, width: f32, height: f32) -> Self {
        let mut ontology = OntologyRegistry::new();
        model.register_ontology(&mut ontology);

        Self {
            model,
            session: AgentSession::new(),
            ontology,
            running: true,
            window_size: crate::core::Size::new(width, height),
            hit_map: crate::event::HitMap::new(),
            messages: std::collections::HashMap::new(),
        }
    }

    /// Whether the application is still running.
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Access the current model.
    pub fn model(&self) -> &M {
        &self.model
    }

    /// Access the ontology registry.
    pub fn ontology(&self) -> &OntologyRegistry {
        &self.ontology
    }

    /// Access the agent session.
    pub fn session(&self) -> &AgentSession {
        &self.session
    }

    /// Get the virtual window size.
    pub fn window_size(&self) -> crate::core::Size {
        self.window_size
    }

    /// Process a single agent request and return the response.
    pub fn process_request(&mut self, request: &AgentRequest) -> AgentResponse {
        // Render to build the UI tree before processing tree-dependent requests
        self.render();

        let (mut response, should_quit) = self.session.process_request(request, &self.ontology);

        // Handle execute_action by dispatching through the model.
        if let AgentRequest::ExecuteAction {
            agent_id,
            action,
            params,
        } = request
        {
            // A widget that carries its own message needs no handler in the
            // application at all; fall back to `execute_action` for the rest.
            let handled = action == "click" && self.dispatch(agent_id);
            let result = if handled {
                serde_json::Value::Null
            } else {
                self.model.execute_action(agent_id, action, params)
            };
            if !result.is_null() {
                response.data = Some(result);
            }
        }

        // Handle batch actions.
        if let AgentRequest::BatchActions { actions } = request {
            let results: Vec<serde_json::Value> = actions
                .iter()
                .map(|entry| {
                    self.model
                        .execute_action(&entry.agent_id, &entry.action, &entry.params)
                })
                .collect();
            if results.iter().any(|value| !value.is_null()) {
                response.data = Some(serde_json::Value::Array(results));
            }
        }

        // Handle injected events
        if let AgentRequest::InjectEvent { event } = request {
            if let Some(ev) = AgentSession::convert_injected_event(event) {
                // A click lands on whatever the hit map says is under it, so a
                // widget with an `action` responds without the application
                // doing coordinate arithmetic in `handle_event`.
                if let crate::event::Event::Mouse(m) = &ev {
                    if m.is_click() {
                        if let Some(id) = self.hit_map.hit_test(m.position).map(str::to_owned) {
                            self.dispatch(&id);
                        }
                    }
                }
                if let Some(msg) = self.model.handle_event(ev) {
                    let cmd = self.model.update(msg);
                    self.process_command(cmd);
                }
            }
        }

        if should_quit {
            self.running = false;
        }

        response
    }

    /// Process a framed request envelope.
    pub fn process_envelope(&mut self, envelope: &RequestEnvelope) -> AgentResponse {
        let mut response = self.process_request(&envelope.request);
        if let Some(ref id) = envelope.id {
            response = response.with_id(id.clone());
        }
        response
    }

    /// Inject a tick event.
    pub fn tick(&mut self) {
        if let Some(msg) = self.model.handle_event(crate::event::Event::Tick) {
            let cmd = self.model.update(msg);
            self.process_command(cmd);
        }
    }

    /// Run the init command for the model.
    pub fn init(&mut self) {
        let cmd = self.model.init();
        self.process_command(cmd);
    }

    /// Compute state-change events by diffing the current ontology tree
    /// against previously seen states. Returns events for subscribed agents.
    pub fn compute_state_diffs(&mut self) -> Vec<super::protocol::AgentEvent> {
        self.session.compute_state_diffs(&self.ontology)
    }

    /// Render the model view to build/refresh the UI tree in the ontology.
    fn render(&mut self) {
        let area =
            crate::core::Rect::new(0.0, 0.0, self.window_size.width, self.window_size.height);
        self.hit_map.clear();
        let mut backend =
            crate::backend::test::TestBackend::new(self.window_size.width, self.window_size.height);
        let mut frame = Frame::new(area, &mut self.hit_map, &mut backend);
        self.model.view(&mut frame);

        self.messages = frame
            .take_messages()
            .into_iter()
            .map(|(id, msg)| (id.into_owned(), msg))
            .collect();

        let nodes = frame.take_nodes();
        if !nodes.is_empty() {
            let mut root =
                crate::ontology::UiNode::new("root", crate::ontology::SemanticRole::Container);
            root.children = nodes;
            self.ontology.set_tree(crate::ontology::UiTree::new(root));
        }
    }

    /// Dispatch the message a widget registered for `agent_id`, if any.
    ///
    /// Returns whether a message was found and applied. This is what lets a
    /// button be driven without the application writing an `execute_action`
    /// arm for it.
    fn dispatch(&mut self, agent_id: &str) -> bool {
        let Some(boxed) = self.messages.remove(agent_id) else {
            return false;
        };
        match boxed.downcast::<M::Msg>() {
            Ok(msg) => {
                let cmd = self.model.update(*msg);
                self.process_command(cmd);
                true
            }
            Err(_) => {
                debug_assert!(
                    false,
                    "widget message for `{agent_id}` was not this model's Msg"
                );
                false
            }
        }
    }

    fn process_command(&mut self, cmd: Command<M::Msg>) {
        match cmd {
            Command::None => {}
            Command::Quit => {
                self.running = false;
            }
            Command::Batch(cmds) => {
                for c in cmds {
                    self.process_command(c);
                }
            }
            Command::Message(msg) => {
                let cmd = self.model.update(msg);
                self.process_command(cmd);
            }
            Command::SetTickRate(_) => {
                // Headless: ignored (caller controls ticking)
            }
            Command::ExportOntology => {
                self.model.register_ontology(&mut self.ontology);
            }
            Command::AgentAction {
                agent_id,
                action,
                params,
            } => {
                log::debug!(
                    "HeadlessDriver: AgentAction {agent_id}.{action}({})",
                    params
                );
            }
            Command::Task(task) => {
                // Spawn the task on a background thread and feed the result message back.
                let msg = task();
                let cmd = self.model.update(msg);
                self.process_command(cmd);
            }
            Command::TaskWithTimeout {
                task,
                timeout,
                on_timeout,
            } => {
                use std::sync::mpsc;
                let (tx, rx) = mpsc::channel();
                std::thread::spawn(move || {
                    let result = task();
                    let _ = tx.send(result);
                });
                let msg = match rx.recv_timeout(timeout) {
                    Ok(result) => result,
                    Err(_) => on_timeout,
                };
                let cmd = self.model.update(msg);
                self.process_command(cmd);
            }
            Command::TaskCancellable { task, token } => {
                let msg = task(token);
                let cmd = self.model.update(msg);
                self.process_command(cmd);
            }
        }
    }
}
