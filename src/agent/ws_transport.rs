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

use super::driver::HeadlessDriver;
use super::protocol::{AgentResponse, RequestEnvelope};
use crate::runtime::Model;

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
/// A thin frame around [`HeadlessDriver`]: this type owns the socket, and the
/// driver owns what a request means. It used to own both, and the copy fell
/// behind — an `execute_action` arriving here became a `Command::AgentAction`
/// that the command loop logged and discarded, so an agent over a WebSocket
/// could read the interface and change nothing.
pub struct WsTransport<M: Model + 'static> {
    driver: HeadlessDriver<M>,
    bind_addr: String,
}

impl<M: Model + 'static> WsTransport<M> {
    /// Create a new WebSocket transport bound to the given address.
    pub fn new(model: M, bind_addr: impl Into<String>) -> Self {
        Self::with_window(model, bind_addr, 1280.0, 720.0)
    }

    /// Create a transport whose virtual window is a given size.
    ///
    /// Bounds in the UI tree are laid out against this, so an agent that cares
    /// where things are should set it to the window the application expects.
    #[must_use]
    pub fn with_window(model: M, bind_addr: impl Into<String>, width: f32, height: f32) -> Self {
        Self {
            driver: HeadlessDriver::new(model, width, height),
            bind_addr: bind_addr.into(),
        }
    }

    /// Run the WebSocket server, accepting one connection and processing messages.
    pub fn run(mut self) -> io::Result<M> {
        self.driver.init();

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

            let json = self.driver.process_envelope_json(&envelope);
            let _ = websocket.write(Message::Text(json));

            for event in self.driver.drain_events_json() {
                let _ = websocket.write(Message::Text(event));
            }

            if !self.driver.is_running() {
                break;
            }
        }

        let _ = websocket.close(None);
        Ok(self.driver.into_model())
    }
}
