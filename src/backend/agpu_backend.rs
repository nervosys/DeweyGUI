//! agpu rendering backend — Vulkan-first GPU-accelerated rendering
//! via the `agpu` crate (wgpu + winit).
//!
//! Provides:
//! - [`AgpuBridgePainter`] — implements Dewey's [`Painter`](crate::paint::Painter)
//!   by delegating to agpu's [`ShapeRenderer`](agpu::ShapeRenderer) and
//!   [`TextEngine`](agpu::TextEngine).
//! - [`AgpuProgram`] — an application runner that drives Dewey's
//!   [`Model`](crate::runtime::Model) trait through a native winit event
//!   loop with GPU rendering.

use std::sync::Arc;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

use crate::core::rect::{Position, Rect, Size};
use crate::core::style::{Color, FontWeight, TextStyle};
use crate::event::HitMap;
use crate::i18n::I18n;
use crate::ontology::{OntologyRegistry, SemanticRole, UiNode, UiTree};
use crate::paint::Painter;
use crate::plugin::{PluginContext, PluginRegistry};
use crate::profiling::Profiler;
use crate::runtime::{Command, Frame, Model, ProgramOptions};
use crate::theme::Theme;

// ── Type conversion helpers ─────────────────────────────────────────

/// Convert a Dewey Rect to an agpu Rect.
#[inline]
fn to_agpu_rect(r: Rect) -> agpu::Rect {
    agpu::Rect::new(r.x, r.y, r.width, r.height)
}

/// Convert a Dewey Position to an agpu Position.
#[inline]
fn to_agpu_pos(p: Position) -> agpu::Position {
    agpu::Position::new(p.x, p.y)
}

/// Convert a Dewey Color to an agpu Color.
#[inline]
fn to_agpu_color(c: Color) -> agpu::Color {
    agpu::Color::rgba(c.r, c.g, c.b, c.a)
}

/// Convert a Dewey FontWeight to an agpu FontWeight.
#[inline]
fn to_agpu_font_weight(w: FontWeight) -> agpu::FontWeight {
    match w {
        FontWeight::Thin => agpu::FontWeight::Thin,
        FontWeight::Light => agpu::FontWeight::Light,
        FontWeight::Regular => agpu::FontWeight::Regular,
        FontWeight::Medium => agpu::FontWeight::Medium,
        FontWeight::SemiBold => agpu::FontWeight::SemiBold,
        FontWeight::Bold => agpu::FontWeight::Bold,
        FontWeight::ExtraBold => agpu::FontWeight::ExtraBold,
    }
}

/// Convert a Dewey TextStyle to an agpu TextStyle.
#[inline]
fn to_agpu_text_style(s: &TextStyle) -> agpu::TextStyle {
    agpu::TextStyle {
        font_size: s.font_size,
        color: to_agpu_color(s.color),
        weight: to_agpu_font_weight(s.weight),
        italic: s.italic,
        underline: s.underline,
        strikethrough: s.strikethrough,
        line_height: s.line_height,
        letter_spacing: s.letter_spacing,
    }
}

/// Convert an agpu Size to a Dewey Size.
#[inline]
#[allow(dead_code)]
fn from_agpu_size(s: agpu::Size) -> Size {
    Size::new(s.width, s.height)
}

// ── AgpuBridgePainter ───────────────────────────────────────────────

/// A painter that implements Dewey's [`Painter`](crate::paint::Painter)
/// trait by delegating to agpu's GPU rendering primitives.
///
/// Wraps agpu's [`ShapeRenderer`](agpu::ShapeRenderer) and
/// [`TextEngine`](agpu::TextEngine) to provide hardware-accelerated
/// 2D rendering through Dewey's abstract painting interface.
pub struct AgpuBridgePainter<'a> {
    shapes: &'a mut agpu::ShapeRenderer,
    text: &'a mut agpu::TextEngine,
    clip_stack: Vec<agpu::Rect>,
}

impl<'a> AgpuBridgePainter<'a> {
    /// Create a new bridge painter wrapping agpu's renderers.
    pub fn new(shapes: &'a mut agpu::ShapeRenderer, text: &'a mut agpu::TextEngine) -> Self {
        Self {
            shapes,
            text,
            clip_stack: Vec::new(),
        }
    }

    /// Current clipping rectangle, if any.
    fn current_clip(&self) -> Option<agpu::Rect> {
        self.clip_stack.last().copied()
    }
}

impl Painter for AgpuBridgePainter<'_> {
    fn fill_rect(&mut self, rect: Rect, color: Color, corner_radius: f32) {
        self.shapes
            .fill_rect(to_agpu_rect(rect), to_agpu_color(color), corner_radius);
    }

    fn stroke_rect(&mut self, rect: Rect, color: Color, width: f32, corner_radius: f32) {
        self.shapes.stroke_rect(
            to_agpu_rect(rect),
            to_agpu_color(color),
            width,
            corner_radius,
        );
    }

    fn fill_circle(&mut self, center: Position, radius: f32, color: Color) {
        self.shapes
            .fill_circle(to_agpu_pos(center), radius, to_agpu_color(color));
    }

    fn stroke_circle(&mut self, center: Position, radius: f32, color: Color, width: f32) {
        self.shapes
            .stroke_circle(to_agpu_pos(center), radius, to_agpu_color(color), width);
    }

    fn line(&mut self, from: Position, to: Position, color: Color, width: f32) {
        self.shapes.line(
            to_agpu_pos(from),
            to_agpu_pos(to),
            to_agpu_color(color),
            width,
        );
    }

    fn text(&mut self, pos: Position, text: &str, style: &TextStyle) {
        let agpu_style = to_agpu_text_style(style);
        let clip = self.current_clip();
        self.text.draw(to_agpu_pos(pos), text, &agpu_style, clip);
    }

    fn measure_text(&self, text: &str, style: &TextStyle) -> Size {
        // TextEngine::measure requires &mut self, so we estimate here.
        // Use per-character width classes for proportional fonts.
        let avg_char_width: f32 = text
            .chars()
            .map(|c| match c {
                'i' | 'l' | '!' | '|' | '.' | ',' | ':' | ';' | '\'' | ' ' => 0.35,
                'm' | 'w' | 'M' | 'W' => 0.75,
                'A'..='Z' => 0.65,
                _ => 0.52,
            })
            .sum();
        let w = style.font_size * avg_char_width;
        let line_h = style.line_height.unwrap_or(style.font_size * 1.3);
        let lines = text.lines().count().max(1) as f32;
        Size::new(w, line_h * lines)
    }

    fn push_clip(&mut self, rect: Rect) {
        let agpu_rect = to_agpu_rect(rect);
        let clipped = if let Some(current) = self.current_clip() {
            // Intersect with current clip
            current.intersection(&agpu_rect).unwrap_or(agpu::Rect::ZERO)
        } else {
            agpu_rect
        };
        self.clip_stack.push(clipped);
    }

    fn pop_clip(&mut self) {
        self.clip_stack.pop();
    }
}

// ── Event conversion ────────────────────────────────────────────────

/// Convert an agpu event to a Dewey event.
///
/// Returns `None` for agpu-only variants that Dewey doesn't handle
/// (AgentAction, ImePreedit, ImeCommit).
fn to_dewey_event(ev: agpu::Event) -> Option<crate::event::Event> {
    use crate::event;

    match ev {
        agpu::Event::Key(k) => Some(event::Event::Key(event::KeyEvent {
            code: to_dewey_keycode(k.code),
            modifiers: to_dewey_modifiers(k.modifiers),
            kind: to_dewey_key_kind(k.kind),
        })),
        agpu::Event::Mouse(m) => Some(event::Event::Mouse(event::MouseEvent {
            kind: to_dewey_mouse_kind(m.kind),
            position: Position::new(m.position.x, m.position.y),
            modifiers: to_dewey_modifiers(m.modifiers),
        })),
        agpu::Event::TextInput(s) => Some(event::Event::TextInput(s)),
        agpu::Event::Resize(s) => Some(event::Event::Resize(Size::new(s.width, s.height))),
        agpu::Event::FocusGained => Some(event::Event::FocusGained),
        agpu::Event::FocusLost => Some(event::Event::FocusLost),
        agpu::Event::CloseRequested => Some(event::Event::CloseRequested),
        agpu::Event::Tick => Some(event::Event::Tick),
        agpu::Event::FileDrop(files) => Some(event::Event::FileDrop(files)),
        agpu::Event::FileHover(files) => Some(event::Event::FileHover(files)),
        agpu::Event::FileHoverCancelled => Some(event::Event::FileHoverCancelled),
        agpu::Event::DragDrop(d) => Some(event::Event::DragDrop(to_dewey_drag_drop(d))),
        // agpu-only variants — skip
        agpu::Event::AgentAction { .. }
        | agpu::Event::ImePreedit { .. }
        | agpu::Event::ImeCommit(_) => None,
    }
}

fn to_dewey_keycode(code: agpu::KeyCode) -> crate::event::KeyCode {
    use crate::event::KeyCode;
    match code {
        agpu::KeyCode::Char(c) => KeyCode::Char(c),
        agpu::KeyCode::F(n) => KeyCode::F(n),
        agpu::KeyCode::Backspace => KeyCode::Backspace,
        agpu::KeyCode::Enter => KeyCode::Enter,
        agpu::KeyCode::Tab => KeyCode::Tab,
        agpu::KeyCode::BackTab => KeyCode::BackTab,
        agpu::KeyCode::Esc => KeyCode::Esc,
        agpu::KeyCode::Left => KeyCode::Left,
        agpu::KeyCode::Right => KeyCode::Right,
        agpu::KeyCode::Up => KeyCode::Up,
        agpu::KeyCode::Down => KeyCode::Down,
        agpu::KeyCode::Home => KeyCode::Home,
        agpu::KeyCode::End => KeyCode::End,
        agpu::KeyCode::PageUp => KeyCode::PageUp,
        agpu::KeyCode::PageDown => KeyCode::PageDown,
        agpu::KeyCode::Insert => KeyCode::Insert,
        agpu::KeyCode::Delete => KeyCode::Delete,
        agpu::KeyCode::Null => KeyCode::Null,
    }
}

fn to_dewey_modifiers(m: agpu::KeyModifiers) -> crate::event::KeyModifiers {
    let mut out = crate::event::KeyModifiers::empty();
    if m.contains(agpu::KeyModifiers::SHIFT) {
        out |= crate::event::KeyModifiers::SHIFT;
    }
    if m.contains(agpu::KeyModifiers::CONTROL) {
        out |= crate::event::KeyModifiers::CONTROL;
    }
    if m.contains(agpu::KeyModifiers::ALT) {
        out |= crate::event::KeyModifiers::ALT;
    }
    if m.contains(agpu::KeyModifiers::SUPER) {
        out |= crate::event::KeyModifiers::SUPER;
    }
    out
}

fn to_dewey_key_kind(k: agpu::KeyEventKind) -> crate::event::KeyEventKind {
    match k {
        agpu::KeyEventKind::Press => crate::event::KeyEventKind::Press,
        agpu::KeyEventKind::Release => crate::event::KeyEventKind::Release,
        agpu::KeyEventKind::Repeat => crate::event::KeyEventKind::Repeat,
    }
}

fn to_dewey_mouse_kind(k: agpu::MouseEventKind) -> crate::event::MouseEventKind {
    match k {
        agpu::MouseEventKind::Click(b) => {
            crate::event::MouseEventKind::Click(to_dewey_mouse_button(b))
        }
        agpu::MouseEventKind::Release(b) => {
            crate::event::MouseEventKind::Release(to_dewey_mouse_button(b))
        }
        agpu::MouseEventKind::Drag(b) => {
            crate::event::MouseEventKind::Drag(to_dewey_mouse_button(b))
        }
        agpu::MouseEventKind::Move => crate::event::MouseEventKind::Move,
        agpu::MouseEventKind::Scroll { delta_x, delta_y } => {
            crate::event::MouseEventKind::Scroll { delta_x, delta_y }
        }
    }
}

fn to_dewey_mouse_button(b: agpu::MouseButton) -> crate::event::MouseButton {
    match b {
        agpu::MouseButton::Left => crate::event::MouseButton::Left,
        agpu::MouseButton::Right => crate::event::MouseButton::Right,
        agpu::MouseButton::Middle => crate::event::MouseButton::Middle,
    }
}

fn to_dewey_drag_drop(d: agpu::DragDropEvent) -> crate::event::DragDropEvent {
    crate::event::DragDropEvent {
        kind: to_dewey_drag_kind(d.kind),
        position: Position::new(d.position.x, d.position.y),
    }
}

fn to_dewey_drag_kind(k: agpu::DragDropKind) -> crate::event::DragDropKind {
    match k {
        agpu::DragDropKind::DragStart { source_id, payload } => {
            crate::event::DragDropKind::DragStart {
                source_id,
                payload: to_dewey_payload(payload),
            }
        }
        agpu::DragDropKind::DragOver { target_id } => {
            crate::event::DragDropKind::DragOver { target_id }
        }
        agpu::DragDropKind::DragLeave { target_id } => {
            crate::event::DragDropKind::DragLeave { target_id }
        }
        agpu::DragDropKind::Drop {
            source_id,
            target_id,
            payload,
        } => crate::event::DragDropKind::Drop {
            source_id,
            target_id,
            payload: to_dewey_payload(payload),
        },
        agpu::DragDropKind::DragCancel => crate::event::DragDropKind::DragCancel,
    }
}

fn to_dewey_payload(p: agpu::DragPayload) -> crate::event::DragPayload {
    match p {
        agpu::DragPayload::Text(s) => crate::event::DragPayload::Text(s),
        agpu::DragPayload::Index(i) => crate::event::DragPayload::Index(i),
        agpu::DragPayload::Path(p) => crate::event::DragPayload::Path(p),
        agpu::DragPayload::Json(v) => crate::event::DragPayload::Json(v),
    }
}

// ── AgpuProgram ─────────────────────────────────────────────────────

/// Application runner for Dewey using the agpu GPU backend.
///
/// Drives a Dewey [`Model`] through a native winit event loop with
/// hardware-accelerated rendering via agpu's wgpu-based pipeline.
///
/// # Example
///
/// ```rust,no_run
/// use dewey::prelude::*;
/// use dewey::backend::agpu_backend::AgpuProgram;
///
/// # struct App;
/// # enum Msg {}
/// # impl Model for App {
/// #     type Msg = Msg;
/// #     fn update(&mut self, _: Msg) -> Command<Msg> { Command::None }
/// #     fn view(&self, _: &mut Frame<'_>) {}
/// #     fn handle_event(&self, _: Event) -> Option<Msg> { None }
/// # }
/// let app = App;
/// AgpuProgram::new(app).run().unwrap();
/// ```
pub struct AgpuProgram<M: Model> {
    model: M,
    options: ProgramOptions,
    backend: agpu::BackendPreference,
    msaa_samples: u32,
    plugins: PluginRegistry,
    profiling: bool,
}

impl<M: Model + 'static> AgpuProgram<M> {
    /// Create a new program with the given model.
    pub fn new(model: M) -> Self {
        Self {
            model,
            options: ProgramOptions::default(),
            backend: agpu::BackendPreference::PlatformDefault,
            msaa_samples: 4,
            plugins: PluginRegistry::new(),
            profiling: false,
        }
    }

    /// Override the default program options.
    pub fn with_options(mut self, options: ProgramOptions) -> Self {
        self.options = options;
        self
    }

    /// Set the GPU backend preference.
    pub fn with_backend(mut self, backend: agpu::BackendPreference) -> Self {
        self.backend = backend;
        self
    }

    /// Set the MSAA sample count (1 = no MSAA, 4 = default).
    pub fn with_msaa(mut self, samples: u32) -> Self {
        self.msaa_samples = samples;
        self
    }

    /// Register a plugin. Plugins are initialised before the first frame.
    pub fn with_plugin(mut self, plugin: impl crate::plugin::Plugin + 'static) -> Self {
        self.plugins.register(plugin);
        self
    }

    /// Enable frame profiling (update / layout / render timers).
    pub fn with_profiling(mut self, enabled: bool) -> Self {
        self.profiling = enabled;
        self
    }

    /// Run the application. Blocks until the window closes.
    pub fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        let event_loop = EventLoop::new()?;
        let mut handler = AppHandler::Uninitialised {
            model: self.model,
            options: self.options,
            backend: self.backend,
            msaa_samples: self.msaa_samples,
            plugins: self.plugins,
            profiling: self.profiling,
        };
        // wgpu 24.x has a Vulkan HAL bug where dropping the Surface
        // panics with "SurfaceSemaphores still in use". This is harmless
        // on shutdown so we catch it instead of crashing the process.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            event_loop.run_app(&mut handler)
        }));
        match result {
            Ok(r) => r?,
            Err(_) => {
                log::warn!("dewey-agpu: suppressed Vulkan surface cleanup panic on exit");
            }
        }
        Ok(())
    }
}

// ── Internal winit ApplicationHandler ───────────────────────────────

enum AppHandler<M: Model> {
    Uninitialised {
        model: M,
        options: ProgramOptions,
        backend: agpu::BackendPreference,
        msaa_samples: u32,
        plugins: PluginRegistry,
        profiling: bool,
    },
    Running(Box<RunningApp<M>>),
    Exited,
}

struct RunningApp<M: Model> {
    model: M,
    window: Arc<Window>,
    gpu: agpu::GpuContext,
    surface: agpu::Surface<'static>,
    surface_config: agpu::SurfaceConfiguration,
    surface_format: agpu::TextureFormat,
    shapes: agpu::ShapeRenderer,
    text: agpu::TextEngine,
    hit_map: HitMap,
    ontology: OntologyRegistry,
    /// When to build the ontology tree (see `ProgramOptions::ontology`).
    ontology_mode: crate::runtime::OntologyMode,
    plugins: PluginRegistry,
    profiler: Option<Profiler>,
    running: bool,
    cursor_position: Position,
    tick_rate: Option<Duration>,
    last_tick: Instant,
    msaa_texture: Option<agpu::TextureView>,
}

impl<M: Model + 'static> RunningApp<M> {
    fn new(
        model: M,
        options: &ProgramOptions,
        backend: agpu::BackendPreference,
        msaa_samples: u32,
        mut plugins: PluginRegistry,
        profiling: bool,
        event_loop: &ActiveEventLoop,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut attrs = Window::default_attributes()
            .with_title(model.title())
            .with_inner_size(winit::dpi::LogicalSize::new(
                options.width as f64,
                options.height as f64,
            ))
            .with_resizable(options.resizable)
            .with_transparent(options.transparent);

        if options.fullscreen {
            attrs = attrs.with_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
        }

        let window = Arc::new(event_loop.create_window(attrs)?);

        // Create wgpu instance with preferred backend
        let backends = backend.to_backends();
        let instance = agpu::Instance::new(&agpu::InstanceDescriptor {
            backends,
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone())?;
        let gpu = pollster::block_on(agpu::GpuContext::from_instance(instance, &surface, backend))?;

        let size = window.inner_size();
        let surface_caps = surface.get_capabilities(gpu.adapter());
        let format = surface_caps
            .formats
            .iter()
            .find(|f: &&agpu::TextureFormat| !f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let present_mode = if options.vsync {
            agpu::PresentMode::AutoVsync
        } else {
            agpu::PresentMode::AutoNoVsync
        };

        let surface_config = agpu::SurfaceConfiguration {
            usage: agpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 3,
        };
        surface.configure(gpu.device(), &surface_config);

        let shapes =
            agpu::ShapeRenderer::new(&gpu, format, options.width, options.height, msaa_samples);
        let text = agpu::TextEngine::with_msaa(&gpu, format, msaa_samples);

        let msaa_texture = if msaa_samples > 1 {
            Some(create_msaa_texture(
                gpu.device(),
                format,
                size.width.max(1),
                size.height.max(1),
                msaa_samples,
            ))
        } else {
            None
        };

        let mut ontology = OntologyRegistry::new();
        model.register_ontology(&mut ontology);

        // Initialise plugins
        let mut i18n = I18n::new("en");
        let mut theme = Theme::dark();
        {
            let mut ctx = PluginContext {
                ontology: &mut ontology,
                i18n: &mut i18n,
                theme: &mut theme,
            };
            plugins.init_all(&mut ctx);
        }

        let profiler = if profiling {
            Some(Profiler::default())
        } else {
            None
        };

        let init_cmd = model.init();

        let mut app = Self {
            model,
            window,
            gpu,
            surface,
            surface_config,
            surface_format: format,
            shapes,
            text,
            hit_map: HitMap::new(),
            ontology,
            ontology_mode: options.ontology,
            plugins,
            profiler,
            running: true,
            cursor_position: Position::ZERO,
            tick_rate: options.tick_rate,
            last_tick: Instant::now(),
            msaa_texture,
        };

        app.process_command(init_cmd);
        Ok(app)
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.surface_config.width = new_size.width;
            self.surface_config.height = new_size.height;
            self.surface
                .configure(self.gpu.device(), &self.surface_config);
            self.shapes
                .set_viewport(new_size.width as f32, new_size.height as f32);
            if self.shapes.sample_count() > 1 {
                self.msaa_texture = Some(create_msaa_texture(
                    self.gpu.device(),
                    self.surface_format,
                    new_size.width,
                    new_size.height,
                    self.shapes.sample_count(),
                ));
            }
        }
    }

    /// Ensure all in-flight GPU work retires before the surface is dropped.
    /// Without this wgpu 24.x panics in the Vulkan HAL because the
    /// swapchain's `SurfaceSemaphores` are still in use.
    fn shutdown(&mut self) {
        // Wait for all submitted work (including the last present).
        self.gpu.device().poll(agpu::Maintain::Wait);
        // Drop the MSAA texture view so the only remaining reference
        // to the surface format is the swapchain itself.
        self.msaa_texture = None;
    }

    fn render(&mut self) {
        if let Some(ref mut profiler) = self.profiler {
            profiler.begin_frame();
        }

        // Plugin per-frame hooks
        self.plugins.on_frame();

        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(agpu::SurfaceError::Lost | agpu::SurfaceError::Outdated) => {
                self.surface
                    .configure(self.gpu.device(), &self.surface_config);
                return;
            }
            Err(agpu::SurfaceError::OutOfMemory) => {
                log::error!("dewey-agpu: out of GPU memory");
                self.running = false;
                return;
            }
            Err(e) => {
                log::warn!("dewey-agpu: surface error: {e:?}");
                return;
            }
        };

        let view = frame
            .texture
            .create_view(&agpu::TextureViewDescriptor::default());

        // Begin frame
        self.shapes.begin_frame();
        self.hit_map.clear();

        let area = Rect::new(
            0.0,
            0.0,
            self.surface_config.width as f32,
            self.surface_config.height as f32,
        );

        // Run the Dewey model's view through our bridge painter
        if let Some(ref mut profiler) = self.profiler {
            profiler.start("render");
        }
        {
            let mut painter = AgpuBridgePainter::new(&mut self.shapes, &mut self.text);
            let mut dewey_frame = Frame::with_ontology(
                area,
                &mut self.hit_map,
                &mut painter,
                self.ontology_mode == crate::runtime::OntologyMode::EveryFrame,
            );
            self.model.view(&mut dewey_frame);

            let nodes = dewey_frame.take_nodes();
            if !nodes.is_empty() {
                let mut root = UiNode::new("root", SemanticRole::Container);
                root.children = nodes;
                self.ontology.set_tree(UiTree::new(root));
            }
        }
        if let Some(ref mut profiler) = self.profiler {
            profiler.stop("render");
        }

        // GPU render pass
        let mut encoder =
            self.gpu
                .device()
                .create_command_encoder(&agpu::CommandEncoderDescriptor {
                    label: Some("dewey_agpu_encoder"),
                });

        {
            let (target_view, resolve_target) = if let Some(msaa_view) = &self.msaa_texture {
                (msaa_view, Some(&view))
            } else {
                (&view, None)
            };
            let mut pass = encoder.begin_render_pass(&agpu::RenderPassDescriptor {
                label: Some("dewey_agpu_pass"),
                color_attachments: &[Some(agpu::RenderPassColorAttachment {
                    view: target_view,
                    resolve_target,
                    ops: agpu::Operations {
                        load: agpu::LoadOp::Clear(agpu::types::Color {
                            r: 0.12,
                            g: 0.12,
                            b: 0.14,
                            a: 1.0,
                        }),
                        store: agpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.shapes.flush(&self.gpu, &mut pass);
            self.text.flush(
                &self.gpu,
                &mut pass,
                self.surface_config.width,
                self.surface_config.height,
            );
        }

        self.gpu.queue().submit(std::iter::once(encoder.finish()));
        frame.present();
        self.text.trim();

        if let Some(ref mut profiler) = self.profiler {
            profiler.end_frame();
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
            Command::SetTickRate(d) => {
                self.tick_rate = Some(d);
            }
            Command::ExportOntology => {
                self.model.register_ontology(&mut self.ontology);
            }
            Command::AgentAction {
                agent_id,
                action,
                params,
            } => {
                // On demand: refresh the tree just before it is read, so the
                // frame loop never pays for it.
                if self.ontology_mode == crate::runtime::OntologyMode::OnDemand {
                    let area = crate::core::Rect::new(
                        0.0,
                        0.0,
                        self.surface_config.width as f32,
                        self.surface_config.height as f32,
                    );
                    let tree = crate::runtime::build_ontology_tree(&self.model, area);
                    self.ontology.set_tree(tree);
                }
                if let Some(node) = self.ontology.find_node(&agent_id) {
                    if let Err(e) =
                        self.ontology
                            .validate_action_params(&node.widget_type, &action, &params)
                    {
                        log::warn!("AgentAction validation failed for {agent_id}.{action}: {e}");
                        return;
                    }
                }
                log::debug!("AgentAction: {agent_id}.{action}({params})");
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

// ── ApplicationHandler ──────────────────────────────────────────────

impl<M: Model + 'static> ApplicationHandler for AppHandler<M> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let AppHandler::Uninitialised { .. } = self {
            let taken = std::mem::replace(self, AppHandler::Exited);
            if let AppHandler::Uninitialised {
                model,
                options,
                backend,
                msaa_samples,
                plugins,
                profiling,
            } = taken
            {
                match RunningApp::new(
                    model,
                    &options,
                    backend,
                    msaa_samples,
                    plugins,
                    profiling,
                    event_loop,
                ) {
                    Ok(app) => {
                        *self = AppHandler::Running(Box::new(app));
                    }
                    Err(e) => {
                        log::error!("dewey-agpu: failed to initialise: {e}");
                        event_loop.exit();
                    }
                }
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let app = match self {
            AppHandler::Running(app) => app.as_mut(),
            _ => return,
        };

        // Handle close: flush GPU and exit immediately to avoid
        // Vulkan surface-semaphore cleanup panic in wgpu 24.x.
        if matches!(event, WindowEvent::CloseRequested) {
            app.running = false;
            app.plugins.on_shutdown();
            app.shutdown();
            event_loop.exit();
            return;
        }

        // Handle resize before converting events
        if let WindowEvent::Resized(size) = event {
            app.resize(size);
        }

        // Track cursor position
        if let WindowEvent::CursorMoved { position, .. } = &event {
            app.cursor_position = Position::new(position.x as f32, position.y as f32);
        }

        // Convert via agpu's event converter, then bridge to Dewey events
        let agpu_events = agpu::convert_window_event(&event);
        for mut agpu_ev in agpu_events {
            // Patch mouse events with tracked cursor position
            if let agpu::Event::Mouse(ref mut mouse) = agpu_ev {
                if mouse.position == agpu::Position::ZERO {
                    mouse.position = to_agpu_pos(app.cursor_position);
                }
            }
            if let Some(dewey_ev) = to_dewey_event(agpu_ev) {
                if let Some(msg) = app.model.handle_event(dewey_ev) {
                    let cmd = app.model.update(msg);
                    app.process_command(cmd);
                }
            }
        }

        // Redraw
        if let WindowEvent::RedrawRequested = event {
            app.render();
        }

        if !app.running {
            app.plugins.on_shutdown();
            app.shutdown();
            event_loop.exit();
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let AppHandler::Running(app) = self {
            let now = Instant::now();
            match app.tick_rate {
                Some(rate) => {
                    if now.duration_since(app.last_tick) >= rate {
                        app.last_tick = now;
                        let tick_ev = crate::event::Event::Tick;
                        if let Some(msg) = app.model.handle_event(tick_ev) {
                            let cmd = app.model.update(msg);
                            app.process_command(cmd);
                        }
                        app.window.request_redraw();
                    }
                }
                None => {
                    app.window.request_redraw();
                }
            }
        }
    }
}

// ── MSAA helper ─────────────────────────────────────────────────────

fn create_msaa_texture(
    device: &agpu::Device,
    format: agpu::TextureFormat,
    width: u32,
    height: u32,
    sample_count: u32,
) -> agpu::TextureView {
    let texture = device.create_texture(&agpu::TextureDescriptor {
        label: Some("dewey_agpu_msaa"),
        size: agpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count,
        dimension: agpu::TextureDimension::D2,
        format,
        usage: agpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    texture.create_view(&agpu::TextureViewDescriptor::default())
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::rect::{Position, Rect};
    use crate::core::style::{Color, FontWeight, TextStyle};
    use crate::event;

    // ── Type conversion ─────────────────────────────────────────

    #[test]
    fn rect_round_trip() {
        let dewey = Rect::new(10.0, 20.0, 300.0, 400.0);
        let agpu_r = to_agpu_rect(dewey);
        assert_eq!(agpu_r.x, 10.0);
        assert_eq!(agpu_r.y, 20.0);
        assert_eq!(agpu_r.width, 300.0);
        assert_eq!(agpu_r.height, 400.0);
    }

    #[test]
    fn position_conversion() {
        let dewey = Position::new(5.5, 7.7);
        let agpu_p = to_agpu_pos(dewey);
        assert_eq!(agpu_p.x, 5.5);
        assert_eq!(agpu_p.y, 7.7);
    }

    #[test]
    fn color_conversion() {
        let dewey = Color::rgba(0.1, 0.2, 0.3, 0.4);
        let agpu_c = to_agpu_color(dewey);
        assert!((agpu_c.r - 0.1).abs() < f32::EPSILON);
        assert!((agpu_c.g - 0.2).abs() < f32::EPSILON);
        assert!((agpu_c.b - 0.3).abs() < f32::EPSILON);
        assert!((agpu_c.a - 0.4).abs() < f32::EPSILON);
    }

    #[test]
    fn font_weight_all_variants() {
        let pairs = [
            (FontWeight::Thin, agpu::FontWeight::Thin),
            (FontWeight::Light, agpu::FontWeight::Light),
            (FontWeight::Regular, agpu::FontWeight::Regular),
            (FontWeight::Medium, agpu::FontWeight::Medium),
            (FontWeight::SemiBold, agpu::FontWeight::SemiBold),
            (FontWeight::Bold, agpu::FontWeight::Bold),
            (FontWeight::ExtraBold, agpu::FontWeight::ExtraBold),
        ];
        for (dewey_w, expected) in pairs {
            assert_eq!(to_agpu_font_weight(dewey_w), expected);
        }
    }

    #[test]
    fn text_style_conversion() {
        let dewey = TextStyle {
            font_size: 16.0,
            color: Color::WHITE,
            weight: FontWeight::Bold,
            italic: true,
            underline: false,
            strikethrough: true,
            line_height: Some(1.5),
            letter_spacing: 0.5,
        };

        let agpu_s = to_agpu_text_style(&dewey);
        assert_eq!(agpu_s.font_size, 16.0);
        assert_eq!(agpu_s.weight, agpu::FontWeight::Bold);
        assert!(agpu_s.italic);
        assert!(!agpu_s.underline);
        assert!(agpu_s.strikethrough);
        assert_eq!(agpu_s.line_height, Some(1.5));
        assert!((agpu_s.letter_spacing - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn size_from_agpu() {
        let agpu_s = agpu::Size::new(640.0, 480.0);
        let dewey_s = from_agpu_size(agpu_s);
        assert_eq!(dewey_s.width, 640.0);
        assert_eq!(dewey_s.height, 480.0);
    }

    // ── Event conversion ────────────────────────────────────────

    #[test]
    fn key_event_conversion() {
        let agpu_ev = agpu::Event::Key(agpu::KeyEvent {
            code: agpu::KeyCode::Enter,
            modifiers: agpu::KeyModifiers::CONTROL | agpu::KeyModifiers::SHIFT,
            kind: agpu::KeyEventKind::Press,
        });

        let dewey_ev = to_dewey_event(agpu_ev).unwrap();
        match dewey_ev {
            event::Event::Key(k) => {
                assert_eq!(k.code, event::KeyCode::Enter);
                assert!(k.modifiers.contains(event::KeyModifiers::CONTROL));
                assert!(k.modifiers.contains(event::KeyModifiers::SHIFT));
                assert!(!k.modifiers.contains(event::KeyModifiers::ALT));
                assert_eq!(k.kind, event::KeyEventKind::Press);
            }
            _ => panic!("Expected Key event"),
        }
    }

    #[test]
    fn keycode_all_variants() {
        // Test each KeyCode variant individually
        let codes = [
            (agpu::KeyCode::Backspace, event::KeyCode::Backspace),
            (agpu::KeyCode::Enter, event::KeyCode::Enter),
            (agpu::KeyCode::Tab, event::KeyCode::Tab),
            (agpu::KeyCode::BackTab, event::KeyCode::BackTab),
            (agpu::KeyCode::Esc, event::KeyCode::Esc),
            (agpu::KeyCode::Left, event::KeyCode::Left),
            (agpu::KeyCode::Right, event::KeyCode::Right),
            (agpu::KeyCode::Up, event::KeyCode::Up),
            (agpu::KeyCode::Down, event::KeyCode::Down),
            (agpu::KeyCode::Home, event::KeyCode::Home),
            (agpu::KeyCode::End, event::KeyCode::End),
            (agpu::KeyCode::PageUp, event::KeyCode::PageUp),
            (agpu::KeyCode::PageDown, event::KeyCode::PageDown),
            (agpu::KeyCode::Insert, event::KeyCode::Insert),
            (agpu::KeyCode::Delete, event::KeyCode::Delete),
            (agpu::KeyCode::Null, event::KeyCode::Null),
            (agpu::KeyCode::Char('a'), event::KeyCode::Char('a')),
            (agpu::KeyCode::F(5), event::KeyCode::F(5)),
        ];
        for (agpu_k, expected) in codes {
            assert_eq!(to_dewey_keycode(agpu_k), expected);
        }
    }

    #[test]
    fn modifier_flags() {
        let all = agpu::KeyModifiers::SHIFT
            | agpu::KeyModifiers::CONTROL
            | agpu::KeyModifiers::ALT
            | agpu::KeyModifiers::SUPER;
        let dewey_m = to_dewey_modifiers(all);
        assert!(dewey_m.contains(event::KeyModifiers::SHIFT));
        assert!(dewey_m.contains(event::KeyModifiers::CONTROL));
        assert!(dewey_m.contains(event::KeyModifiers::ALT));
        assert!(dewey_m.contains(event::KeyModifiers::SUPER));
    }

    #[test]
    fn empty_modifiers() {
        let none = agpu::KeyModifiers::empty();
        let dewey_m = to_dewey_modifiers(none);
        assert!(dewey_m.is_empty());
    }

    #[test]
    fn key_event_kinds() {
        assert_eq!(
            to_dewey_key_kind(agpu::KeyEventKind::Press),
            event::KeyEventKind::Press
        );
        assert_eq!(
            to_dewey_key_kind(agpu::KeyEventKind::Release),
            event::KeyEventKind::Release
        );
        assert_eq!(
            to_dewey_key_kind(agpu::KeyEventKind::Repeat),
            event::KeyEventKind::Repeat
        );
    }

    #[test]
    fn mouse_event_conversion() {
        let agpu_ev = agpu::Event::Mouse(agpu::MouseEvent {
            kind: agpu::MouseEventKind::Click(agpu::MouseButton::Left),
            position: agpu::Position::new(100.0, 200.0),
            modifiers: agpu::KeyModifiers::empty(),
        });

        let dewey_ev = to_dewey_event(agpu_ev).unwrap();
        match dewey_ev {
            event::Event::Mouse(m) => {
                assert!(matches!(
                    m.kind,
                    event::MouseEventKind::Click(event::MouseButton::Left)
                ));
                assert_eq!(m.position.x, 100.0);
                assert_eq!(m.position.y, 200.0);
            }
            _ => panic!("Expected Mouse event"),
        }
    }

    #[test]
    fn mouse_buttons_all() {
        assert_eq!(
            to_dewey_mouse_button(agpu::MouseButton::Left),
            event::MouseButton::Left
        );
        assert_eq!(
            to_dewey_mouse_button(agpu::MouseButton::Right),
            event::MouseButton::Right
        );
        assert_eq!(
            to_dewey_mouse_button(agpu::MouseButton::Middle),
            event::MouseButton::Middle
        );
    }

    #[test]
    fn mouse_kinds_all() {
        // Click
        match to_dewey_mouse_kind(agpu::MouseEventKind::Click(agpu::MouseButton::Right)) {
            event::MouseEventKind::Click(event::MouseButton::Right) => {}
            other => panic!("Expected Click(Right), got {other:?}"),
        }
        // Release
        match to_dewey_mouse_kind(agpu::MouseEventKind::Release(agpu::MouseButton::Middle)) {
            event::MouseEventKind::Release(event::MouseButton::Middle) => {}
            other => panic!("Expected Release(Middle), got {other:?}"),
        }
        // Drag
        match to_dewey_mouse_kind(agpu::MouseEventKind::Drag(agpu::MouseButton::Left)) {
            event::MouseEventKind::Drag(event::MouseButton::Left) => {}
            other => panic!("Expected Drag(Left), got {other:?}"),
        }
        // Move
        assert!(matches!(
            to_dewey_mouse_kind(agpu::MouseEventKind::Move),
            event::MouseEventKind::Move
        ));
        // Scroll
        match to_dewey_mouse_kind(agpu::MouseEventKind::Scroll {
            delta_x: 1.0,
            delta_y: -2.0,
        }) {
            event::MouseEventKind::Scroll { delta_x, delta_y } => {
                assert_eq!(delta_x, 1.0);
                assert_eq!(delta_y, -2.0);
            }
            other => panic!("Expected Scroll, got {other:?}"),
        }
    }

    #[test]
    fn simple_events() {
        assert!(matches!(
            to_dewey_event(agpu::Event::FocusGained).unwrap(),
            event::Event::FocusGained
        ));
        assert!(matches!(
            to_dewey_event(agpu::Event::FocusLost).unwrap(),
            event::Event::FocusLost
        ));
        assert!(matches!(
            to_dewey_event(agpu::Event::CloseRequested).unwrap(),
            event::Event::CloseRequested
        ));
        assert!(matches!(
            to_dewey_event(agpu::Event::Tick).unwrap(),
            event::Event::Tick
        ));
        assert!(matches!(
            to_dewey_event(agpu::Event::FileHoverCancelled).unwrap(),
            event::Event::FileHoverCancelled
        ));
    }

    #[test]
    fn text_input_event() {
        let agpu_ev = agpu::Event::TextInput("hello".into());
        match to_dewey_event(agpu_ev).unwrap() {
            event::Event::TextInput(s) => assert_eq!(s, "hello"),
            _ => panic!("Expected TextInput"),
        }
    }

    #[test]
    fn resize_event() {
        let agpu_ev = agpu::Event::Resize(agpu::Size::new(1920.0, 1080.0));
        match to_dewey_event(agpu_ev).unwrap() {
            event::Event::Resize(s) => {
                assert_eq!(s.width, 1920.0);
                assert_eq!(s.height, 1080.0);
            }
            _ => panic!("Expected Resize"),
        }
    }

    #[test]
    fn file_drop_events() {
        let files = vec!["a.txt".into(), "b.rs".into()];
        match to_dewey_event(agpu::Event::FileDrop(files.clone())).unwrap() {
            event::Event::FileDrop(f) => assert_eq!(f, files),
            _ => panic!("Expected FileDrop"),
        }
        match to_dewey_event(agpu::Event::FileHover(files.clone())).unwrap() {
            event::Event::FileHover(f) => assert_eq!(f, files),
            _ => panic!("Expected FileHover"),
        }
    }

    #[test]
    fn agpu_only_events_are_skipped() {
        assert!(
            to_dewey_event(agpu::Event::AgentAction {
                agent_id: "x".into(),
                action: "click".into(),
                params: serde_json::Value::Null,
            })
            .is_none()
        );
        assert!(to_dewey_event(agpu::Event::ImeCommit("hi".into())).is_none());
        assert!(
            to_dewey_event(agpu::Event::ImePreedit {
                text: "hi".into(),
                cursor: None,
            })
            .is_none()
        );
    }

    #[test]
    fn drag_drop_event() {
        let dd = agpu::DragDropEvent {
            kind: agpu::DragDropKind::DragStart {
                source_id: "widget_1".into(),
                payload: agpu::DragPayload::Text("data".into()),
            },
            position: agpu::Position::new(50.0, 60.0),
        };
        let dewey_dd = to_dewey_drag_drop(dd);
        assert_eq!(dewey_dd.position.x, 50.0);
        assert_eq!(dewey_dd.position.y, 60.0);
        match dewey_dd.kind {
            event::DragDropKind::DragStart {
                source_id, payload, ..
            } => {
                assert_eq!(source_id, "widget_1");
                match payload {
                    event::DragPayload::Text(t) => assert_eq!(t, "data"),
                    _ => panic!("Expected Text payload"),
                }
            }
            _ => panic!("Expected DragStart"),
        }
    }

    #[test]
    fn drag_drop_all_kinds() {
        // DragOver
        match to_dewey_drag_kind(agpu::DragDropKind::DragOver {
            target_id: "t".into(),
        }) {
            event::DragDropKind::DragOver { target_id } => assert_eq!(target_id, "t"),
            _ => panic!("Expected DragOver"),
        }
        // DragLeave
        match to_dewey_drag_kind(agpu::DragDropKind::DragLeave {
            target_id: "t".into(),
        }) {
            event::DragDropKind::DragLeave { target_id } => assert_eq!(target_id, "t"),
            _ => panic!("Expected DragLeave"),
        }
        // DragCancel
        assert!(matches!(
            to_dewey_drag_kind(agpu::DragDropKind::DragCancel),
            event::DragDropKind::DragCancel
        ));
        // Drop
        match to_dewey_drag_kind(agpu::DragDropKind::Drop {
            source_id: "s".into(),
            target_id: "t".into(),
            payload: agpu::DragPayload::Index(42),
        }) {
            event::DragDropKind::Drop {
                source_id,
                target_id,
                payload,
            } => {
                assert_eq!(source_id, "s");
                assert_eq!(target_id, "t");
                assert!(matches!(payload, event::DragPayload::Index(42)));
            }
            _ => panic!("Expected Drop"),
        }
    }

    #[test]
    fn payload_all_variants() {
        match to_dewey_payload(agpu::DragPayload::Text("hi".into())) {
            event::DragPayload::Text(s) => assert_eq!(s, "hi"),
            _ => panic!("Expected Text"),
        }
        match to_dewey_payload(agpu::DragPayload::Index(7)) {
            event::DragPayload::Index(i) => assert_eq!(i, 7),
            _ => panic!("Expected Index"),
        }
        match to_dewey_payload(agpu::DragPayload::Path(vec![
            "tmp".into(),
            "file.txt".into(),
        ])) {
            event::DragPayload::Path(p) => assert_eq!(p, vec!["tmp", "file.txt"]),
            _ => panic!("Expected Path"),
        }
        let json = serde_json::json!({"key": "value"});
        match to_dewey_payload(agpu::DragPayload::Json(json.clone())) {
            event::DragPayload::Json(v) => assert_eq!(v, json),
            _ => panic!("Expected Json"),
        }
    }

    // ── AgpuProgram builder ─────────────────────────────────────

    #[test]
    fn program_default_options() {
        struct Dummy;
        #[derive(Debug)]
        enum DMsg {}
        impl Model for Dummy {
            type Msg = DMsg;
            fn update(&mut self, _: DMsg) -> Command<DMsg> {
                Command::None
            }
            fn view(&self, _: &mut Frame<'_>) {}
            fn handle_event(&self, _: event::Event) -> Option<DMsg> {
                None
            }
        }

        let prog = AgpuProgram::new(Dummy);
        assert_eq!(prog.msaa_samples, 4);
        assert!(!prog.profiling);
        assert!(matches!(
            prog.backend,
            agpu::BackendPreference::PlatformDefault
        ));
    }

    #[test]
    fn program_builder_chain() {
        struct Dummy;
        #[derive(Debug)]
        enum DMsg {}
        impl Model for Dummy {
            type Msg = DMsg;
            fn update(&mut self, _: DMsg) -> Command<DMsg> {
                Command::None
            }
            fn view(&self, _: &mut Frame<'_>) {}
            fn handle_event(&self, _: event::Event) -> Option<DMsg> {
                None
            }
        }

        let opts = ProgramOptions {
            width: 1024.0,
            height: 768.0,
            vsync: false,
            fullscreen: true,
            transparent: true,
            ..Default::default()
        };

        let prog = AgpuProgram::new(Dummy)
            .with_options(opts)
            .with_backend(agpu::BackendPreference::OpenGLPreferred)
            .with_msaa(8)
            .with_profiling(true);

        assert_eq!(prog.msaa_samples, 8);
        assert!(prog.profiling);
        assert!(matches!(
            prog.backend,
            agpu::BackendPreference::OpenGLPreferred
        ));
        assert_eq!(prog.options.width, 1024.0);
        assert_eq!(prog.options.height, 768.0);
        assert!(!prog.options.vsync);
        assert!(prog.options.fullscreen);
        assert!(prog.options.transparent);
    }
}
