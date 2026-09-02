//! # agpu — Standalone Agentic-First GPU Backend
//!
//! A standalone wgpu replacement with built-in ontology for agent
//! discoverability. Supports Vulkan (preferred), OpenGL/GLES, and
//! platform-default backends with automatic fallback.
//!
//! ## Architecture
//!
//! ```text
//! ┌────────────────────────────────────────────────┐
//! │  AgpuApp<M: Model>                             │
//! │  ├── GpuContext (instance/adapter/device/queue) │
//! │  ├── ShapeRenderer (batched 2D geometry)        │
//! │  ├── TextEngine (glyphon GPU text)              │
//! │  ├── AgpuPainter (Painter trait impl)           │
//! │  └── OntologyRegistry (agent discoverability)   │
//! └────────────────────────────────────────────────┘
//! ```
//!
//! ## Quick Start
//!
//! `no_run` rather than `ignore`: this opens a window, so it is compiled but
//! not executed. An ignored example is checked by nothing, and the two in this
//! workspace that were ignored both described APIs that did not exist.
//!
//! ```no_run
//! use agpu::prelude::*;
//!
//! struct Counter {
//!     count: i32,
//! }
//!
//! enum Msg {
//!     Increment,
//! }
//!
//! impl Model for Counter {
//!     type Msg = Msg;
//!
//!     fn update(&mut self, msg: Msg) -> Command<Msg> {
//!         match msg {
//!             Msg::Increment => self.count += 1,
//!         }
//!         Command::None
//!     }
//!
//!     fn view(&self, frame: &mut Frame<'_>) {
//!         let area = frame.area;
//!         frame
//!             .painter()
//!             .fill_rect(area, Color::rgba(0.1, 0.1, 0.12, 1.0), 0.0);
//!     }
//!
//!     fn handle_event(&self, _event: Event) -> Option<Msg> {
//!         None
//!     }
//! }
//!
//! fn main() {
//!     AgpuApp::new(Counter { count: 0 }).run().unwrap();
//! }
//! ```

pub mod accessibility;
pub mod animation;
pub mod app;
pub mod command;
pub mod compute;
pub mod context;
pub mod core;
pub mod event;
pub mod layout;
pub mod multiwindow;
pub mod ontology;
pub mod paint;
pub mod painter;
pub mod plugin;
pub mod renderer;
pub mod resource;
pub mod runtime;
pub mod scene3d;
pub mod surface;
pub mod text;
pub mod theme;
pub mod types;
pub mod vertex;
pub mod widget;

// ── Public API re-exports ───────────────────────────────────────────

// Core geometry and style types
pub use core::{Color, FontWeight, Margin, Position, Rect, Size, TextStyle};

// Core context and errors
pub use context::{GpuContext, GpuError};
pub use types::BackendPreference;

// Resource types
pub use resource::{
    GpuBindGroup, GpuBindGroupLayout, GpuBuffer, GpuComputePipeline, GpuPipelineLayout,
    GpuQuerySet, GpuRenderPipeline, GpuSampler, GpuShaderModule, GpuTexture, GpuTextureView,
    ShaderSourceKind,
};

// Command encoding
pub use command::{GpuCommandBuffer, GpuCommandEncoder, GpuComputePass, GpuRenderPass};

// Surface
pub use surface::{GpuSurface, GpuSurfaceTexture};

// Type re-exports (users import from agpu instead of wgpu)
pub use types::*;

// Application and rendering
pub use app::AgpuApp;
pub use event::convert_window_event;
pub use ontology::gpu::{
    AgpuAppOntology, PipelineOntology, ShapeRendererOntology, SurfaceOntology, TextEngineOntology,
};
pub use painter::AgpuPainter;
pub use renderer::ShapeRenderer;
pub use text::TextEngine;
pub use vertex::Vertex;

// Ontology (agent discoverability)
pub use ontology::{
    Accessibility, AgentAction, AgentCapability, Discoverable, NodeBounds, OntologyRegistry,
    SemanticRole, UiNode, UiTree, WidgetSchema,
};

// Paint trait
pub use paint::{Gradient, GradientStop, ImageHandle, NullPainter, Painter, Shadow};

// Runtime (Elm architecture)
pub use runtime::{CancellationToken, Command, Frame, Model, ProgramOptions, Router, Subscription};

// Event system
pub use event::{
    DragDropEvent, DragDropKind, DragPayload, Event, HitMap, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

/// Prelude — import everything commonly needed.
pub mod prelude {
    pub use crate::app::AgpuApp;
    pub use crate::core::{Color, FontWeight, Margin, Position, Rect, Size, TextStyle};
    pub use crate::event::{
        Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    pub use crate::ontology::{
        AgentAction, AgentCapability, Discoverable, OntologyRegistry, SemanticRole, UiNode,
        WidgetSchema,
    };
    pub use crate::paint::{NullPainter, Painter, Shadow};
    pub use crate::runtime::{Command, Frame, Model, ProgramOptions};
    pub use crate::types::BackendPreference;
}
