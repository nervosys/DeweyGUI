//! WebSocket transport: JSON protocol over WebSocket.
//!
//! Alternative to the stdin/stdout [`RpcTransport`](super::RpcTransport) —
//! runs the same JSON-based agent protocol over a WebSocket connection,
//! enabling remote agent control over the network.

use std::io;
use std::net::TcpListener;
use std::time::Instant;

use tungstenite::Message;
use tungstenite::accept;

use super::protocol::{AgentRequest, AgentResponse, RequestEnvelope};
use super::session::AgentSession;
use crate::ontology::OntologyRegistry;
use crate::runtime::{Command, Model};

/// Maximum allowed size for a single JSON message (1 MB).
const MAX_MESSAGE_BYTES: usize = 1_048_576;

/// Maximum requests per second before throttling.
const MAX_REQUESTS_PER_SEC: u32 = 1000;

/// Runs a Dewey application over a WebSocket connection.
///
/// Binds a TCP listener and accepts one client at a time,
/// processing the same JSON protocol as [`RpcTransport`](super::RpcTransport).
///
/// # Example
///
/// ```rust,no_run
/// use dewey::agent::WsTransport;
/// # struct MyApp;
/// # impl dewey::runtime::Model for MyApp {
/// #     type Msg = ();
/// #     fn update(&mut self, _: ()) -> dewey::runtime::Command<()> { dewey::runtime::Command::None }
/// #     fn view(&self, _: &mut dewey::runtime::Frame<'_>) {}
/// #     fn handle_event(&self, _: dewey::event::Event) -> Option<()> { None }
/// # }
///
/// let transport = WsTransport::new(MyApp, "127.0.0.1:9001");
/// transport.run().unwrap();
/// ```
pub struct WsTransport<M: Model + 'static> {
    model: M,
    session: AgentSession,
    ontology: OntologyRegistry,
    running: bool,
    bind_addr: String,
}

impl<M: Model + 'static> WsTransport<M> {
    /// Create a new WebSocket transport bound to the given address.
    pub fn new(model: M, bind_addr: impl Into<String>) -> Self {
        let mut ontology = OntologyRegistry::new();
        model.register_ontology(&mut ontology);

        Self {
            model,
            session: AgentSession::new(),
            ontology,
            running: true,
            bind_addr: bind_addr.into(),
        }
    }

    /// Run the WebSocket server, accepting one connection and processing messages.
    pub fn run(mut self) -> io::Result<M> {
        let init_cmd = self.model.init();
        self.process_command(init_cmd);
        self.model.register_ontology(&mut self.ontology);

        let listener = TcpListener::bind(&self.bind_addr)?;
        log::info!("WsTransport listening on {}", self.bind_addr);

        // Accept one connection
        let (stream, peer) = listener.accept()?;
        log::info!("WsTransport accepted connection from {peer}");

        let mut websocket = accept(stream)
            .map_err(|e| io::Error::new(io::ErrorKind::ConnectionAborted, e.to_string()))?;

        let mut window_start = Instant::now();
        let mut request_count: u32 = 0;

        loop {
            let msg = match websocket.read() {
                Ok(msg) => msg,
                Err(tungstenite::Error::ConnectionClosed) => break,
                Err(tungstenite::Error::Protocol(..)) => break,
                Err(e) => {
                    log::error!("WsTransport read error: {e}");
                    break;
                }
            };

            let text = match msg {
                Message::Text(t) => t,
                Message::Close(_) => break,
                Message::Ping(data) => {
                    let _ = websocket.write(Message::Pong(data));
                    continue;
                }
                _ => continue,
            };

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
                let _ = websocket.write(Message::Text(json));
                continue;
            }

            // Reject oversized messages
            if text.len() > MAX_MESSAGE_BYTES {
                let resp = AgentResponse::err(format!(
                    "Message too large ({} bytes, max {MAX_MESSAGE_BYTES})",
                    text.len(),
                ));
                let json = serde_json::to_string(&resp).unwrap_or_default();
                let _ = websocket.write(Message::Text(json));
                continue;
            }

            let envelope: RequestEnvelope = match serde_json::from_str(&text) {
                Ok(e) => e,
                Err(err) => {
                    let resp = AgentResponse::err(format!("Invalid JSON: {err}"));
                    let json = serde_json::to_string(&resp).unwrap_or_default();
                    let _ = websocket.write(Message::Text(json));
                    continue;
                }
            };

            let (mut response, should_quit) = self
                .session
                .process_request(&envelope.request, &self.ontology);

            // Handle side effects
            if let AgentRequest::ExecuteAction {
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

            if let AgentRequest::InjectEvent { event } = &envelope.request {
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
            let _ = websocket.write(Message::Text(json));

            if should_quit || !self.running {
                break;
            }
        }

        let _ = websocket.close(None);
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
                log::debug!("WsTransport: AgentAction {agent_id}.{action}({params})");
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
