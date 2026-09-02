//! JSON-based agent protocol messages.
//!
//! Defines the request/response/event types that AI agents use to communicate
//! with a Dewey application. Same protocol as Louie (TUI), enabling
//! cross-framework agent compatibility.

use serde::{Deserialize, Serialize};

/// The current protocol version.
pub const PROTOCOL_VERSION: u32 = 2;

/// The minimum protocol version that this server can interoperate with.
pub const MIN_PROTOCOL_VERSION: u32 = 1;

/// All capabilities that this server implementation supports.
pub const SERVER_CAPABILITIES: &[&str] = &[
    "state_diffs",
    "batch_actions",
    "screenshot",
    "ws_transport",
    "protocol_v2",
];

/// A request from an AI agent to the Dewey application.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AgentRequest {
    /// Query the ontology type catalog.
    #[serde(rename = "query_ontology")]
    QueryOntology {
        #[serde(default)]
        query: Option<String>,
        #[serde(default)]
        role: Option<String>,
    },

    /// Get the schema for a specific widget type.
    #[serde(rename = "get_schema")]
    GetSchema { widget_type: String },

    /// Check the rendered interface for structural faults.
    ///
    /// Returns a list of [`Diagnostic`](crate::ontology::Diagnostic)s: widgets
    /// that are unclickable, duplicated ids, zero-size or offscreen bounds.
    /// This is how an agent confirms the interface it just scaffolded is
    /// actually operable, rather than merely rendering.
    ///
    /// Pass `strict` when the application is meant to be driven unattended: it
    /// promotes warnings to errors and additionally reports a widget that
    /// publishes actions with nothing wired to any of them.
    #[serde(rename = "validate")]
    Validate {
        #[serde(default)]
        strict: bool,
    },

    /// Get the current UI tree snapshot.
    ///
    /// Pass `since` with the `version` from a previous reply to be told
    /// `unchanged` instead of receiving the tree again. Rebuilding and
    /// serialising the tree is the most expensive thing an agent can ask for,
    /// and an agent polling a screen that has not moved asks for it
    /// repeatedly.
    ///
    /// Pass `viewport` to be sent only the widgets whose bounds intersect a
    /// rectangle. The tree otherwise describes every widget in the interface,
    /// including the ones scrolled out of sight, so for a long list it is
    /// larger and slower than a screenshot of the same application — a
    /// screenshot only ever shows one window's worth.
    #[serde(rename = "get_tree")]
    GetTree {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        since: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        viewport: Option<Viewport>,
    },

    /// Get the state of a specific widget by its agent ID.
    #[serde(rename = "get_state")]
    GetState { agent_id: String },

    /// Execute an action on a widget.
    #[serde(rename = "execute_action")]
    ExecuteAction {
        agent_id: String,
        action: String,
        #[serde(default)]
        params: serde_json::Value,
    },

    /// Inject an event into the application.
    #[serde(rename = "inject_event")]
    InjectEvent { event: InjectedEvent },

    /// Subscribe to application events.
    #[serde(rename = "subscribe")]
    Subscribe { events: Vec<String> },

    /// Unsubscribe from application events.
    #[serde(rename = "unsubscribe")]
    Unsubscribe { events: Vec<String> },

    /// Ping the application (keepalive / connection testing).
    #[serde(rename = "ping")]
    Ping,

    /// Request the application to quit.
    #[serde(rename = "quit")]
    Quit,

    /// Take a screenshot of the current frame.
    #[serde(rename = "screenshot")]
    Screenshot {
        /// Output format: "png", "raw", etc.
        #[serde(default = "default_format")]
        format: String,
    },

    /// Execute multiple actions atomically in a single request.
    #[serde(rename = "batch_actions")]
    BatchActions { actions: Vec<BatchActionEntry> },

    /// Negotiate protocol version and capabilities.
    #[serde(rename = "negotiate")]
    Negotiate {
        /// The protocol version the client supports.
        client_version: u32,
        /// Optional list of capabilities the client requests.
        #[serde(default)]
        capabilities: Vec<String>,
    },
}

/// A single entry in a batch action request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchActionEntry {
    pub agent_id: String,
    pub action: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

fn default_format() -> String {
    "png".into()
}

/// An event injected by an agent into the application.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum InjectedEvent {
    /// A key press event.
    #[serde(rename = "key")]
    Key {
        code: String,
        #[serde(default)]
        modifiers: Vec<String>,
    },
    /// A mouse click event (GUI coordinates).
    #[serde(rename = "mouse_click")]
    MouseClick { x: f32, y: f32, button: String },
    /// A mouse move event.
    #[serde(rename = "mouse_move")]
    MouseMove { x: f32, y: f32 },
    /// A mouse scroll event.
    #[serde(rename = "mouse_scroll")]
    MouseScroll {
        x: f32,
        y: f32,
        delta_x: f32,
        delta_y: f32,
    },
    /// A text input / paste event.
    #[serde(rename = "text_input")]
    TextInput { text: String },
    /// A window resize event.
    #[serde(rename = "resize")]
    Resize { width: f32, height: f32 },
}

/// A rectangle limiting which widgets a tree reply describes.
///
/// Matches the region of the interface an agent can currently see, so a
/// 10,000-row list costs one window's worth of tree rather than all of it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Viewport {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Viewport {
    /// Whether a node's bounds fall partly inside this rectangle.
    ///
    /// Touching edges do not count as visible: a widget ending exactly at the
    /// top of the viewport is above it.
    #[must_use]
    pub fn shows(&self, bounds: &crate::ontology::NodeBounds) -> bool {
        bounds.x < self.x + self.width
            && bounds.x + bounds.width > self.x
            && bounds.y < self.y + self.height
            && bounds.y + bounds.height > self.y
    }
}

/// A response from the Dewey application to an agent request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl AgentResponse {
    pub fn ok(data: serde_json::Value) -> Self {
        Self {
            success: true,
            id: None,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(message: impl Into<String>) -> Self {
        Self {
            success: false,
            id: None,
            data: None,
            error: Some(message.into()),
        }
    }

    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }
}

/// An event streamed from the application to a subscribed agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AgentEvent {
    #[serde(rename = "state_changed")]
    StateChanged {
        agent_id: String,
        state: serde_json::Value,
    },

    #[serde(rename = "action_result")]
    ActionResult {
        agent_id: String,
        action: String,
        result: serde_json::Value,
    },

    #[serde(rename = "render_update")]
    RenderUpdate { tree: serde_json::Value },

    #[serde(rename = "app_quit")]
    AppQuit,

    #[serde(rename = "pong")]
    Pong,

    #[serde(rename = "error")]
    Error { message: String },
}

/// A framed message with an optional ID for request/response correlation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestEnvelope {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub request: AgentRequest,
}
