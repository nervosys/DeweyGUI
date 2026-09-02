//! RPC transport: JSON Lines over stdin/stdout.
//!
//! Implements the stdio-based JSON Lines protocol used by AI coding agents
//! to embed and control Dewey applications.

use std::io::{self, Write};
use std::time::Instant;

use super::driver::HeadlessDriver;
use super::protocol::{AgentResponse, RequestEnvelope};
use crate::runtime::Model;

/// Maximum allowed size for a single JSON request line (1 MB).
const MAX_LINE_BYTES: usize = 1_048_576;

/// Maximum requests per second before throttling.
const MAX_REQUESTS_PER_SEC: u32 = 1000;

/// The virtual window a transport lays out against unless told otherwise.
const DEFAULT_WIDTH: f32 = 1280.0;
const DEFAULT_HEIGHT: f32 = 720.0;

/// Runs a Dewey application over stdin/stdout JSON Lines protocol.
///
/// A thin frame around [`HeadlessDriver`]: this type owns the line reading,
/// the rate limit and the size cap, and the driver owns what a request means.
/// It used to own both, and the copy fell behind — an `execute_action` arriving
/// here was turned into a `Command::AgentAction` that the command loop logged
/// and discarded, so an agent over stdio could read the interface and change
/// nothing.
pub struct RpcTransport<M: Model + 'static> {
    driver: HeadlessDriver<M>,
}

impl<M: Model + 'static> RpcTransport<M> {
    /// Create a new RPC transport with the given model.
    pub fn new(model: M) -> Self {
        Self {
            driver: HeadlessDriver::new(model, DEFAULT_WIDTH, DEFAULT_HEIGHT),
        }
    }

    /// Create a transport whose virtual window is a given size.
    ///
    /// Bounds in the UI tree are laid out against this, so an agent that cares
    /// where things are should set it to the window the application expects.
    #[must_use]
    pub fn with_window(model: M, width: f32, height: f32) -> Self {
        Self {
            driver: HeadlessDriver::new(model, width, height),
        }
    }

    /// Run the RPC loop, reading from stdin and writing to stdout.
    pub fn run(mut self) -> io::Result<M> {
        self.driver.init();

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

            let json = self.driver.process_envelope_json(&envelope);
            writeln!(stdout, "{json}")?;

            // A subscribed agent is told what changed, on the same stream. The
            // protocol has always accepted `subscribe`; until now nothing ever
            // sent anything back.
            for event in self.driver.drain_events_json() {
                writeln!(stdout, "{event}")?;
            }
            stdout.flush()?;

            if !self.driver.is_running() {
                break;
            }
        }

        Ok(self.driver.into_model())
    }
}
