//! RPC transport: JSON Lines over stdin/stdout.
//!
//! Implements the stdio-based JSON Lines protocol used by AI coding agents
//! to embed and control Dewey applications.

use std::io::{self, Write};
use std::time::Instant;

use super::protocol::{AgentResponse, RequestEnvelope};
use super::session::AgentSession;
use crate::ontology::OntologyRegistry;
use crate::runtime::{Command, Model};

/// Maximum allowed size for a single JSON request line (1 MB).
const MAX_LINE_BYTES: usize = 1_048_576;

/// Maximum requests per second before throttling.
const MAX_REQUESTS_PER_SEC: u32 = 1000;

/// Runs a Dewey application over stdin/stdout JSON Lines protocol.
pub struct RpcTransport<M: Model> {
    model: M,
    session: AgentSession,
    ontology: OntologyRegistry,
    running: bool,
}

impl<M: Model> RpcTransport<M> {
    /// Create a new RPC transport with the given model.
    pub fn new(model: M) -> Self {
        let mut ontology = OntologyRegistry::new();
        model.register_ontology(&mut ontology);

        Self {
            model,
            session: AgentSession::new(),
            ontology,
            running: true,
        }
    }

    /// Run the RPC loop, reading from stdin and writing to stdout.
    pub fn run(mut self) -> io::Result<M> {
        // Initialize
        let init_cmd = self.model.init();
        self.process_command(init_cmd);
        self.model.register_ontology(&mut self.ontology);

        let stdin = io::stdin();
        let mut stdout = io::stdout();
        let mut reader = stdin.lock();

        let mut window_start = Instant::now();
        let mut request_count: u32 = 0;

        while let Some((raw, oversized)) = super::read_capped_line(&mut reader, MAX_LINE_BYTES)? {
            let line = String::from_utf8_lossy(&raw);
            let trimmed = line.trim();
            if !oversized && trimmed.is_empty() {
                continue;
            }

            // Rate limiting
            let elapsed = window_start.elapsed();
            if elapsed.as_secs() >= 1 {
                window_start = Instant::now();
                request_count = 0;
            }
            request_count += 1;
            if request_count > MAX_REQUESTS_PER_SEC {
                let resp = AgentResponse::err(format!(
                    "Rate limit exceeded ({MAX_REQUESTS_PER_SEC} req/s)"
                ));
                let json = serde_json::to_string(&resp).unwrap_or_default();
                writeln!(stdout, "{json}")?;
                stdout.flush()?;
                continue;
            }

            // Reject oversized requests. The reader caps buffering at
            // MAX_LINE_BYTES, so an unbounded line can never exhaust memory
            // before this guard fires.
            if oversized {
                let resp =
                    AgentResponse::err(format!("Request too large (max {MAX_LINE_BYTES} bytes)"));
                let json = serde_json::to_string(&resp).unwrap_or_default();
                writeln!(stdout, "{json}")?;
                stdout.flush()?;
                continue;
            }

            let envelope: RequestEnvelope = match serde_json::from_str(trimmed) {
                Ok(e) => e,
                Err(err) => {
                    let resp = AgentResponse::err(format!("Invalid JSON: {err}"));
                    let json = serde_json::to_string(&resp).unwrap_or_default();
                    writeln!(stdout, "{json}")?;
                    stdout.flush()?;
                    continue;
                }
            };

            let (mut response, should_quit) = self
                .session
                .process_request(&envelope.request, &self.ontology);

            // Handle side effects
            if let super::protocol::AgentRequest::ExecuteAction {
                agent_id,
                action,
                params,
            } = &envelope.request
            {
                let cmd = Command::AgentAction {
                    agent_id: agent_id.clone(),
                    action: action.clone(),
                    params: params.clone(),
                };
                self.process_command(cmd);
            }

            if let super::protocol::AgentRequest::InjectEvent { event } = &envelope.request {
                if let Some(ev) = AgentSession::convert_injected_event(event) {
                    if let Some(msg) = self.model.handle_event(ev) {
                        let cmd = self.model.update(msg);
                        self.process_command(cmd);
                    }
                }
            }

            if let Some(ref id) = envelope.id {
                response = response.with_id(id.clone());
            }

            let json = serde_json::to_string(&response).unwrap_or_default();
            writeln!(stdout, "{json}")?;
            stdout.flush()?;

            if should_quit {
                break;
            }

            if !self.running {
                break;
            }
        }

        Ok(self.model)
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
            Command::SetTickRate(_) => {}
            Command::ExportOntology => {
                self.model.register_ontology(&mut self.ontology);
            }
            Command::AgentAction {
                agent_id,
                action,
                params,
            } => {
                log::debug!("RpcTransport: AgentAction {agent_id}.{action}({params})");
            }
            Command::Task(task) => {
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
