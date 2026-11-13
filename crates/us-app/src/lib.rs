//! Urban Sprawl app: window, menus, camera and the playable town. The desktop binary is a thin
//! wrapper over [`run`]; other platforms start the game through their own entry point.

mod cam;
pub(crate) mod game;
mod platform;
mod theme;
mod world;

use game::{State, touch};
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Fullscreen, Window, WindowId};

pub const VERSION_LABEL: &str = "1.0.0 preview 2";

struct Gfx {
    window: Arc<Window>,
    renderer: us_render::Renderer,
    egui_ctx: egui::Context,
    egui_state: egui_winit::State,
    egui_rend: egui_wgpu::Renderer,
    /// egui texture changes (the font atlas among them) not yet uploaded. A frame the surface
    /// skips keeps them here for the next one, so no glyph update is ever lost.
    pending_tex: egui::TexturesDelta,
}

struct App {
    gfx: Option<Gfx>,
    /// The window outlives the graphics: when the app goes to the background the graphics are
    /// dropped (iOS takes the surface away) and rebuilt on the way back.
    window: Option<Arc<Window>>,
    st: State,
    last: Instant,
    fatal: Option<String>,
    applied_scale: f32,
    logged_frames: u32,
    applied_fullscreen: bool,
}

pub(crate) fn log_path() -> std::path::PathBuf {
    std::env::temp_dir().join("urban-sprawl.log")
}

fn log(msg: &str) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path())
    {
        let _ = writeln!(f, "{msg}");
    }
    eprintln!("{msg}");
}

impl App {
    fn init_gfx(&mut self, el: &ActiveEventLoop) {
        let attrs =
            Window::default_attributes().with_title(format!("Urban Sprawl {VERSION_LABEL}"));
        // desktop windows open at a set size; a phone or tablet window is the whole screen
        #[cfg(not(target_os = "ios"))]
        let attrs = attrs
            .with_inner_size(winit::dpi::LogicalSize::new(1360.0, 820.0))
            .with_min_inner_size(winit::dpi::LogicalSize::new(900.0, 560.0));
        #[cfg(target_os = "ios")]
        let attrs = {
            use winit::platform::ios::WindowAttributesExtIOS;
            attrs
                .with_prefers_status_bar_hidden(true)
                .with_prefers_home_indicator_hidden(true)
        };
        let window = match self.window.clone() {
            Some(w) => w,
            None => match el.create_window(attrs) {
                Ok(w) => Arc::new(w),
                Err(e) => {
                    self.fatal = Some(format!("could not open a window: {e}"));
                    el.exit();
                    return;
                }
            },
        };
        self.window = Some(window.clone());
        let renderer = match us_render::Renderer::for_window_handle(window.clone()) {
            Ok(r) => r,
            Err(e) => {
                self.fatal = Some(format!("{e}"));
                el.exit();
                return;
            }
        };
        let a = renderer.adapter();
        log(&format!(
            "graphics: {} ({}, {})",
            a.name, a.backend, a.device_type
        ));
        let egui_ctx = egui::Context::default();
        theme::install(&egui_ctx);
        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &*window,
            Some(window.scale_factor() as f32),
            None,
            Some(renderer.device().limits().max_texture_dimension_2d as usize),
        );
        let format = renderer.surface_format().expect("window surface");
        let egui_rend = egui_wgpu::Renderer::new(
            renderer.device(),
            format,
            egui_wgpu::RendererOptions::default(),
        );
        window.request_redraw();
        self.gfx = Some(Gfx {
            window,
            renderer,
            egui_ctx,
            egui_state,
            egui_rend,
            pending_tex: egui::TexturesDelta::default(),
        });
    }

    fn redraw(&mut self, el: &ActiveEventLoop) {
        let Some(gfx) = self.gfx.as_mut() else { return };
        let size = gfx.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        let now = Instant::now();
        let dt = now
            .duration_since(self.last)
            .as_secs_f32()
            .clamp(0.0001, 0.1);
        self.last = now;

        if (self.st.settings.ui_scale - self.applied_scale).abs() > 1e-3 {
            gfx.egui_ctx.set_zoom_factor(self.st.settings.ui_scale);
            self.applied_scale = self.st.settings.ui_scale;
        }
        if self.st.settings.fullscreen != self.applied_fullscreen {
            gfx.window.set_fullscreen(
                self.st
                    .settings
                    .fullscreen
                    .then(|| Fullscreen::Borderless(None)),
            );
            self.applied_fullscreen = self.st.settings.fullscreen;
        }

        let (w, h) = (size.width, size.height);
        self.st.touch_tick();
        self.st.update(dt, w, h);

        let mut raw = gfx.egui_state.take_egui_input(&gfx.window);
        // keep the interface out from under the notch, rounded corners and home indicator
        let [top, left, bottom, right] = platform::safe_insets(&gfx.window);
        if top + left + bottom + right > 0.0 {
            let k = 1.0 / self.st.settings.ui_scale.max(0.1);
            raw.safe_area_insets = Some(egui::SafeAreaInsets(egui::epaint::MarginF32 {
                top: top * k,
                left: left * k,
                bottom: bottom * k,
                right: right * k,
            }));
        }
        if self.logged_frames < 3 {
            self.logged_frames += 1;
            log(&format!(
                "frame: window {}x{} px, scale {}, egui screen {:?}, ppp {}",
                w,
                h,
                gfx.window.scale_factor(),
                raw.screen_rect,
                gfx.egui_ctx.pixels_per_point()
            ));
        }
        let st = &mut self.st;
        let out = gfx.egui_ctx.run_ui(raw, |ui| st.ui(ui, w, h));
        gfx.egui_state
            .handle_platform_output(&gfx.window, out.platform_output);
        let ppp = out.pixels_per_point;
        let prims = gfx.egui_ctx.tessellate(out.shapes, ppp);
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [w, h],
            pixels_per_point: ppp,
        };
        let (scene, camera, style) = self.st.scene_camera();
        gfx.pending_tex.append(out.textures_delta);
        let egui_rend = &mut gfx.egui_rend;
        let textures = &gfx.pending_tex;
        let mut uploaded = false;
        let stats = gfx.renderer.render_to_surface_with(
            scene,
            &camera,
            &style,
            w,
            h,
            |device, queue, enc, view| {
                for (id, deltas) in &textures.set {
                    for delta in deltas {
                        egui_rend.update_texture(device, queue, *id, delta);
                    }
                }
                uploaded = true;
                let extra = egui_rend.update_buffers(device, queue, enc, &prims, &screen);
                if !extra.is_empty() {
                    queue.submit(extra);
                }
                let mut pass = enc
                    .begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("ui"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    })
                    .forget_lifetime();
                egui_rend.render(&mut pass, &prims, &screen);
            },
        );
        if uploaded {
            for id in &gfx.pending_tex.free {
                gfx.egui_rend.free_texture(id);
            }
            gfx.pending_tex.clear();
        }
        self.st.frame_done(dt, stats.is_some());
        if self.st.quit {
            el.exit();
        }
        gfx.window.request_redraw();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gfx.is_none() {
            self.last = Instant::now();
            self.init_gfx(el);
        }
    }

    fn suspended(&mut self, _el: &ActiveEventLoop) {
        // Backgrounded: stop drawing and let go of the graphics. The game stands still because
        // it only advances when a frame is drawn.
        self.gfx = None;
        self.st.release_all();
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if matches!(event, WindowEvent::RedrawRequested) {
            self.redraw(el);
            return;
        }
        let Some(gfx) = self.gfx.as_mut() else { return };
        let resp = gfx.egui_state.on_window_event(&gfx.window, &event);
        let st = &mut self.st;
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::CursorMoved { position, .. } => {
                st.cursor_moved(
                    position.x as f32,
                    position.y as f32,
                    gfx.window.inner_size().height,
                );
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let down = state == ElementState::Pressed;
                match button {
                    MouseButton::Left => st.mouse_button(true, down, resp.consumed),
                    MouseButton::Right => st.mouse_button(false, down, resp.consumed),
                    _ => {}
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if !resp.consumed {
                    let lines = match delta {
                        MouseScrollDelta::LineDelta(_, y) => y,
                        MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                    };
                    st.wheel(lines);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    let down = event.state == ElementState::Pressed;
                    if down && code == KeyCode::F11 {
                        st.settings.fullscreen = !st.settings.fullscreen;
                    }
                    st.key(code, down, event.repeat, resp.consumed);
                }
            }
            WindowEvent::Touch(t) => {
                let phase = match t.phase {
                    TouchPhase::Started => touch::Phase::Down,
                    TouchPhase::Moved => touch::Phase::Move,
                    TouchPhase::Ended => touch::Phase::Up,
                    TouchPhase::Cancelled => touch::Phase::Cancel,
                };
                // Does the finger land on a button or panel? Decided from the last frame's layout.
                let ppp = gfx.egui_ctx.pixels_per_point();
                let at = egui::pos2(t.location.x as f32 / ppp, t.location.y as f32 / ppp);
                let on_ui = phase == touch::Phase::Down
                    && (gfx
                        .egui_ctx
                        .layer_id_at(at)
                        .is_some_and(|l| l.order != egui::Order::Background)
                        || gfx
                            .egui_ctx
                            .interactive_rects_last_pass()
                            .iter()
                            .any(|r| r.expand(4.0).contains(at)));
                st.touch(
                    t.id,
                    phase,
                    t.location.x as f32,
                    t.location.y as f32,
                    on_ui,
                    gfx.window.inner_size().height,
                );
            }
            WindowEvent::Focused(false) => st.release_all(),
            _ => {}
        }
        if resp.repaint {
            gfx.window.request_redraw();
        }
    }

    fn about_to_wait(&mut self, _el: &ActiveEventLoop) {
        if let Some(g) = &self.gfx {
            g.window.request_redraw();
        }
    }
}

/// Start the game and run until the player quits. The desktop binary calls this from `main`.
pub fn run() {
    std::panic::set_hook(Box::new(|info| {
        log(&format!("panic: {info}"));
        // Only the main thread ends the game; a failed background task (for example the
        // launcher sign-in) must never pop up a dialog while the game keeps running.
        if std::thread::current().name() == Some("main") {
            platform::tell_user(&format!("Something went wrong: {info}"));
        }
    }));
    let el = match EventLoop::new() {
        Ok(e) => e,
        Err(e) => {
            log(&format!("could not start the window system: {e}"));
            platform::tell_user(&format!("The window system did not start: {e}"));
            std::process::exit(1);
        }
    };
    el.set_control_flow(ControlFlow::Poll);
    let mut app = App {
        gfx: None,
        window: None,
        st: State::new(),
        last: Instant::now(),
        fatal: None,
        applied_scale: 1.0,
        logged_frames: 0,
        applied_fullscreen: false,
    };
    if let Err(e) = el.run_app(&mut app) {
        log(&format!("event loop error: {e}"));
    }
    if let Some(f) = app.fatal {
        log(&format!("Urban Sprawl could not start: {f}"));
        platform::tell_user(&format!(
            "The graphics could not start. Update your graphics driver.\n\
             On Linux, install the Mesa Vulkan driver (package mesa-vulkan-drivers).\n\n\
             Technical detail: {f}"
        ));
        std::process::exit(1);
    }
}
