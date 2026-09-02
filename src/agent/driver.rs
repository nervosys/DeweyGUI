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
    /// Changes registered by widgets during the last render.
    handlers: crate::runtime::Handlers<M>,
    /// Interactive widgets that rendered without an id in the last frame.
    unaddressable: Vec<&'static str>,
    /// Bumped whenever a request could have changed the model. An agent that
    /// passes the version it last saw is told `unchanged` rather than being
    /// sent an identical tree.
    version: u64,
}

impl<M: Model + 'static> HeadlessDriver<M> {
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
            handlers: crate::runtime::Handlers::default(),
            unaddressable: Vec::new(),
            version: 0,
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
        // Only the requests that actually read the tree pay for building it.
        // Answering `get_schema` or `ping` used to re-render the whole
        // application first.
        // An agent polling a screen that has not moved gets told so, without
        // the application being rendered or the tree serialised.
        if let AgentRequest::GetTree { since: Some(seen) } = request {
            if *seen == self.version {
                return AgentResponse::ok(serde_json::json!({
                    "unchanged": true,
                    "version": self.version,
                }));
            }
        }

        if Self::needs_tree(request) {
            self.render();
        }

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
            let handled = self.dispatch(agent_id, action, params);
            let result = if handled {
                // The session answers from the UI tree, and a widget can carry
                // a handler without appearing in it: a closed `CommandPalette`
                // renders nothing but still advertises `open`. The change
                // reached the model, so the call succeeded.
                response.success = true;
                response.error = None;
                serde_json::Value::Null
            } else {
                self.model.execute_action(agent_id, action, params)
            };
            if !result.is_null() {
                response.data = Some(result);
            }
        }

        // `screenshot format=text` returns the golden-comparable snapshot
        // rather than a JSON tree.
        if let AgentRequest::Screenshot { format } = request {
            if format == "text" {
                let snap = self
                    .ontology
                    .tree()
                    .map(crate::ontology::UiTree::snapshot)
                    .unwrap_or_default();
                response = AgentResponse::ok(serde_json::json!({
                    "format": "text",
                    "kind": "snapshot",
                    "snapshot": snap,
                }));
            }
        }

        // Structural check: answered here because it needs the frame's own
        // record of what rendered, which the session cannot see.
        if matches!(request, AgentRequest::Validate) {
            let findings = self.validate();
            let errors = findings
                .iter()
                .filter(|d| d.severity == crate::ontology::Severity::Error)
                .count();
            response = AgentResponse::ok(serde_json::json!({
                "ok": errors == 0,
                "errors": errors,
                "diagnostics": findings,
            }));
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
                            self.dispatch_primary(&id);
                        }
                    }
                }
                if let Some(msg) = self.model.handle_event(ev) {
                    let cmd = self.model.update(msg);
                    self.process_command(cmd);
                }
            }
        }

        // Stamp the version onto a tree reply so the agent can ask
        // conditionally next time.
        if matches!(request, AgentRequest::GetTree { .. }) {
            if let Some(data) = response.data.as_mut() {
                if let Some(obj) = data.as_object_mut() {
                    obj.insert("version".into(), serde_json::json!(self.version));
                }
            }
        }

        // Anything that can mutate the model invalidates a cached tree.
        // Over-counting only costs a needless refresh; under-counting would
        // hand an agent a stale screen, so the set is deliberately broad.
        if Self::may_mutate(request) {
            self.version = self.version.wrapping_add(1);
        }

        if should_quit {
            self.running = false;
        }

        response
    }

    /// Whether this request could change what the interface shows.
    fn may_mutate(request: &AgentRequest) -> bool {
        matches!(
            request,
            AgentRequest::ExecuteAction { .. }
                | AgentRequest::BatchActions { .. }
                | AgentRequest::InjectEvent { .. }
        )
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

        self.unaddressable = frame.take_unaddressable();
        self.handlers = crate::runtime::Handlers::take_from(&mut frame);

        let nodes = frame.take_nodes();
        if !nodes.is_empty() {
            let mut root =
                crate::ontology::UiNode::new("root", crate::ontology::SemanticRole::Container);
            root.children = nodes;
            self.ontology.set_tree(crate::ontology::UiTree::new(root));
        }
    }

    /// Check the rendered interface for structural faults.
    ///
    /// Renders once, then reports widgets that cannot be clicked or addressed,
    /// duplicated ids, and bounds that are empty or offscreen. An agent can
    /// call this after scaffolding an interface to confirm it is operable
    /// without opening a window.
    pub fn validate(&mut self) -> Vec<crate::ontology::Diagnostic> {
        self.render();
        let tree = self.ontology.tree().cloned().unwrap_or_else(|| {
            crate::ontology::UiTree::new(crate::ontology::UiNode::new(
                "root",
                crate::ontology::SemanticRole::Container,
            ))
        });
        let handlers = self.handlers.list();
        crate::ontology::diagnostics::check(
            &tree,
            &self.unaddressable,
            self.window_size,
            &handlers,
            &self.ontology,
        )
    }

    /// A stable text rendering of the interface, for golden comparison.
    ///
    /// See [`UiTree::snapshot`](crate::ontology::UiTree::snapshot). Renders
    /// first, so the result reflects current model state.
    pub fn snapshot(&mut self) -> String {
        self.render();
        self.ontology
            .tree()
            .map(crate::ontology::UiTree::snapshot)
            .unwrap_or_default()
    }

    /// Whether answering this request requires a freshly rendered UI tree.
    ///
    /// Type catalogue queries (`query_ontology`, `get_schema`), liveness checks
    /// and session bookkeeping are answered from the registry alone.
    fn needs_tree(request: &AgentRequest) -> bool {
        matches!(
            request,
            AgentRequest::GetTree { .. }
                | AgentRequest::GetState { .. }
                | AgentRequest::ExecuteAction { .. }
                | AgentRequest::BatchActions { .. }
                | AgentRequest::InjectEvent { .. }
                | AgentRequest::Screenshot { .. }
                | AgentRequest::Validate
        )
    }

    /// Dispatch the message a widget registered for `agent_id`, if any.
    ///
    /// Returns whether a message was found and applied. This is what lets a
    /// button be driven without the application writing an `execute_action`
    /// arm for it.
    /// Fire whatever action the widget registered, as a mouse click does.
    ///
    /// A click is physical: it means "activate this widget", not any
    /// particular action name. A `Checkbox` advertises `toggle`, a `Button`
    /// advertises `click`, and pressing either must work.
    fn dispatch_primary(&mut self, agent_id: &str) -> bool {
        let Some(action) = self.handlers.primary_action(agent_id) else {
            return false;
        };
        self.dispatch(agent_id, action, &serde_json::Value::Null)
    }

    fn dispatch(&mut self, agent_id: &str, action: &str, params: &serde_json::Value) -> bool {
        let Some(cmd) = self
            .handlers
            .apply(agent_id, action, params, &mut self.model)
        else {
            return false;
        };
        self.process_command(cmd);
        true
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
