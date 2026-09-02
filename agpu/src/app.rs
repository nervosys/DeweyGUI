//! Application runner — the agpu equivalent of eframe's `run_native`.
//!
//! Uses winit for windowing and event dispatch, wgpu for GPU rendering,
//! and integrates the agpu `Model` trait to drive the Elm architecture.
//!
//! Every layer is agent-discoverable via the ontology.

use crate::context::GpuContext;
use crate::core::{Position, Rect};
use crate::event::{HitMap, convert_window_event};
use crate::ontology::{OntologyRegistry, SemanticRole, UiNode, UiTree};
use crate::painter::AgpuPainter;
use crate::renderer::ShapeRenderer;
use crate::runtime::{Command, Frame, Model, ProgramOptions, Subscription};
use crate::text::TextEngine;
use crate::types::BackendPreference;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

/// The agpu application runner.
///
/// Wraps an agpu `Model` and drives it with wgpu rendering and winit events.
pub struct AgpuApp<M: Model> {
    model: M,
    options: ProgramOptions,
}

impl<M: Model + 'static> AgpuApp<M> {
    /// Create a new application with the given model.
    pub fn new(model: M) -> Self {
        Self {
            model,
            options: ProgramOptions::default(),
        }
    }

    /// Override the default program options.
    pub fn with_options(mut self, options: ProgramOptions) -> Self {
        self.options = options;
        self
    }

    /// Set the GPU backend preference.
    pub fn with_backend(mut self, preference: BackendPreference) -> Self {
        self.options.backend = preference;
        self
    }

    /// Run the application. Blocks until the window closes.
    pub fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        let event_loop = EventLoop::new()?;
        event_loop.set_control_flow(ControlFlow::Poll);

        let mut handler = AppHandler::Uninitialised {
            model: self.model,
            options: self.options,
        };

        event_loop.run_app(&mut handler)?;
        Ok(())
    }
}

// ── Internal handler ────────────────────────────────────────────────

/// Internal state machine — uninitialised until the first `resumed` call
/// because wgpu/winit require an active event loop to create the window.
enum AppHandler<M: Model> {
    Uninitialised {
        model: M,
        options: ProgramOptions,
    },
    Running(Box<RunningApp<M>>),
    /// Sentinel after an error or close.
    Exited,
}

struct RunningApp<M: Model> {
    model: M,
    window: Arc<Window>,
    gpu: GpuContext,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    surface_format: wgpu::TextureFormat,
    shapes: ShapeRenderer,
    text: TextEngine,
    hit_map: HitMap,
    ontology: OntologyRegistry,
    running: bool,
    cursor_position: Position,
    tick_rate: Option<Duration>,
    last_tick: Instant,
    msaa_texture: Option<wgpu::TextureView>,
    active_timers: Vec<(String, Duration, Instant)>,
    pending_delays: Vec<(String, Instant)>,
}

impl<M: Model + 'static> RunningApp<M> {
    fn new(
        model: M,
        options: &ProgramOptions,
        event_loop: &ActiveEventLoop,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let attrs = Window::default_attributes()
            .with_title(model.title())
            .with_inner_size(winit::dpi::LogicalSize::new(
                options.width as f64,
                options.height as f64,
            ))
            .with_resizable(options.resizable);

        let window = Arc::new(event_loop.create_window(attrs)?);

        // Create a temporary instance to create the surface,
        // then let GpuContext handle backend selection with fallback.
        let preference = options.backend;
        let backends = preference.to_backends();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends,
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone())?;

        let gpu = pollster::block_on(GpuContext::new(&surface, preference))?;

        let size = window.inner_size();
        let surface_caps = surface.get_capabilities(gpu.adapter());
        let format = surface_caps
            .formats
            .iter()
            .find(|f| !f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let present_mode = if options.vsync {
            wgpu::PresentMode::AutoVsync
        } else {
            wgpu::PresentMode::AutoNoVsync
        };

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 3,
        };
        surface.configure(gpu.device(), &surface_config);

        let shapes = ShapeRenderer::new(
            &gpu,
            format,
            options.width,
            options.height,
            options.msaa_samples,
        );
        let text = TextEngine::new(&gpu, format);

        let msaa_texture = if options.msaa_samples > 1 {
            Some(create_msaa_texture(
                gpu.device(),
                format,
                size.width.max(1),
                size.height.max(1),
                options.msaa_samples,
            ))
        } else {
            None
        };

        let mut ontology = OntologyRegistry::new();
        // The GPU's own schemas. These types exist, implement `Discoverable`
        // and are exported in the prelude, and nothing had ever registered
        // one — so an agent querying an agpu application was told about
        // whatever the application registered and nothing about the renderer,
        // the pipeline or the surface it was running on. The crate's stated
        // difference from wgpu is that all of it is discoverable.
        crate::ontology::gpu::register_gpu_ontology(&mut ontology);
        model.register_ontology(&mut ontology);

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
            running: true,
            cursor_position: Position::ZERO,
            tick_rate: options.tick_rate,
            last_tick: Instant::now(),
            msaa_texture,
            active_timers: Vec::new(),
            pending_delays: Vec::new(),
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

    fn render(&mut self) {
        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface
                    .configure(self.gpu.device(), &self.surface_config);
                return;
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                log::error!("agpu: out of GPU memory");
                self.running = false;
                return;
            }
            Err(e) => {
                log::warn!("agpu: surface error: {e:?}");
                return;
            }
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Begin frame
        self.shapes.begin_frame();
        self.hit_map.clear();

        let area = Rect::new(
            0.0,
            0.0,
            self.surface_config.width as f32,
            self.surface_config.height as f32,
        );

        // Run model view
        {
            let mut painter = AgpuPainter::new(&mut self.shapes, &mut self.text);
            let mut frame = Frame::new(area, &mut self.hit_map, &mut painter);
            self.model.view(&mut frame);

            let nodes = frame.take_nodes();
            if !nodes.is_empty() {
                let mut root = UiNode::new("root", SemanticRole::Container);
                root.children = nodes;
                self.ontology.set_tree(UiTree::new(root));
            }
        }

        // GPU render pass
        let mut encoder =
            self.gpu
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("agpu_encoder"),
                });

        {
            let (target_view, resolve_target) = if let Some(msaa_view) = &self.msaa_texture {
                (msaa_view, Some(&view))
            } else {
                (&view, None)
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("agpu_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
                    resolve_target,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.08,
                            g: 0.08,
                            b: 0.10,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
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
                // Validate params against the ontology schema if a node exists
                if let Some(node) = self.ontology.find_node(&agent_id) {
                    if let Err(e) =
                        self.ontology
                            .validate_action_params(&node.widget_type, &action, &params)
                    {
                        log::warn!("AgentAction validation failed for {agent_id}.{action}: {e}");
                        return;
                    }
                }
                // Dispatch to the model as an event
                let ev = crate::event::Event::AgentAction {
                    agent_id,
                    action,
                    params,
                };
                if let Some(msg) = self.model.handle_event(ev) {
                    let cmd = self.model.update(msg);
                    self.process_command(cmd);
                }
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

    fn refresh_subscriptions(&mut self) {
        let subs = self.model.subscriptions();
        // Reconcile active timers
        let new_ids: Vec<String> = subs
            .iter()
            .filter_map(|s| match s {
                Subscription::Timer { id, .. } => Some(id.clone()),
                _ => None,
            })
            .collect();
        // Remove timers no longer subscribed
        self.active_timers.retain(|(id, _, _)| new_ids.contains(id));
        // Add new timers
        for sub in &subs {
            if let Subscription::Timer { id, interval, .. } = sub {
                if !self.active_timers.iter().any(|(aid, _, _)| aid == id) {
                    self.active_timers
                        .push((id.clone(), *interval, Instant::now()));
                }
            }
        }
        // Add new delays
        for sub in subs {
            if let Subscription::Delay { id, duration, .. } = &sub {
                if !self.pending_delays.iter().any(|(did, _)| did == id) {
                    self.pending_delays
                        .push((id.clone(), Instant::now() + *duration));
                }
            }
        }
    }
}

// ── winit ApplicationHandler ────────────────────────────────────────

impl<M: Model + 'static> ApplicationHandler for AppHandler<M> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Initialise on first resume
        if let AppHandler::Uninitialised { .. } = self {
            let taken = std::mem::replace(self, AppHandler::Exited);
            if let AppHandler::Uninitialised { model, options } = taken {
                match RunningApp::new(model, &options, event_loop) {
                    Ok(app) => {
                        *self = AppHandler::Running(Box::new(app));
                    }
                    Err(e) => {
                        log::error!("agpu: failed to initialise: {e}");
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

        // Handle resize before converting events
        if let WindowEvent::Resized(size) = event {
            app.resize(size);
        }

        // Track cursor position
        if let WindowEvent::CursorMoved { position, .. } = &event {
            app.cursor_position = Position::new(position.x as f32, position.y as f32);
        }

        // Convert and dispatch events
        let events = convert_window_event(&event);
        for mut ev in events {
            // Patch mouse events with current cursor position
            if let crate::event::Event::Mouse(ref mut mouse) = ev {
                if mouse.position == Position::ZERO {
                    mouse.position = app.cursor_position;
                }
            }
            if let Some(msg) = app.model.handle_event(ev) {
                let cmd = app.model.update(msg);
                app.process_command(cmd);
            }
        }

        // Redraw
        if let WindowEvent::RedrawRequested = event {
            app.render();
        }

        if !app.running {
            event_loop.exit();
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let AppHandler::Running(app) = self {
            let now = Instant::now();

            // Tick subscriptions: check active timers
            let mut fired_timer_ids = Vec::new();
            for (id, interval, last_fire) in &mut app.active_timers {
                if now.duration_since(*last_fire) >= *interval {
                    *last_fire = now;
                    fired_timer_ids.push(id.clone());
                }
            }
            if !fired_timer_ids.is_empty() {
                // Re-fetch subscriptions to get message factories
                let subs = app.model.subscriptions();
                for sub in &subs {
                    if let Subscription::Timer { id, msg, .. } = sub {
                        if fired_timer_ids.contains(id) {
                            let m = msg();
                            let cmd = app.model.update(m);
                            app.process_command(cmd);
                        }
                    }
                }
            }

            // Check pending one-shot delays
            let mut fired_delays = Vec::new();
            app.pending_delays.retain(|(id, deadline)| {
                if now >= *deadline {
                    fired_delays.push(id.clone());
                    false
                } else {
                    true
                }
            });
            if !fired_delays.is_empty() {
                let subs = app.model.subscriptions();
                for sub in subs {
                    if let Subscription::Delay { id, msg, .. } = sub {
                        if fired_delays.contains(&id) {
                            let cmd = app.model.update(msg);
                            app.process_command(cmd);
                        }
                    }
                }
            }

            // Main tick
            match app.tick_rate {
                Some(rate) => {
                    if now.duration_since(app.last_tick) >= rate {
                        app.last_tick = now;
                        if let Some(msg) = app.model.handle_event(crate::event::Event::Tick) {
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

            // Refresh subscription state after updates
            app.refresh_subscriptions();
        }
    }
}

/// Create a multisampled texture view for MSAA rendering.
fn create_msaa_texture(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    sample_count: u32,
) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("agpu_msaa_texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}
