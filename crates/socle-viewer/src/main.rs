#![recursion_limit = "256"]

mod camera;
mod prepare;
mod renderer;
mod vertex;

use std::sync::Arc;
use std::time::Instant;

use camera::FlyCamera;
use orkester::ThreadPool;
use prepare::WgpuPreparer;
use renderer::Renderer;
use selekt::AllVisibleLruPolicy;
use terra::Ellipsoid;
use tiles3d_selekt::TilesetBuilder;
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

const FOV_Y: f64 = std::f64::consts::FRAC_PI_4; // 45°
const NEAR: f64 = 1.0;
const FAR: f64 = 1e9;

struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    tileset: Option<tiles3d_selekt::Tileset<Vec<prepare::GpuTile>>>,
    globe_tileset: Option<tiles3d_selekt::Tileset<Vec<prepare::GpuTile>>>,
    camera: FlyCamera,
    ellipsoid: Ellipsoid,
    work_queue: orkester::WorkQueue,
    thread_pool: ThreadPool,
    // Input state
    keys: Keys,
    left_down: bool,
    right_down: bool,
    last_frame: Instant,
    tileset_path: Option<String>,
    /// Set to true after the camera has been auto-positioned above the tileset.
    camera_positioned: bool,
    no_cull: bool,
}

#[derive(Default)]
struct Keys {
    w: bool,
    a: bool,
    s: bool,
    d: bool,
    q: bool,
    e: bool,
}

impl App {
    fn new(tileset_path: Option<String>, no_cull: bool) -> Self {
        let ellipsoid = Ellipsoid::wgs84();
        let camera = FlyCamera::default(&ellipsoid);
        let work_queue = orkester::WorkQueue::new();
        let thread_pool = ThreadPool::new(4);
        Self {
            window: None,
            renderer: None,
            tileset: None,
            camera,
            ellipsoid,
            work_queue,
            thread_pool,
            keys: Keys::default(),
            left_down: false,
            right_down: false,
            last_frame: Instant::now(),
            tileset_path,
            camera_positioned: false,
            no_cull,
            globe_tileset: None,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("socle-viewer")
                        .with_inner_size(winit::dpi::LogicalSize::new(1280u32, 720u32)),
                )
                .expect("create window"),
        );
        self.window = Some(Arc::clone(&window));

        let renderer = pollster::block_on(Renderer::new(Arc::clone(&window)));
        let device = Arc::clone(&renderer.device);
        let queue = Arc::clone(&renderer.queue);
        let texture_layout = Arc::clone(&renderer.texture_layout);

        let preparer = Arc::new(WgpuPreparer::new(device, queue, texture_layout));
        let bg_ctx = self.thread_pool.context();
        let main_ctx = self.work_queue.context();

        // Globe tileset — shares the same thread pool and work queue
        let globe_accessor: Arc<dyn orkester_io::AssetAccessor> =
            Arc::new(orkester_io::FileAccessor::new(bg_ctx.clone()));
        self.globe_tileset = Some(
            TilesetBuilder::ellipsoid(self.ellipsoid.clone())
                .with_main_context(main_ctx.clone())
                .build(bg_ctx.clone(), globe_accessor, Arc::clone(&preparer)),
        );

        let accessor: Arc<dyn orkester_io::AssetAccessor> = match &self.tileset_path {
            Some(path)
                if path.ends_with(".3tz") || path.ends_with(".slpk") || path.ends_with(".zip") =>
            {
                Arc::new(
                    orkester_io::ArchiveAccessor::open(path, bg_ctx.clone()).expect("open archive"),
                )
            }
            Some(_) | None => Arc::new(orkester_io::FileAccessor::new(bg_ctx.clone())),
        };

        let on_error = |d: &selekt::LoadFailureDetails| {
            eprintln!(
                "[tile-error] node={:?} {:?}: {}",
                d.node_id, d.failure_type, d.message
            )
        };
        if let Some(path) = &self.tileset_path {
            let mut b = TilesetBuilder::open(path.clone())
                .with_main_context(main_ctx)
                .on_error(on_error);
            if self.no_cull {
                b = b.policy(AllVisibleLruPolicy);
            }
            self.tileset = Some(b.build(bg_ctx, accessor, preparer));
        }

        self.renderer = Some(renderer);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(r) = &mut self.renderer {
                    r.resize(size.width, size.height);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state == ElementState::Pressed;
                if let PhysicalKey::Code(code) = event.physical_key {
                    match code {
                        KeyCode::KeyW => self.keys.w = pressed,
                        KeyCode::KeyA => self.keys.a = pressed,
                        KeyCode::KeyS => self.keys.s = pressed,
                        KeyCode::KeyD => self.keys.d = pressed,
                        KeyCode::KeyQ => self.keys.q = pressed,
                        KeyCode::KeyE => self.keys.e = pressed,
                        _ => {}
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let pressed = state == ElementState::Pressed;
                match button {
                    MouseButton::Left => self.left_down = pressed,
                    MouseButton::Right => self.right_down = pressed,
                    _ => {}
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let scroll = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y as f64,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y / 120.0,
                };
                self.camera.zoom_delta += scroll;
            }
            WindowEvent::RedrawRequested => {
                self.render_frame();
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        if let DeviceEvent::MouseMotion { delta: (dx, dy) } = event {
            if self.left_down {
                self.camera.rotate(dx, dy);
            } else if self.right_down {
                self.camera.pan(dx, dy, &self.ellipsoid);
            }
        }
    }
}

impl App {
    fn render_frame(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32();
        self.last_frame = now;

        // Pump main-thread GPU uploads
        self.work_queue
            .pump_timed(std::time::Duration::from_millis(4));

        // Auto-position camera above the tileset root bounding volume once ready
        if !self.camera_positioned {
            if let Some(tileset) = &self.tileset {
                if let Some(bounds) = tileset.root_bounds() {
                    self.camera.position_above_bounds(bounds, &self.ellipsoid);
                    self.camera_positioned = true;
                    eprintln!("camera auto-positioned above tileset bounds");
                }
            }
        }

        // Camera movement
        let fwd = self.keys.w as i32 - self.keys.s as i32;
        let right = self.keys.d as i32 - self.keys.a as i32;
        let up = self.keys.e as i32 - self.keys.q as i32;
        if fwd != 0 || right != 0 || up != 0 {
            self.camera
                .fly(fwd as f64, right as f64, up as f64, dt as f64, &self.ellipsoid);
        }

        // Apply scroll wheel zoom
        self.camera.apply_zoom(&self.ellipsoid);

        let size = self
            .window
            .as_ref()
            .map(|w| w.inner_size())
            .unwrap_or_default();
        let viewport = [size.width.max(1), size.height.max(1)];

        // Selekt ViewState uses ECEF
        let view_state = self.camera.view_state(viewport, FOV_Y, &self.ellipsoid);

        // Update tile selection
        if let Some(tileset) = &mut self.tileset {
            tileset.update(&[view_state.clone()], dt);
        }
        if let Some(globe) = &mut self.globe_tileset {
            globe.update(&[view_state], dt);
        }

        // Compute camera-relative transforms.
        // Dynamic near/far based on altitude to preserve depth buffer precision.
        let altitude = self
            .ellipsoid
            .ecef_to_cartographic(self.camera.position_ecef)
            .map(|c| c.height)
            .unwrap_or(1e6)
            .max(1.0);
        let near = (altitude * 0.01).max(0.1);
        let far = altitude * 100.0 + 1e7; // always includes at least Earth-radius range
        let (proj_view, ecef_to_enu) =
            self.camera
                .proj_view(viewport, FOV_Y, near, far, &self.ellipsoid);

        // Draw
        if let Some(renderer) = &self.renderer {
            let nodes: Vec<_> = self
                .tileset
                .as_ref()
                .map(|t| t.render_nodes().collect())
                .unwrap_or_default();
            let globe_nodes: Vec<_> = self
                .globe_tileset
                .as_ref()
                .map(|t| t.render_nodes().collect())
                .unwrap_or_default();
            let tile_count: usize = nodes.iter().map(|rn| rn.content.len()).sum();
            let load_progress = self
                .tileset
                .as_ref()
                .map(|t| t.compute_load_progress())
                .unwrap_or(0.0);
            let empty_fr = selekt::FrameResult::default();
            let fr = self
                .tileset
                .as_ref()
                .map(|t| t.last_result())
                .unwrap_or(&empty_fr);
            // Throttle to once per second to avoid terminal spam
            static LAST_PRINT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            if now_ms.saturating_sub(LAST_PRINT.load(std::sync::atomic::Ordering::Relaxed)) >= 1000
            {
                LAST_PRINT.store(now_ms, std::sync::atomic::Ordering::Relaxed);
                eprintln!(
                    "frame: ready={} render_nodes={} gpu_tiles={} load={:.0}% globe_nodes={} | visited={} culled={} kicked={} to_render={} bytes={}",
                    self.tileset.as_ref().map(|t| t.is_ready()).unwrap_or(false),
                    nodes.len(),
                    tile_count,
                    load_progress,
                    globe_nodes.len(),
                    fr.nodes_visited,
                    fr.nodes_culled,
                    fr.nodes_kicked,
                    fr.nodes_to_render.len(),
                    fr.bytes_resident,
                );
                eprintln!(
                    "  cam_ecef={:.0?} cam_ecef_len={:.0}km",
                    self.camera.position_ecef,
                    self.camera.position_ecef.length() / 1000.0,
                );
                if let Some(bounds) = self.tileset.as_ref().and_then(|t| t.root_bounds()) {
                    eprintln!("  root_bounds={bounds:?}");
                }
                if let Some(rn) = nodes.first() {
                    let model_to_enu = (ecef_to_enu * rn.world_transform).as_mat4();
                    let mvp = proj_view * model_to_enu;
                    let nan = mvp
                        .to_cols_array()
                        .iter()
                        .any(|v| v.is_nan() || v.is_infinite());
                    eprintln!(
                        "  first_node world={:?} mvp_has_nan={}",
                        rn.world_transform.col(3),
                        nan
                    );
                }
            }
            let globe_iter = globe_nodes.iter().flat_map(|rn| {
                let transform = rn.world_transform;
                rn.content.iter().map(move |tile| (tile, transform))
            });
            renderer.draw_frame(
                proj_view,
                ecef_to_enu,
                globe_iter.chain(nodes.iter().flat_map(|rn| {
                    let transform = rn.world_transform;
                    rn.content.iter().map(move |tile| (tile, transform))
                })),
            );
        }
    }
}

fn main() {
    env_logger::init();

    let args: Vec<String> = std::env::args().collect();
    let tileset_path = args[1..].iter().find(|a| !a.starts_with("--")).cloned();
    let no_cull = args.iter().any(|a| a == "--no-cull");

    let event_loop = EventLoop::new().expect("create event loop");
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App::new(tileset_path, no_cull);
    event_loop.run_app(&mut app).expect("run app");
}
