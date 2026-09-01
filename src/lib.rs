//! # Dewey — Agentic-First GUI Framework
//!
//! Dewey is a backend-agnostic Rust GUI framework that provides a complete
//! **ontology** over every widget, enabling AI agents to discover, inspect,
//! and control GUI applications through a structured protocol — no
//! screen-scraping required.
//!
//! All rendering goes through the [`Painter`](paint::Painter) trait, making
//! backends fully pluggable. The default `egui-backend` feature provides
//! GPU-accelerated rendering via egui/wgpu.
//!
//! ## Architecture (Elm-style)
//!
//! ```text
//! ┌──────────┐   Event    ┌──────────┐  Command   ┌──────────┐
//! │  Backend │ ────────→ │  Model   │ ────────→ │ Runtime  │
//! │ (Painter)│           │ (update) │           │ (Program)│
//! │          │ ←──────── │  (view)  │ ←──────── │          │
//! └──────────┘   Frame   └──────────┘           └──────────┘
//!       ↕                      ↕
//! ┌──────────┐         ┌──────────────┐
//! │  Widgets │         │  Ontology    │
//! │  (30)    │         │  (Registry)  │
//! └──────────┘         └──────────────┘
//!                            ↕
//!                      ┌──────────┐
//!                      │  Agent   │
//!                      │ Protocol │
//!                      └──────────┘
//! ```
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use dewey::prelude::*;
//!
//! struct App { count: i32 }
//!
//! enum Msg { Increment, Decrement }
//!
//! impl Model for App {
//!     type Msg = Msg;
//!
//!     fn update(&mut self, msg: Msg) -> Command<Msg> {
//!         match msg {
//!             Msg::Increment => self.count += 1,
//!             Msg::Decrement => self.count -= 1,
//!         }
//!         Command::None
//!     }
//!
//!     fn view(&self, frame: &mut Frame<'_>) {
//!         // Render widgets to frame
//!         let area = frame.area;
//!         Label::new(format!("Count: {}", self.count)).render(area, frame);
//!     }
//!
//!     fn handle_event(&self, _event: Event) -> Option<Msg> {
//!         None
//!     }
//! }
//! ```
//!
//! ## Modules
//!
//! | Module       | Purpose                                        |
//! |-------------|--------------------------------------------------|
//! | `core`      | Rect, Position, Size, Color, Style primitives    |
//! | `widget`    | 27 widgets (Button, Label, TextInput, Table...)   |
//! | `ontology`  | Schema, capabilities, actions, registry           |
//! | `agent`     | JSON Lines protocol, RPC, headless driver        |
//! | `runtime`   | Model trait, Command, Program runner             |
//! | `event`     | Keyboard, mouse, touch, file-drop events         |
//! | `layout`    | Constraint-based layout engine                   |
//! | `animation` | Easing, Tween, Spring, Timeline, Keyframes       |
//! | `backend`   | egui/eframe (wgpu) + test backend                |
//! | `focus`     | Tab navigation focus management                  |
//! | `overlay`   | Modal/dialog overlay stack                       |
//! | `theme`     | Token-based theming with dark/light presets       |
//! | `util`      | Fuzzy matching, undo/redo, state persistence     |

#[cfg(feature = "accesskit")]
pub mod accesskit_bridge;
pub mod agent;
pub mod animation;
pub mod backend;
pub mod core;

// Re-export derive macro
#[cfg(feature = "derive")]
pub use dewey_derive::Widget;
pub mod dialog;
pub mod error;
pub mod event;
pub mod focus;
pub mod gpu;
pub mod i18n;
pub mod layout;
pub mod memory;
pub mod ontology;
pub mod overlay;
pub mod paint;
pub mod plugin;
pub mod profiling;
pub mod runtime;
pub mod theme;
pub mod tray;
pub mod util;
pub mod widget;
pub mod window;

/// Convenience prelude — import everything needed for a typical app.
pub mod prelude {
    #[cfg(feature = "agpu-backend")]
    pub use crate::backend::agpu_backend::{AgpuBridgePainter, AgpuProgram};
    pub use crate::core::{
        Alignment, Color, FontWeight, Margin, Position, Rect, Shadow, Size, Style, TextStyle,
        VerticalAlignment,
    };
    pub use crate::error::{Error, Result};
    pub use crate::event::{
        DragDropEvent, DragDropKind, DragPayload, Event, KeyCode, KeyEvent, KeyModifiers,
        MouseButton, MouseEvent,
    };
    pub use crate::focus::FocusManager;
    pub use crate::i18n::{I18n, MessageCatalog};
    pub use crate::layout::{Constraint, Direction, Flex, Layout};
    pub use crate::ontology::{
        AgentAction, AgentCapability, Discoverable, OntologyRegistry, SemanticRole, UiNode, UiTree,
        WidgetSchema,
    };
    pub use crate::overlay::{ModalBox, Overlay, OverlayStack};
    pub use crate::paint::{NullPainter, Painter};
    pub use crate::plugin::{Plugin, PluginContext, PluginRegistry};
    #[cfg(feature = "egui-backend")]
    pub use crate::runtime::Program;
    pub use crate::runtime::{
        CancellationToken, Command, Frame, Model, OntologyMode, ProgramOptions,
    };
    pub use crate::theme::{Theme, ThemeToken, ThemeWatcher};
    pub use crate::widget::*;
    pub use crate::window::{WindowConfig, WindowId, WindowManager};
}
