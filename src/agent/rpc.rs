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

/// How long a windowed application has to answer before it is called stopped.
///
/// A frame at 60 Hz is 17 ms, so this is three orders of magnitude of slack;
/// it exists so that a wedged frame loop is reported rather than hanging the
/// transport for the life of the process.
const ANSWER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

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
        serve_stdio(&mut self.driver)?;
        Ok(self.driver.into_model())
    }
}

/// Whatever can answer an agent request.
///
/// The reading, the line cap and the rate limit are the same whether the
/// application is headless or has a window open; only the answering differs.
/// Splitting them here is what lets a windowed application be driven by the
/// same protocol, rather than a second loop being written for it — the last
/// two copies of this loop both answered `execute_action` with a `log::debug!`.
pub trait RequestSink {
    /// Answer one envelope, already parsed, as a line of JSON.
    fn answer(&mut self, envelope: &RequestEnvelope) -> String;

    /// Any events a subscribed agent is owed, as lines of JSON.
    fn drain_events(&mut self) -> Vec<String>;

    /// Whether the application is still running.
    fn is_running(&self) -> bool;
}

impl<M: Model + 'static> RequestSink for HeadlessDriver<M> {
    fn answer(&mut self, envelope: &RequestEnvelope) -> String {
        self.process_envelope_json(envelope)
    }

    fn drain_events(&mut self) -> Vec<String> {
        self.drain_events_json()
    }

    fn is_running(&self) -> bool {
        HeadlessDriver::is_running(self)
    }
}

/// Read JSON Lines requests from stdin and write the answers to stdout.
///
/// Returns when stdin closes or the application stops.
pub fn serve_stdio(sink: &mut impl RequestSink) -> io::Result<()> {
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

        let json = sink.answer(&envelope);
        writeln!(stdout, "{json}")?;

        // A subscribed agent is told what changed, on the same stream. The
        // protocol has always accepted `subscribe`; until now nothing ever
        // sent anything back.
        for event in sink.drain_events() {
            writeln!(stdout, "{event}")?;
        }
        stdout.flush()?;

        if !sink.is_running() {
            break;
        }
    }

    Ok(())
}

/// One request waiting for the frame loop to answer it.
pub struct AgentJob {
    envelope: RequestEnvelope,
    reply: std::sync::mpsc::Sender<Vec<String>>,
}

/// A [`RequestSink`] that hands each request to another thread and waits.
///
/// This is how a windowed application is agent-driven. `Program::run` owns the
/// model on the thread that owns the window, so a transport cannot also own it;
/// the transport thread sends the parsed envelope here and blocks until the
/// frame loop answers. Latency is therefore up to one frame, which for a
/// protocol whose replies are measured in microseconds is the whole cost.
///
/// Before this existed, an application was agent-driven or windowed and never
/// both — the framework's premise held only if you picked one.
pub struct ChannelSink {
    jobs: std::sync::mpsc::Sender<AgentJob>,
    /// Wake the frame loop, so a request is answered by an idle window.
    ///
    /// Without this the transport blocks forever on an application that has
    /// nothing to redraw: a windowed backend runs `update` when something asks
    /// it to, and a request arriving on another thread is not something it
    /// knows to ask about.
    wake: Box<dyn Fn() + Send>,
    /// Subscription events that came back with the last reply.
    pending: Vec<String>,
    running: bool,
}

impl ChannelSink {
    /// Create a sink that sends to `jobs` and calls `wake` after each send.
    #[must_use]
    pub fn new(jobs: std::sync::mpsc::Sender<AgentJob>, wake: Box<dyn Fn() + Send>) -> Self {
        Self {
            jobs,
            wake,
            pending: Vec::new(),
            running: true,
        }
    }
}

impl RequestSink for ChannelSink {
    fn answer(&mut self, envelope: &RequestEnvelope) -> String {
        let (reply, answers) = std::sync::mpsc::channel();
        let job = AgentJob {
            envelope: envelope.clone(),
            reply,
        };
        if self.jobs.send(job).is_err() {
            // The window has gone. Say so rather than hanging.
            self.running = false;
            return serde_json::to_string(&AgentResponse::err("application has stopped"))
                .unwrap_or_default();
        }
        (self.wake)();
        // Bounded, because an agent that is never answered is worse than one
        // that is told the application is not responding: a wedged frame loop
        // would otherwise hang the transport for as long as the process lives.
        match answers.recv_timeout(ANSWER_TIMEOUT) {
            Ok(mut lines) if !lines.is_empty() => {
                let first = lines.remove(0);
                self.pending = lines;
                first
            }
            Ok(_) => serde_json::to_string(&AgentResponse::err("application sent no answer"))
                .unwrap_or_default(),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                serde_json::to_string(&AgentResponse::err("application did not answer within 5s"))
                    .unwrap_or_default()
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                self.running = false;
                serde_json::to_string(&AgentResponse::err("application has stopped"))
                    .unwrap_or_default()
            }
        }
    }

    fn drain_events(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending)
    }

    fn is_running(&self) -> bool {
        self.running
    }
}

/// Answer one job from the frame loop, using the driver that owns the model.
///
/// The reply carries the response first and any subscription events after it,
/// which is the order [`serve_stdio`] writes them in.
pub fn answer_job<M: Model + 'static>(job: AgentJob, driver: &mut HeadlessDriver<M>) {
    let mut lines = vec![driver.process_envelope_json(&job.envelope)];
    lines.extend(driver.drain_events_json());
    let _ = job.reply.send(lines);
}
