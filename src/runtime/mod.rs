//! Elm-architecture runtime for Dewey applications.
//!
//! - **Model**: Application state
//! - **Message**: Events that update state
//! - **Update**: Pure function `(Model, Msg) -> (Model, Command)`
//! - **View**: Pure function `Model -> UI description`

use std::time::Duration;

use crate::core::Rect;
use crate::ontology::OntologyRegistry;

/// A token that can be checked to determine if a task should be cancelled.
#[derive(Clone)]
pub struct CancellationToken {
    cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl CancellationToken {
    /// Create a new cancellation token.
    pub fn new() -> Self {
        Self {
            cancelled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Check if cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Request cancellation.
    pub fn cancel(&self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

/// A command returned from [`Model::update`] to request side effects.
pub enum Command<Msg> {
    /// No operation.
    None,
    /// Quit the application.
    Quit,
    /// Execute multiple commands.
    Batch(Vec<Command<Msg>>),
    /// Produce a message asynchronously after the current update.
    Message(Msg),
    /// Set the tick interval for animation / periodic updates.
    SetTickRate(Duration),
    /// Request that the agent ontology registry be exported to JSON.
    ExportOntology,
    /// Execute an agent action on a widget identified by agent_id.
    AgentAction {
        agent_id: String,
        action: String,
        params: serde_json::Value,
    },
    /// Show or hide the window without ending the program.
    ///
    /// The defining gesture of a tray application is "click the icon, toggle
    /// the window", and until this existed there was no way to say it: the
    /// only window operation a `Model` could reach was [`Command::Quit`], so
    /// closing to the tray and quitting were the same thing.
    SetWindowVisible(bool),
    /// Bring the window to the front and give it keyboard focus.
    FocusWindow,
    /// Minimise the window to the taskbar or dock.
    MinimiseWindow,
    /// Move the window's top-left corner to a screen position.
    SetWindowPosition { x: f32, y: f32 },
    /// Resize the window.
    SetWindowSize { width: f32, height: f32 },
    /// Keep the window above others, or stop doing so.
    SetAlwaysOnTop(bool),
    /// Enter or leave fullscreen.
    SetFullscreen(bool),
    /// Set the window title.
    SetWindowTitle(String),
    /// Spawn an asynchronous task that eventually produces a message.
    Task(Box<dyn FnOnce() -> Msg + Send>),
    /// Spawn an async task with a timeout. If the task doesn't complete
    /// within the given duration, the timeout message is delivered instead.
    TaskWithTimeout {
        task: Box<dyn FnOnce() -> Msg + Send>,
        timeout: Duration,
        on_timeout: Msg,
    },
    /// Spawn a cancellable async task. The closure receives a [`CancellationToken`]
    /// that it can poll to exit early.
    TaskCancellable {
        task: Box<dyn FnOnce(CancellationToken) -> Msg + Send>,
        token: CancellationToken,
    },
}

impl<Msg: std::fmt::Debug> std::fmt::Debug for Command<Msg> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::Quit => write!(f, "Quit"),
            Self::Batch(cmds) => f.debug_tuple("Batch").field(cmds).finish(),
            Self::Message(msg) => f.debug_tuple("Message").field(msg).finish(),
            Self::SetTickRate(d) => f.debug_tuple("SetTickRate").field(d).finish(),
            Self::ExportOntology => write!(f, "ExportOntology"),
            Self::SetWindowVisible(v) => f.debug_tuple("SetWindowVisible").field(v).finish(),
            Self::FocusWindow => write!(f, "FocusWindow"),
            Self::MinimiseWindow => write!(f, "MinimiseWindow"),
            Self::SetWindowPosition { x, y } => f
                .debug_struct("SetWindowPosition")
                .field("x", x)
                .field("y", y)
                .finish(),
            Self::SetWindowSize { width, height } => f
                .debug_struct("SetWindowSize")
                .field("width", width)
                .field("height", height)
                .finish(),
            Self::SetAlwaysOnTop(v) => f.debug_tuple("SetAlwaysOnTop").field(v).finish(),
            Self::SetFullscreen(v) => f.debug_tuple("SetFullscreen").field(v).finish(),
            Self::SetWindowTitle(t) => f.debug_tuple("SetWindowTitle").field(t).finish(),
            Self::AgentAction {
                agent_id,
                action,
                params,
            } => f
                .debug_struct("AgentAction")
                .field("agent_id", agent_id)
                .field("action", action)
                .field("params", params)
                .finish(),
            Self::Task(_) => write!(f, "Task(<fn>)"),
            Self::TaskWithTimeout { timeout, .. } => {
                write!(f, "TaskWithTimeout({}ms)", timeout.as_millis())
            }
            Self::TaskCancellable { .. } => write!(f, "TaskCancellable(<fn>)"),
        }
    }
}

/// The core trait for application models (Elm Architecture).
pub trait Model: Sized {
    /// The message type for this application.
    type Msg: Send + 'static;

    /// Handle a message and return an updated model plus optional command.
    fn update(&mut self, msg: Self::Msg) -> Command<Self::Msg>;

    /// Render the model into the GUI frame.
    ///
    /// Called each frame by the runtime. Widgets render through the
    /// abstract `Painter` available via `frame.painter()`.
    fn view(&self, frame: &mut Frame<'_>);

    /// Convert a raw event into an application message.
    /// Return `None` to ignore the event.
    /// Defaults to ignoring every event, which is what an application driven
    /// entirely by widget interaction and agent actions wants. Override to
    /// handle raw keyboard or mouse input.
    fn handle_event(&self, _event: crate::event::Event) -> Option<Self::Msg> {
        None
    }

    /// Called once at startup. Return an initial command.
    fn init(&self) -> Command<Self::Msg> {
        Command::None
    }

    /// Called when the agent ontology is exported. Override to customize.
    fn register_ontology(&self, _registry: &mut OntologyRegistry) {}

    /// Called once after plugins have initialised, with what they contributed.
    ///
    /// A plugin's ontology registrations reach the agent protocol on their
    /// own. Its theme and message-catalogue contributions have nowhere else to
    /// go: widgets take a [`Style`](crate::core::Style) rather than reading an
    /// ambient theme, so the application is what consumes them. They were
    /// previously dropped as soon as `init` returned.
    fn plugins_ready(&mut self, _contributions: &crate::plugin::PluginContributions) {}

    /// Perform an agent-requested action and return its result.
    ///
    /// The ontology lets an agent *discover* an application; this is what lets
    /// it *act on* one. Without it, `execute_action` reaches only behaviour
    /// expressible as an injected click — which limits an agent to what a mouse
    /// can reach and leaves an application's real operations (open this file,
    /// search for that, export) unreachable.
    ///
    /// The returned value is sent back to the agent. The default returns null,
    /// meaning "this application defines no actions", so existing
    /// implementations are unaffected.
    ///
    /// Implementations should route to the same code the user interface calls.
    /// An action that exists only for agents is a second implementation waiting
    /// to disagree with the first.
    fn execute_action(
        &mut self,
        _agent_id: &str,
        _action: &str,
        _params: &serde_json::Value,
    ) -> serde_json::Value {
        serde_json::Value::Null
    }

    /// Application title (used as window title).
    fn title(&self) -> &str {
        "Dewey App"
    }
}

/// A rendering frame — abstraction over the GUI backend.
///
/// During `Model::view`, the frame provides methods to draw widgets
/// and manage the UI tree for agent discoverability. All rendering
/// goes through the [`Painter`](crate::paint::Painter) trait.
pub struct Frame<'a> {
    /// The available drawing area.
    pub area: Rect,
    /// The hit map for mouse routing.
    pub hit_map: &'a mut crate::event::HitMap,
    /// The ontology tree being built for this frame.
    ui_nodes: Vec<crate::ontology::UiNode>,
    /// The painter for this frame.
    painter: &'a mut dyn crate::paint::Painter,
    /// Whether anything is listening to the ontology this frame.
    ontology: bool,
    /// Messages widgets want dispatched when they are activated.
    messages: Vec<(
        std::borrow::Cow<'static, str>,
        &'static str,
        Box<dyn std::any::Any + Send>,
    )>,
    /// Interactive widgets that rendered without an id, and so cannot be
    /// clicked or addressed. Collected here because such a widget never
    /// reaches the UI tree to be noticed afterwards.
    unaddressable: Vec<&'static str>,
    /// Widgets the viewport excluded. Counted rather than built, so a reply
    /// can say how much of the interface it is showing without a second pass
    /// over the whole thing — which is what the first version of this did, and
    /// it cost more than the clipping saved.
    skipped: usize,
    /// When set, only widgets intersecting this rectangle are described.
    ///
    /// Clipping the finished tree kept the reply small and left the work in
    /// place: a thousand-row list still built a thousand `UiNode`s to throw
    /// away all but ten. Asking here instead means an off-screen widget costs
    /// nothing at all, the same way `ontology` already skips everything when
    /// no agent is attached.
    viewport: Option<Rect>,
}

impl<'a> Frame<'a> {
    /// Create a new frame with the given area, hit map, and painter.
    pub fn new(
        area: Rect,
        hit_map: &'a mut crate::event::HitMap,
        painter: &'a mut dyn crate::paint::Painter,
    ) -> Self {
        Self::with_ontology(area, hit_map, painter, true)
    }

    /// Create a frame, choosing whether to build the ontology tree.
    ///
    /// Building the tree costs an allocation-heavy `UiNode` per widget, and a
    /// frame that no agent will ever inspect throws all of it away. Pass
    /// `false` when no agent session is attached; hit-testing and painting are
    /// unaffected, so input keeps working either way.
    pub fn with_ontology(
        area: Rect,
        hit_map: &'a mut crate::event::HitMap,
        painter: &'a mut dyn crate::paint::Painter,
        ontology: bool,
    ) -> Self {
        Self {
            area,
            hit_map,
            ui_nodes: Vec::new(),
            painter,
            ontology,
            messages: Vec::new(),
            unaddressable: Vec::new(),
            skipped: 0,
            viewport: None,
        }
    }

    /// Describe only the widgets intersecting `viewport`.
    #[must_use]
    pub fn clipped_to(mut self, viewport: Rect) -> Self {
        self.viewport = Some(viewport);
        self
    }

    /// Whether this frame is collecting ontology nodes.
    ///
    /// Widgets check this before building a [`UiNode`](crate::ontology::UiNode)
    /// so the construction cost is skipped, not just the registration.
    #[must_use]
    pub fn ontology_enabled(&self) -> bool {
        self.ontology
    }

    /// Whether this frame wants a description of a widget occupying `area`.
    ///
    /// Widgets call this instead of [`ontology_enabled`](Self::ontology_enabled)
    /// so that building a [`UiNode`](crate::ontology::UiNode) is skipped
    /// entirely for a widget outside the viewport, rather than the node being
    /// built and then discarded.
    pub fn describes(&mut self, area: Rect) -> bool {
        if !self.ontology {
            return false;
        }
        let Some(view) = self.viewport else {
            return true;
        };
        let visible = area.x < view.x + view.width
            && area.x + area.width > view.x
            && area.y < view.y + view.height
            && area.y + area.height > view.y;
        if !visible {
            self.skipped += 1;
        }
        visible
    }

    /// How many widgets the viewport excluded this frame.
    #[must_use]
    pub fn skipped(&self) -> usize {
        self.skipped
    }

    /// Get a mutable reference to the painter for this frame.
    pub fn painter(&mut self) -> &mut dyn crate::paint::Painter {
        self.painter
    }

    /// Register a widget in the UI tree for agent discoverability.
    pub fn register_widget(&mut self, node: crate::ontology::UiNode) {
        if !self.ontology {
            return;
        }
        self.ui_nodes.push(node);
    }

    /// Register a hitbox for mouse event routing.
    pub fn register_hitbox(
        &mut self,
        agent_id: impl Into<std::borrow::Cow<'static, str>>,
        bounds: Rect,
        z_order: u32,
    ) {
        self.hit_map.register(agent_id, bounds, z_order);
    }

    /// Register the message to dispatch when `agent_id` is activated.
    ///
    /// Widgets call this from [`Widget::action`](crate::widget::Button::action).
    /// The message is type-erased because [`Frame`] is not generic over the
    /// application's message type; the runtime downcasts it back.
    pub fn register_message(
        &mut self,
        agent_id: impl Into<std::borrow::Cow<'static, str>>,
        action: &'static str,
        msg: Box<dyn std::any::Any + Send>,
    ) {
        self.messages.push((agent_id.into(), action, msg));
    }

    /// Record that an interactive widget rendered with no id.
    ///
    /// Such a widget looks correct on screen but is dead: no hitbox, no
    /// ontology node, and nothing an agent can name.
    pub fn note_unaddressable(&mut self, widget_type: &'static str) {
        self.unaddressable.push(widget_type);
    }

    /// Take the unaddressable-widget reports collected this frame.
    pub fn take_unaddressable(&mut self) -> Vec<&'static str> {
        std::mem::take(&mut self.unaddressable)
    }

    /// Take the messages widgets registered this frame.
    pub fn take_messages(
        &mut self,
    ) -> Vec<(
        std::borrow::Cow<'static, str>,
        &'static str,
        Box<dyn std::any::Any + Send>,
    )> {
        std::mem::take(&mut self.messages)
    }

    /// Take the collected UI nodes (consumed by the runtime after rendering).
    pub fn take_nodes(&mut self) -> Vec<crate::ontology::UiNode> {
        std::mem::take(&mut self.ui_nodes)
    }
}

/// A change to apply to the model when a widget is activated.
///
/// The Elm loop asks for a message type and an `update` arm per message, which
/// is the right shape for an application with real state transitions and the
/// wrong shape for a button that adds one to a number. A widget can carry one
/// of these instead, and the runtime applies it directly — no `Msg` variant, no
/// `update` arm, no `execute_action` handler, and it is driven by an agent
/// exactly like a message is.
pub type Mutation<M> = Box<dyn FnOnce(&mut M) + Send>;

/// A change that takes the widget's new value, as JSON.
///
/// What [`Mutation`] is for a button, this is for a text field or a slider:
/// the value arrives from a person typing or an agent's
/// `execute_action(id, "set_text", {"text": ...})`, by the same path.
pub type ValueMutation<M> = Box<dyn FnOnce(&mut M, &serde_json::Value) + Send>;

/// The changes the widgets of one frame registered, keyed by widget *and*
/// action.
///
/// A widget may register more than one: a `Tree` advertises `expand`,
/// `collapse`, `expand_all` and `collapse_all`, and an agent reading the
/// ontology may call any of them. Keying by id alone would silently keep only
/// the last.
///
/// Both the headless driver and the windowed backend dispatch through this one
/// type. They previously kept a map each and drifted apart, which is how a
/// handler bound to the wrong action name survived: fixing one copy did not
/// fix the other.
pub struct Handlers<M> {
    entries: Vec<(String, &'static str, Box<dyn std::any::Any + Send>)>,
    model: std::marker::PhantomData<fn(&mut M)>,
}

impl<M> Default for Handlers<M> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            model: std::marker::PhantomData,
        }
    }
}

impl<M: Model + 'static> std::fmt::Debug for Handlers<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Handlers")
            .field("registered", &self.list())
            .finish()
    }
}

impl<M: Model + 'static> Handlers<M> {
    /// Take everything the widgets registered while rendering `frame`.
    pub fn take_from(frame: &mut Frame<'_>) -> Self {
        Self {
            entries: frame
                .take_messages()
                .into_iter()
                .map(|(id, action, msg)| (id.into_owned(), action, msg))
                .collect(),
            model: std::marker::PhantomData,
        }
    }

    /// Forget everything registered.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// The `(widget, action)` pairs that have a handler, in registration order.
    #[must_use]
    pub fn list(&self) -> Vec<(String, &'static str)> {
        self.entries
            .iter()
            .map(|(id, action, _)| (id.clone(), *action))
            .collect()
    }

    /// Activate `agent_id` the way a click on it does, and return the command.
    ///
    /// Every host that turns a physical click into a widget's action goes
    /// through here — the headless driver, the agpu backend and the default
    /// one. Each had its own copy of "look up the primary action, then apply
    /// it", and this project has now twice watched the third copy of something
    /// be the one that disagreed.
    pub fn apply_primary(&mut self, agent_id: &str, model: &mut M) -> Option<Command<M::Msg>> {
        let action = self.primary_action(agent_id)?;
        self.apply(agent_id, action, &serde_json::Value::Null, model)
    }

    /// The action a click on `agent_id` should fire: the first one registered.
    ///
    /// A click is physical — it means "activate this widget", not any
    /// particular action name. A `Checkbox` advertises `toggle` and a `Button`
    /// advertises `click`; pressing either must work.
    #[must_use]
    pub fn primary_action(&self, agent_id: &str) -> Option<&'static str> {
        self.entries
            .iter()
            .find(|(id, _, _)| id == agent_id)
            .map(|(_, action, _)| *action)
    }

    /// Apply the handler registered for `agent_id`/`action`, if there is one.
    ///
    /// Returns the command it produced, or `None` when nothing was registered
    /// under that exact pair — a widget answers for its own actions only, so
    /// `execute_action(id, "focus")` must not fire the handler bound to a
    /// click.
    pub fn apply(
        &mut self,
        agent_id: &str,
        action: &str,
        params: &serde_json::Value,
        model: &mut M,
    ) -> Option<Command<M::Msg>> {
        let at = self
            .entries
            .iter()
            .position(|(id, registered, _)| id == agent_id && *registered == action)?;
        let (_, _, boxed) = self.entries.remove(at);
        match boxed.downcast::<M::Msg>() {
            Ok(msg) => Some(Command::Message(*msg)),
            Err(other) => match other.downcast::<Mutation<M>>() {
                Ok(change) => {
                    (*change)(model);
                    Some(Command::None)
                }
                Err(other) => match other.downcast::<ValueMutation<M>>() {
                    Ok(change) => {
                        (*change)(model, params);
                        Some(Command::None)
                    }
                    Err(_) => {
                        debug_assert!(
                            false,
                            "widget `{agent_id}` carried neither this model's Msg nor a Mutation"
                        );
                        None
                    }
                },
            },
        }
    }
}

/// When the runtime builds the agent ontology tree.
///
/// Building it allocates a [`UiNode`](crate::ontology::UiNode) per widget. A
/// GUI redraws at 60 Hz; an agent inspects it a handful of times a second at
/// most, so building the tree on every frame does that work one to two orders
/// of magnitude more often than anything reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OntologyMode {
    /// Build the tree just before an agent reads it, by running an extra
    /// paint-free `view` pass. `Model::view` takes `&self`, so the pass has no
    /// side effects, and the tree an agent sees is current by construction.
    ///
    /// This is the default: the tree is always fresh when read, and normal
    /// frames skip the work entirely.
    #[default]
    OnDemand,
    /// Build the tree during every rendered frame.
    ///
    /// Use when something outside the agent request path reads the ontology
    /// registry and expects it to track the last rendered frame.
    EveryFrame,
    /// Never build the tree. Agents see an empty ontology.
    Disabled,
}

/// Build an ontology tree for a model without painting anything.
///
/// Runs `view` against a [`NullPainter`](crate::paint::NullPainter), so it
/// costs widget construction and layout but no rendering. This is what
/// [`OntologyMode::OnDemand`] uses to answer an agent query.
pub fn build_ontology_tree<M: Model>(model: &M, area: Rect) -> crate::ontology::UiTree {
    let mut painter = crate::paint::NullPainter;
    let mut hit_map = crate::event::HitMap::new();
    let mut frame = Frame::with_ontology(area, &mut hit_map, &mut painter, true);
    model.view(&mut frame);

    let mut root = crate::ontology::UiNode::new("root", crate::ontology::SemanticRole::Container);
    root.children = frame.take_nodes();
    crate::ontology::UiTree::new(root)
}

/// Configuration for the application runner.
pub struct ProgramOptions {
    /// Tick interval for animation. `None` disables ticking.
    pub tick_rate: Option<Duration>,
    /// Initial window width in logical pixels.
    pub width: f32,
    /// Initial window height in logical pixels.
    pub height: f32,
    /// Whether to start in fullscreen.
    pub fullscreen: bool,
    /// Whether the window is resizable.
    pub resizable: bool,
    /// Whether to enable vsync.
    pub vsync: bool,
    /// Whether to use a transparent window.
    pub transparent: bool,
    /// Whether the window floats above others.
    ///
    /// A docked sidebar or utility panel needs this; without it the window is
    /// an ordinary one that disappears behind whatever is clicked next.
    pub always_on_top: bool,
    /// Whether the window has a title bar and borders.
    pub decorated: bool,
    /// Initial top-left position in logical pixels. `None` lets the platform
    /// place the window.
    pub position: Option<(f32, f32)>,
    /// Smallest size the window may be resized to.
    pub min_size: Option<(f32, f32)>,
    /// Largest size the window may be resized to.
    pub max_size: Option<(f32, f32)>,
    /// When to build the agent ontology tree. See [`OntologyMode`].
    pub ontology: OntologyMode,
}

impl Default for ProgramOptions {
    fn default() -> Self {
        Self {
            tick_rate: Some(Duration::from_millis(16)), // ~60fps
            width: 800.0,
            height: 600.0,
            fullscreen: false,
            resizable: true,
            vsync: true,
            transparent: false,
            always_on_top: false,
            decorated: true,
            position: None,
            min_size: None,
            max_size: None,
            ontology: OntologyMode::default(),
        }
    }
}

/// The main application runner.
///
/// Manages the event loop, rendering, and command dispatch.
/// When the "egui-backend" feature is enabled, this runs an eframe app.
#[cfg(feature = "egui-backend")]
pub struct Program<M: Model> {
    model: M,
    options: ProgramOptions,
    plugins: crate::plugin::PluginRegistry,
}

#[cfg(feature = "egui-backend")]
impl<M: Model + 'static> Program<M> {
    /// Create a new program with the given model.
    pub fn new(model: M) -> Self {
        Self {
            model,
            options: ProgramOptions::default(),
            plugins: crate::plugin::PluginRegistry::new(),
        }
    }

    /// Override the default program options.
    pub fn with_options(mut self, options: ProgramOptions) -> Self {
        self.options = options;
        self
    }

    /// Register a plugin. Plugins are initialised before the first frame.
    ///
    /// This existed only on `AgpuProgram`, which is the opt-in backend. Under
    /// the default one a plugin could not be registered at all, so `init`,
    /// `on_frame` and `on_shutdown` were never called and a plugin's ontology
    /// registrations never reached an agent.
    pub fn with_plugin(mut self, plugin: impl crate::plugin::Plugin + 'static) -> Self {
        self.plugins.register(plugin);
        self
    }

    /// Run the application. This blocks until the window closes.
    pub fn run(self) -> Result<(), eframe::Error> {
        let options = eframe::NativeOptions {
            viewport: {
                // Every field here was already declared on `ProgramOptions` or
                // `WindowConfig` and several never reached the window:
                // `fullscreen` produced a fullscreen window under the agpu
                // backend and was silently dropped under this one.
                let mut viewport = egui::ViewportBuilder::default()
                    .with_inner_size([self.options.width, self.options.height])
                    .with_resizable(self.options.resizable)
                    .with_transparent(self.options.transparent)
                    .with_fullscreen(self.options.fullscreen)
                    .with_decorations(self.options.decorated);
                if self.options.always_on_top {
                    viewport = viewport.with_always_on_top();
                }
                if let Some((x, y)) = self.options.position {
                    viewport = viewport.with_position([x, y]);
                }
                if let Some((w, h)) = self.options.min_size {
                    viewport = viewport.with_min_inner_size([w, h]);
                }
                if let Some((w, h)) = self.options.max_size {
                    viewport = viewport.with_max_inner_size([w, h]);
                }
                viewport
            },
            vsync: self.options.vsync,
            ..Default::default()
        };

        let title = self.model.title().to_string();

        eframe::run_native(
            &title,
            options,
            Box::new(move |_cc| {
                Ok(Box::new(DeweyApp::new(
                    self.model,
                    self.options,
                    self.plugins,
                )))
            }),
        )
    }
}

/// Internal eframe app wrapper.
#[cfg(feature = "egui-backend")]
struct DeweyApp<M: Model> {
    model: M,
    hit_map: crate::event::HitMap,
    ontology: OntologyRegistry,
    options: ProgramOptions,
    running: bool,
    last_tick: std::time::Instant,
    /// Whether the window had keyboard focus last frame, so that a change can
    /// be reported as an event. egui exposes focus as state, not an event.
    focused: bool,
    /// The window size last frame, for the same reason.
    last_size: crate::core::Size,
    /// Whether files were hovering last frame, so the hover is reported once
    /// at each end rather than on every frame in between.
    hovering_files: bool,
    /// Whether the ontology tree is also published to the platform
    /// accessibility API. Set once at startup; the tree must then be built
    /// every frame, because a screen reader reads it between agent requests.
    accessibility: bool,
    /// Window operations waiting for a frame to send them on.
    ///
    /// `Model::update` runs without an `egui::Context` — deliberately, so that
    /// an application is not written against one backend — so a command that
    /// moves the window is queued here and flushed when the frame that has a
    /// context comes round.
    pending_viewport: Vec<egui::ViewportCommand>,
    /// What the widgets registered while rendering the last frame.
    ///
    /// A click is hit-tested against `hit_map` and activated through this, so
    /// a `Button::action` fires without the application doing coordinate
    /// arithmetic in `handle_event`. Both are from the previous frame, which
    /// is the frame the user was looking at when they pressed the button.
    handlers: Handlers<M>,
    /// Plugins, initialised before the first frame and ticked on every one.
    plugins: crate::plugin::PluginRegistry,
    /// Whether the plugin shutdown hook has run, so it runs exactly once.
    shut_down: bool,
}

#[cfg(feature = "egui-backend")]
impl<M: Model> DeweyApp<M> {
    fn new(
        mut model: M,
        options: ProgramOptions,
        mut plugins: crate::plugin::PluginRegistry,
    ) -> Self {
        let initial_size = crate::core::Size::new(options.width, options.height);
        let mut ontology = OntologyRegistry::new();
        model.register_ontology(&mut ontology);
        // Before `init`: a plugin registers widgets and capabilities that the
        // model's first command may already depend on.
        let contributions = crate::plugin::initialise(&mut plugins, &mut ontology);
        model.plugins_ready(&contributions);
        let init_cmd = model.init();

        let mut app = Self {
            model,
            hit_map: crate::event::HitMap::new(),
            ontology,
            options,
            running: true,
            last_tick: std::time::Instant::now(),
            accessibility: cfg!(feature = "accesskit"),
            focused: true,
            last_size: initial_size,
            hovering_files: false,
            pending_viewport: Vec::new(),
            handlers: Handlers::default(),
            plugins,
            shut_down: false,
        };
        app.process_command(init_cmd);
        app
    }

    /// Run the plugin shutdown hook once, however the application is ending.
    ///
    /// `eframe::App::on_exit` would be the obvious place, and its signature
    /// gains a `glow::Context` parameter when anything in the dependency graph
    /// enables eframe's `glow` feature — as `benches/scaffold` does through
    /// egui — so implementing it breaks the build for a downstream crate and
    /// not for this one.
    fn shutdown(&mut self) {
        if !self.shut_down {
            self.shut_down = true;
            self.plugins.on_shutdown();
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
            Command::SetTickRate(_duration) => {
                // Handled by egui's repaint scheduling
            }
            Command::ExportOntology => {
                self.model.register_ontology(&mut self.ontology);
            }
            Command::AgentAction {
                agent_id,
                action,
                params,
            } => {
                log::debug!("AgentAction: {agent_id}.{action}({params})");
            }
            Command::SetWindowVisible(visible) => {
                self.pending_viewport
                    .push(egui::ViewportCommand::Visible(visible));
            }
            Command::FocusWindow => {
                self.pending_viewport
                    .push(egui::ViewportCommand::Visible(true));
                self.pending_viewport.push(egui::ViewportCommand::Focus);
            }
            Command::MinimiseWindow => {
                self.pending_viewport
                    .push(egui::ViewportCommand::Minimized(true));
            }
            Command::SetWindowPosition { x, y } => {
                self.pending_viewport
                    .push(egui::ViewportCommand::OuterPosition(egui::pos2(x, y)));
            }
            Command::SetWindowSize { width, height } => {
                self.pending_viewport
                    .push(egui::ViewportCommand::InnerSize(egui::vec2(width, height)));
            }
            Command::SetAlwaysOnTop(on) => {
                self.pending_viewport
                    .push(egui::ViewportCommand::WindowLevel(if on {
                        egui::WindowLevel::AlwaysOnTop
                    } else {
                        egui::WindowLevel::Normal
                    }));
            }
            Command::SetFullscreen(on) => {
                self.pending_viewport
                    .push(egui::ViewportCommand::Fullscreen(on));
            }
            Command::SetWindowTitle(title) => {
                self.pending_viewport
                    .push(egui::ViewportCommand::Title(title));
            }
            Command::Task(task) => {
                // Execute the task synchronously in the update cycle.
                // For truly async I/O, wrap with tokio::task::spawn_blocking externally.
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

#[cfg(feature = "egui-backend")]
impl<M: Model + 'static> eframe::App for DeweyApp<M> {
    fn update(&mut self, ctx: &egui::Context, _eframe: &mut eframe::Frame) {
        // Before the running check: a model that hid its window still expects
        // the hide to happen.
        for cmd in self.pending_viewport.drain(..) {
            ctx.send_viewport_cmd(cmd);
        }

        if !self.running {
            self.shutdown();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        self.plugins.on_frame();

        // Convert egui input events to Dewey events and dispatch
        let mut events = convert_egui_events(ctx);

        // Files hovering over the window are republished every frame for as
        // long as the pointer is over it. The events are the hover starting
        // and the hover ending, not sixty a second saying it is still there.
        let hovering: Vec<String> = ctx.input(|i| {
            i.raw
                .hovered_files
                .iter()
                .filter_map(|f| f.path.as_ref())
                .map(|p| p.to_string_lossy().into_owned())
                .collect()
        });
        if !hovering.is_empty() && !self.hovering_files {
            self.hovering_files = true;
            events.push(crate::event::Event::FileHover(hovering));
        } else if hovering.is_empty() && self.hovering_files {
            self.hovering_files = false;
            // A drop ends the hover by landing; anything else cancels it.
            if !events
                .iter()
                .any(|e| matches!(e, crate::event::Event::FileDrop(_)))
            {
                events.push(crate::event::Event::FileHoverCancelled);
            }
        }

        // Size and focus are both state that egui reports every frame, and an
        // application wants to hear about the change. Emitting on every frame
        // instead would put a resize event in front of the model sixty times a
        // second while nothing moved.
        let size = ctx.input(|i| i.screen_rect());
        let size = crate::core::Size::new(size.width(), size.height());
        if (size.width - self.last_size.width).abs() > f32::EPSILON
            || (size.height - self.last_size.height).abs() > f32::EPSILON
        {
            self.last_size = size;
            events.push(crate::event::Event::Resize(size));
        }

        // Focus is state rather than an event: egui reports where it is, and
        // the change is what an application wants to hear about.
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
        if focused != self.focused {
            self.focused = focused;
            events.push(if focused {
                crate::event::Event::FocusGained
            } else {
                crate::event::Event::FocusLost
            });
        }

        for event in events {
            // The window closing is the end of the application whether or not
            // the model turns it into a `Quit`; a plugin that holds a file
            // handle should hear about it either way.
            if matches!(event, crate::event::Event::CloseRequested) {
                self.shutdown();
            }
            // A click goes to whatever the hit map says is under it. This
            // backend converted the click into an `Event::Mouse`, handed it to
            // `handle_event` and stopped: it never hit-tested and held no
            // handlers, so `Button::action` — the wiring the README's quick
            // start is built on — did nothing under the backend `Program::run`
            // actually uses. It worked headless and under agpu, so every test
            // passed.
            if let crate::event::Event::Mouse(m) = &event {
                if m.is_click() {
                    if let Some(id) = self.hit_map.hit_test(m.position).map(str::to_owned) {
                        if let Some(cmd) = self.handlers.apply_primary(&id, &mut self.model) {
                            self.process_command(cmd);
                        }
                    }
                }
            }
            if let Some(msg) = self.model.handle_event(event) {
                let cmd = self.model.update(msg);
                self.process_command(cmd);
            }
        }

        // Emit tick events at the configured rate
        if let Some(tick_rate) = self.options.tick_rate {
            if self.last_tick.elapsed() >= tick_rate {
                self.last_tick = std::time::Instant::now();
                if let Some(msg) = self.model.handle_event(crate::event::Event::Tick) {
                    let cmd = self.model.update(msg);
                    self.process_command(cmd);
                }
            }
        }

        // Render
        self.hit_map.clear();
        let available = ctx.available_rect();
        let area = Rect::new(
            available.min.x,
            available.min.y,
            available.width(),
            available.height(),
        );

        // `Frame::new` builds the ontology unconditionally, so this backend
        // was paying for a tree on every frame regardless of `OntologyMode` —
        // the option is documented as defaulting to `OnDemand` and the agpu
        // backend honours it. Screen-reader support needs the tree every
        // frame, so accessibility forces it back on.
        let build_tree = self.options.ontology == OntologyMode::EveryFrame || self.accessibility;

        // The closure borrows `self.hit_map` through the frame, so what the
        // widgets registered comes out here and is stored after it returns.
        let mut handlers = Handlers::default();
        egui::CentralPanel::default().show(ctx, |ui| {
            let mut egui_painter = crate::backend::egui_backend::EguiPainter::new(ctx);
            let mut frame =
                Frame::with_ontology(area, &mut self.hit_map, &mut egui_painter, build_tree);
            self.model.view(&mut frame);
            handlers = Handlers::take_from(&mut frame);

            // Collect UI tree
            let nodes = frame.take_nodes();
            if !nodes.is_empty() {
                let root =
                    crate::ontology::UiNode::new("root", crate::ontology::SemanticRole::Container);
                let mut root = root;
                root.children = nodes;
                self.ontology.set_tree(crate::ontology::UiTree::new(root));
            }

            #[cfg(feature = "accesskit")]
            if self.accessibility {
                if let Some(tree) = self.ontology.tree() {
                    crate::accesskit_bridge::publish(ui, tree);
                }
            }
            let _ = ui;
        });
        self.handlers = handlers;

        // Schedule next repaint at the tick rate to avoid overwhelming the swapchain
        if let Some(tick_rate) = self.options.tick_rate {
            ctx.request_repaint_after(tick_rate);
        }
    }
}

/// Convert egui input events to Dewey events.
#[cfg(feature = "egui-backend")]
fn convert_egui_events(ctx: &egui::Context) -> Vec<crate::event::Event> {
    let mut events = Vec::new();
    let input = ctx.input(|i| i.clone());

    // Files dragged onto the window. egui collects these outside its event
    // stream, so a backend that only walks `input.events` never sees them —
    // which is why the default backend delivered none of this while the agpu
    // one delivered all of it.
    //
    // These are the same three events agpu emits, not a second way of saying
    // the same thing: an application must not have to ask which backend it is
    // running on to know which event a dropped file arrives as.
    // A drop is a single moment, so it belongs here. The hover around it is a
    // state that egui republishes every frame, and is handled where the last
    // frame is remembered.
    if !input.raw.dropped_files.is_empty() {
        let dropped: Vec<String> = input
            .raw
            .dropped_files
            .iter()
            .filter_map(|f| f.path.as_ref())
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        if !dropped.is_empty() {
            events.push(crate::event::Event::FileDrop(dropped));
        }
    }

    for event in &input.events {
        match event {
            egui::Event::Key {
                key,
                pressed,
                modifiers,
                ..
            } => {
                if let Some(code) = convert_egui_key(*key) {
                    let mut mods = crate::event::KeyModifiers::empty();
                    if modifiers.shift {
                        mods |= crate::event::KeyModifiers::SHIFT;
                    }
                    if modifiers.ctrl || modifiers.command {
                        mods |= crate::event::KeyModifiers::CONTROL;
                    }
                    if modifiers.alt {
                        mods |= crate::event::KeyModifiers::ALT;
                    }
                    let kind = if *pressed {
                        crate::event::KeyEventKind::Press
                    } else {
                        crate::event::KeyEventKind::Release
                    };
                    events.push(crate::event::Event::Key(crate::event::KeyEvent {
                        code,
                        modifiers: mods,
                        kind,
                    }));
                }
            }
            egui::Event::Text(text) => {
                events.push(crate::event::Event::TextInput(text.clone()));
            }
            egui::Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers,
            } => {
                let btn = match button {
                    egui::PointerButton::Primary => crate::event::MouseButton::Left,
                    egui::PointerButton::Secondary => crate::event::MouseButton::Right,
                    egui::PointerButton::Middle => crate::event::MouseButton::Middle,
                    _ => crate::event::MouseButton::Left,
                };
                let mut mods = crate::event::KeyModifiers::empty();
                if modifiers.shift {
                    mods |= crate::event::KeyModifiers::SHIFT;
                }
                if modifiers.ctrl || modifiers.command {
                    mods |= crate::event::KeyModifiers::CONTROL;
                }
                if modifiers.alt {
                    mods |= crate::event::KeyModifiers::ALT;
                }
                let kind = if *pressed {
                    crate::event::MouseEventKind::Click(btn)
                } else {
                    crate::event::MouseEventKind::Release(btn)
                };
                events.push(crate::event::Event::Mouse(crate::event::MouseEvent {
                    kind,
                    position: crate::core::Position::new(pos.x, pos.y),
                    modifiers: mods,
                }));
            }
            egui::Event::PointerMoved(pos) => {
                events.push(crate::event::Event::Mouse(crate::event::MouseEvent {
                    kind: crate::event::MouseEventKind::Move,
                    position: crate::core::Position::new(pos.x, pos.y),
                    modifiers: crate::event::KeyModifiers::empty(),
                }));
            }
            egui::Event::MouseWheel { delta, .. } => {
                events.push(crate::event::Event::Mouse(crate::event::MouseEvent {
                    kind: crate::event::MouseEventKind::Scroll {
                        delta_x: delta.x,
                        delta_y: delta.y,
                    },
                    position: crate::core::Position::ZERO,
                    modifiers: crate::event::KeyModifiers::empty(),
                }));
            }
            _ => {}
        }
    }

    if input.viewport().close_requested() {
        events.push(crate::event::Event::CloseRequested);
    }

    events
}

/// Convert egui key to Dewey KeyCode.
#[cfg(feature = "egui-backend")]
fn convert_egui_key(key: egui::Key) -> Option<crate::event::KeyCode> {
    use crate::event::KeyCode;
    Some(match key {
        egui::Key::ArrowDown => KeyCode::Down,
        egui::Key::ArrowLeft => KeyCode::Left,
        egui::Key::ArrowRight => KeyCode::Right,
        egui::Key::ArrowUp => KeyCode::Up,
        egui::Key::Escape => KeyCode::Esc,
        egui::Key::Tab => KeyCode::Tab,
        egui::Key::Backspace => KeyCode::Backspace,
        egui::Key::Enter => KeyCode::Enter,
        egui::Key::Space => KeyCode::Char(' '),
        egui::Key::Insert => KeyCode::Insert,
        egui::Key::Delete => KeyCode::Delete,
        egui::Key::Home => KeyCode::Home,
        egui::Key::End => KeyCode::End,
        egui::Key::PageUp => KeyCode::PageUp,
        egui::Key::PageDown => KeyCode::PageDown,
        egui::Key::F1 => KeyCode::F(1),
        egui::Key::F2 => KeyCode::F(2),
        egui::Key::F3 => KeyCode::F(3),
        egui::Key::F4 => KeyCode::F(4),
        egui::Key::F5 => KeyCode::F(5),
        egui::Key::F6 => KeyCode::F(6),
        egui::Key::F7 => KeyCode::F(7),
        egui::Key::F8 => KeyCode::F(8),
        egui::Key::F9 => KeyCode::F(9),
        egui::Key::F10 => KeyCode::F(10),
        egui::Key::F11 => KeyCode::F(11),
        egui::Key::F12 => KeyCode::F(12),
        egui::Key::A => KeyCode::Char('a'),
        egui::Key::B => KeyCode::Char('b'),
        egui::Key::C => KeyCode::Char('c'),
        egui::Key::D => KeyCode::Char('d'),
        egui::Key::E => KeyCode::Char('e'),
        egui::Key::F => KeyCode::Char('f'),
        egui::Key::G => KeyCode::Char('g'),
        egui::Key::H => KeyCode::Char('h'),
        egui::Key::I => KeyCode::Char('i'),
        egui::Key::J => KeyCode::Char('j'),
        egui::Key::K => KeyCode::Char('k'),
        egui::Key::L => KeyCode::Char('l'),
        egui::Key::M => KeyCode::Char('m'),
        egui::Key::N => KeyCode::Char('n'),
        egui::Key::O => KeyCode::Char('o'),
        egui::Key::P => KeyCode::Char('p'),
        egui::Key::Q => KeyCode::Char('q'),
        egui::Key::R => KeyCode::Char('r'),
        egui::Key::S => KeyCode::Char('s'),
        egui::Key::T => KeyCode::Char('t'),
        egui::Key::U => KeyCode::Char('u'),
        egui::Key::V => KeyCode::Char('v'),
        egui::Key::W => KeyCode::Char('w'),
        egui::Key::X => KeyCode::Char('x'),
        egui::Key::Y => KeyCode::Char('y'),
        egui::Key::Z => KeyCode::Char('z'),
        egui::Key::Num0 => KeyCode::Char('0'),
        egui::Key::Num1 => KeyCode::Char('1'),
        egui::Key::Num2 => KeyCode::Char('2'),
        egui::Key::Num3 => KeyCode::Char('3'),
        egui::Key::Num4 => KeyCode::Char('4'),
        egui::Key::Num5 => KeyCode::Char('5'),
        egui::Key::Num6 => KeyCode::Char('6'),
        egui::Key::Num7 => KeyCode::Char('7'),
        egui::Key::Num8 => KeyCode::Char('8'),
        egui::Key::Num9 => KeyCode::Char('9'),
        _ => return None,
    })
}
