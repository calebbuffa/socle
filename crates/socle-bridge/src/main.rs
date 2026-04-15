//! socle-bridge — WebSocket server bridging Rust tile selection to Babylon.js.
//!
//! Protocol (binary framing over WebSocket):
//!
//! **Client → Server (JSON text messages):**
//!   `{ "type": "view", "position": [x,y,z], "direction": [x,y,z], "up": [x,y,z],
//!      "viewport": [w,h], "fov_x": f64, "fov_y": f64 }`
//!   `{ "type": "open", "url": "https://..." }`
//!   `{ "type": "sse", "value": f64 }`
//!
//! **Server → Client:**
//!   JSON text: `{ "type": "frame", "add": [{ "id": n, "transform": [16 floats] }], "remove": [id, ...] }`
//!   Binary:    4-byte LE node-id ++ GLB bytes  (sent once per new tile)

use std::collections::{HashMap, HashSet};
use std::net::TcpListener;
use std::sync::Arc;
use std::time::Instant;

use clap::Parser;
use glam::DVec3;
use serde::{Deserialize, Serialize};
use tungstenite::accept;
use tungstenite::protocol::Message;

use egaku::{ContentPipeline, PipelineError};
use kasane::{Basemap, OverlayTarget};
use kiban::{FadeState, Kiban};
use moderu::GltfModel;
use moderu_io::GltfWriter;
use orkester::{Context, ThreadPool, WorkQueue};
use orkester_io::HttpAccessor;
use selekt::ViewState;
use terra::Ellipsoid;
use tiles3d_selekt::{TilesetBuilder, TilesetResult};

#[derive(Parser)]
#[command(name = "socle-bridge", about = "WebSocket bridge for Babylon.js")]
struct Cli {
    /// Address to listen on.
    #[arg(long, default_value = "127.0.0.1:9001")]
    addr: String,

    /// Initial tileset URL (can also be sent via "open" message).
    #[arg(long)]
    url: Option<String>,

    /// Path to a local tileset.json file. Will be served over HTTP automatically.
    #[arg(long)]
    file: Option<std::path::PathBuf>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientMsg {
    View {
        position: [f64; 3],
        direction: [f64; 3],
        up: [f64; 3],
        viewport: [u32; 2],
        fov_x: f64,
        fov_y: f64,
    },
    Open {
        url: String,
    },
    Sse {
        value: f64,
    },
}

#[derive(Serialize)]
struct FrameMsg {
    #[serde(rename = "type")]
    msg_type: &'static str,
    add: Vec<TileAdd>,
    remove: Vec<u64>,
}

#[derive(Serialize)]
struct TileAdd {
    id: u64,
    transform: [f64; 16],
    /// Opacity for LOD fade transitions: 0.0 (transparent) → 1.0 (opaque).
    alpha: f32,
}

/// Duration of LOD fade-in / fade-out transitions at the renderer.
const LOD_TRANSITION_SECS: f32 = 1.0;

/// The "content" stored per tile: the glTF model (for overlay baking) and
/// its serialised GLB bytes ready to send to the browser.
struct GlbContent {
    model: GltfModel,
    bytes: Vec<u8>,
    /// Incremented each time the bytes are re-serialised (e.g. overlay bake).
    version: u64,
}

impl OverlayTarget for GlbContent {
    fn attach_raster(
        &mut self,
        tile: &kasane::RasterOverlayTile,
        translation: [f64; 2],
        scale: [f64; 2],
    ) {
        log::info!(
            "attach_raster: tile {}x{} pixels={} tx=[{:.4},{:.4}] sc=[{:.4},{:.4}] bufs={} bvs={} accs={} mats={} texs={}",
            tile.width,
            tile.height,
            tile.pixels.len(),
            translation[0],
            translation[1],
            scale[0],
            scale[1],
            self.model.buffers.len(),
            self.model.buffer_views.len(),
            self.model.accessors.len(),
            self.model.materials.len(),
            self.model.textures.len(),
        );
        let ok = kasane::apply_raster_overlay(&mut self.model, tile, translation, scale);
        log::info!(
            "after apply: ok={ok} bufs={} bvs={} accs={} buf0_len={}",
            self.model.buffers.len(),
            self.model.buffer_views.len(),
            self.model.accessors.len(),
            self.model
                .buffers
                .first()
                .map(|b| b.data.len())
                .unwrap_or(0),
        );
        // Re-serialise GLB with the new overlay texture baked in.
        let mut buf = Vec::new();
        match GltfWriter::default().write_glb_to_buffer(&self.model, &mut buf) {
            Ok(()) => {
                log::info!("GLB re-serialized: {} bytes", buf.len());
                self.bytes = buf;
                self.version += 1;
            }
            Err(e) => {
                log::error!("GLB write failed: {e}");
            }
        }
    }

    fn detach_raster(&mut self, _overlay_id: kasane::OverlayId) {}
}

fn make_pipeline(bg: Context, main: Context) -> Arc<ContentPipeline<GlbContent>> {
    Arc::new(ContentPipeline::new(move |model| {
        bg.run(move || {
            let mut buf = Vec::new();
            match GltfWriter::default().write_glb_to_buffer(&model, &mut buf) {
                Ok(()) => Ok((model, buf)),
                Err(e) => Err(Box::new(e) as PipelineError),
            }
        })
        .then(
            &main,
            move |result: Result<(GltfModel, Vec<u8>), PipelineError>| {
                orkester::resolved(result.map(|(model, bytes)| GlbContent {
                    model,
                    bytes,
                    version: 0,
                }))
            },
        )
    }))
}

/// Convert a [`TilesetResult`] into a [`Kiban`] runtime layer.
fn build_kiban(
    result: TilesetResult<GlbContent>,
    accessor: Arc<dyn orkester_io::AssetAccessor>,
    bg: Context,
) -> Kiban<GlbContent> {
    match result {
        TilesetResult::Ready(r) => {
            let sse = r.maximum_screen_space_error;
            let mut k = Kiban::ready(r, accessor, bg);
            k.set_maximum_screen_space_error(sse);
            k
        }
        TilesetResult::Loading {
            task,
            maximum_screen_space_error,
        } => {
            let mut k = Kiban::from_task(task, accessor, bg);
            k.set_maximum_screen_space_error(maximum_screen_space_error);
            k
        }
    }
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let mut cli = Cli::parse();

    // If --file is given, serve its parent directory on port 9090 and build the URL.
    if let Some(ref file_path) = cli.file {
        let file_path = std::fs::canonicalize(file_path)
            .unwrap_or_else(|e| panic!("cannot resolve --file path: {e}"));
        let dir = file_path
            .parent()
            .expect("--file must point to a file, not a directory")
            .to_path_buf();
        let filename = file_path
            .file_name()
            .expect("--file has no filename")
            .to_string_lossy()
            .to_string();
        let data_dir = dir.clone();
        std::thread::spawn(move || {
            serve_data_dir(data_dir);
        });
        cli.url = Some(format!("http://127.0.0.1:9090/{filename}"));
        log::info!("serving tileset dir {} on :9090", dir.display());
    }

    // Spawn a tiny HTTP server for the web/ directory on port 8080.
    std::thread::spawn(|| {
        serve_web_dir();
    });

    let listener = TcpListener::bind(&cli.addr).expect("failed to bind");
    log::info!("listening on ws://{}", cli.addr);
    log::info!("open http://127.0.0.1:8080 in your browser");

    // Accept one client at a time (sufficient for a debug viewer).
    for stream in listener.incoming() {
        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                log::error!("accept error: {e}");
                continue;
            }
        };
        log::info!("client connected: {}", stream.peer_addr().unwrap());
        if let Err(e) = handle_client(stream, &cli) {
            log::error!("session error: {e}");
        }
    }
}

fn handle_client(stream: std::net::TcpStream, cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    let mut ws = accept(stream)?;

    // Runtime
    let pool = ThreadPool::new(4);
    let mut work_queue = WorkQueue::new();
    let bg = pool.context();
    let main_ctx = work_queue.context();
    let ellipsoid = Ellipsoid::wgs84();
    let accessor: Arc<dyn orkester_io::AssetAccessor> = Arc::new(HttpAccessor::new(bg.clone()));
    let pipeline = make_pipeline(bg.clone(), main_ctx.clone());

    // Tilesets
    let mut tileset: Option<Kiban<GlbContent>> = None;
    let mut globe: Option<Kiban<GlbContent>>;
    let mut sse = 16.0f64;

    // Open initial URL if provided.
    if let Some(url) = &cli.url {
        let mut ts_opts = selekt::SelectionOptions::default();
        ts_opts.streaming.enable_lod_transition = true;
        tileset = Some(build_kiban(
            TilesetBuilder::open(url)
                .maximum_screen_space_error(sse)
                .options(ts_opts)
                .build(bg.clone(), Arc::clone(&accessor), Arc::clone(&pipeline)),
            Arc::clone(&accessor),
            bg.clone(),
        ));
    }

    // Always create the globe.
    let mut globe_opts = selekt::SelectionOptions::default();
    globe_opts.culling.render_nodes_under_camera = true;
    globe_opts.streaming.enable_lod_transition = true;
    let mut globe_ts = build_kiban(
        TilesetBuilder::ellipsoid(ellipsoid.clone())
            .maximum_screen_space_error(sse)
            .options(globe_opts)
            .build(bg.clone(), Arc::clone(&accessor), Arc::clone(&pipeline)),
        Arc::clone(&accessor),
        bg.clone(),
    );
    globe_ts.overlays.add(Basemap::Osm.into_overlay());
    globe = Some(globe_ts);

    // Track which node IDs the client already has GLB data for.
    let mut client_has: HashMap<u64, u64> = HashMap::new();
    let mut last_rendered: HashSet<u64> = HashSet::new();
    // Per-node fade-in start times (keyed on offsetted ID).
    let mut fade_in_starts: HashMap<u64, Instant> = HashMap::new();
    // Per-node fade-out start times (keyed on offsetted ID).
    let mut fade_out_starts: HashMap<u64, Instant> = HashMap::new();
    let mut last_frame = Instant::now();

    // Set non-blocking so we can pump the work queue between messages.
    ws.get_ref().set_nonblocking(true)?;

    loop {
        // Pump main-thread work (tile finalization).
        work_queue.flush_timed(std::time::Duration::from_millis(16));

        // Read messages (non-blocking).
        match ws.read() {
            Ok(Message::Text(text)) => {
                match serde_json::from_str::<ClientMsg>(&text) {
                    Ok(ClientMsg::View {
                        position,
                        direction,
                        up,
                        viewport,
                        fov_x,
                        fov_y,
                    }) => {
                        let dt = last_frame.elapsed().as_secs_f32();
                        last_frame = Instant::now();

                        let pos = DVec3::from_array(position);
                        let dir = DVec3::from_array(direction);
                        let up_vec = DVec3::from_array(up);

                        let view = ViewState::perspective(pos, dir, up_vec, viewport, fov_x, fov_y)
                            .with_ellipsoid(ellipsoid.clone());
                        let views = [view];

                        // Update tilesets via staged API.
                        if let Some(t) = &mut tileset {
                            t.update_view_group(&views, dt);
                            t.load_nodes();
                            let _ = t.dispatch_main_thread_events();
                        }
                        if let Some(g) = &mut globe {
                            g.update_view_group(&views, dt);
                            g.load_nodes();
                            let _ = g.dispatch_main_thread_events();
                        }

                        // Drain telemetry events after staged dispatch.
                        for layer in [&mut globe, &mut tileset] {
                            if let Some(ts) = layer {
                                for _ in ts.drain_events() {}
                            }
                        }

                        // Collect render nodes
                        let mut current_rendered: HashSet<u64> = HashSet::new();
                        let mut adds: Vec<TileAdd> = Vec::new();

                        // Collect tiles whose GLB version has changed so we can
                        // send a remove frame before the new binary. Without this
                        // Babylon.js stacks the new mesh on top of the old one.
                        let mut version_changed: Vec<u64> = Vec::new();
                        let check_version =
                            |ts: &Kiban<GlbContent>, offset: u64, out: &mut Vec<u64>| {
                                for rn in ts.render_nodes() {
                                    let id = rn.id.0.get() + offset;
                                    if let Some(&v) = client_has.get(&id) {
                                        if v != rn.content.version {
                                            out.push(id);
                                        }
                                    }
                                }
                            };
                        if let Some(g) = &globe {
                            check_version(g, 0, &mut version_changed);
                        }
                        if let Some(t) = &tileset {
                            check_version(t, 1_000_000, &mut version_changed);
                        }
                        if !version_changed.is_empty() {
                            let pre_remove = FrameMsg {
                                msg_type: "frame",
                                add: Vec::new(),
                                remove: version_changed.clone(),
                            };
                            let _ = ws.send(Message::Text(
                                serde_json::to_string(&pre_remove).unwrap().into(),
                            ));
                            for id in &version_changed {
                                client_has.remove(id);
                            }
                        }

                        // Helper: process render nodes from a tileset.
                        // `id_offset` separates namespaces so globe and tileset IDs don't collide.
                        let mut process =
                            |ts: &Kiban<GlbContent>, id_offset: u64, ws: &mut tungstenite::WebSocket<std::net::TcpStream>| {
                                for rn in ts.render_nodes() {
                                    let id = rn.id.0.get() + id_offset;
                                    current_rendered.insert(id);

                                    // Send GLB binary if client doesn't have it or version changed.
                                    let client_ver = client_has.get(&id).copied();
                                    if client_ver != Some(rn.content.version) {
                                        let mut payload =
                                            Vec::with_capacity(4 + rn.content.bytes.len());
                                        payload.extend_from_slice(&(id as u32).to_le_bytes());
                                        payload.extend_from_slice(&rn.content.bytes);
                                        let _ = ws.send(Message::Binary(payload.into()));
                                        client_has.insert(id, rn.content.version);
                                    }

                                    // Compute fade alpha from transition state.
                                    let alpha = match rn.fade_state {
                                        FadeState::Normal => {
                                            fade_in_starts.remove(&id);
                                            1.0f32
                                        }
                                        FadeState::FadingIn => {
                                            let start = fade_in_starts
                                                .entry(id)
                                                .or_insert_with(Instant::now);
                                            (start.elapsed().as_secs_f32() / LOD_TRANSITION_SECS)
                                                .min(1.0)
                                        }
                                        FadeState::FadingOut => {
                                            let start = fade_out_starts
                                                .entry(id)
                                                .or_insert_with(Instant::now);
                                            (1.0 - start.elapsed().as_secs_f32()
                                                / LOD_TRANSITION_SECS)
                                                .max(0.0)
                                        }
                                    };

                                    // World transform as column-major f64 array.
                                    let cols = rn.world_transform.to_cols_array();
                                    adds.push(TileAdd { id, transform: cols, alpha });
                                }
                            };

                        if let Some(g) = &globe {
                            process(g, 0, &mut ws);
                        }
                        if let Some(t) = &tileset {
                            process(t, 1_000_000, &mut ws);
                        }

                        // Determine removed tiles.
                        let removes: Vec<u64> = last_rendered
                            .difference(&current_rendered)
                            .copied()
                            .collect();

                        if !adds.is_empty() || !removes.is_empty() {
                            log::info!(
                                "frame: add={} rm={} total={} pos=({:.0},{:.0},{:.0})",
                                adds.len(),
                                removes.len(),
                                current_rendered.len(),
                                pos.x,
                                pos.y,
                                pos.z,
                            );
                        }

                        // Clean up client cache for removed tiles.
                        for &r in &removes {
                            client_has.remove(&r);
                            fade_in_starts.remove(&r);
                            fade_out_starts.remove(&r);
                        }

                        last_rendered = current_rendered;

                        // Send frame message.
                        let frame = FrameMsg {
                            msg_type: "frame",
                            add: adds,
                            remove: removes,
                        };
                        let _ =
                            ws.send(Message::Text(serde_json::to_string(&frame).unwrap().into()));
                    }
                    Ok(ClientMsg::Open { url }) => {
                        log::info!("opening tileset: {url}");
                        client_has.clear();
                        last_rendered.clear();
                        fade_in_starts.clear();
                        fade_out_starts.clear();
                        let mut ts_opts = selekt::SelectionOptions::default();
                        ts_opts.streaming.enable_lod_transition = true;
                        tileset = Some(build_kiban(
                            TilesetBuilder::open(&url)
                                .maximum_screen_space_error(sse)
                                .options(ts_opts)
                                .build(bg.clone(), Arc::clone(&accessor), Arc::clone(&pipeline)),
                            Arc::clone(&accessor),
                            bg.clone(),
                        ));
                    }
                    Ok(ClientMsg::Sse { value }) => {
                        log::info!("SSE → {value}");
                        sse = value;
                        // Rebuild tilesets with new SSE (simplest approach).
                    }
                    Err(e) => {
                        log::warn!("bad message: {e}");
                    }
                }
            }
            Ok(Message::Close(_)) => {
                log::info!("client disconnected");
                break;
            }
            Ok(_) => {} // ping/pong/binary from client — ignore
            Err(tungstenite::Error::Io(ref e)) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // No message available — sleep briefly then loop.
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            Err(e) => {
                log::error!("ws read error: {e}");
                break;
            }
        }
    }

    Ok(())
}

/// Minimal HTTP server serving the `web/` directory on port 8080.
fn serve_web_dir() {
    use std::io::{BufRead, BufReader, Write};

    // Find the web/ directory relative to the executable or CWD.
    let web_dir = find_web_dir();
    let listener = match TcpListener::bind("127.0.0.1:8080") {
        Ok(l) => l,
        Err(e) => {
            log::warn!("could not start HTTP server on :8080: {e}");
            return;
        }
    };
    log::info!("HTTP serving {}", web_dir.display());

    for stream in listener.incoming().flatten() {
        let web_dir = web_dir.clone();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(&stream);
            let mut request_line = String::new();
            if reader.read_line(&mut request_line).is_err() {
                return;
            }
            // Parse "GET /path HTTP/1.1"
            let path = request_line.split_whitespace().nth(1).unwrap_or("/");
            let path = if path == "/" { "/index.html" } else { path };

            // Drain headers
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
                    break;
                }
            }

            let file_path = web_dir.join(&path[1..]); // strip leading /
            let (status, content_type, body) =
                if file_path.exists() && file_path.starts_with(&web_dir) {
                    let body = std::fs::read(&file_path).unwrap_or_default();
                    let ct = match file_path.extension().and_then(|e| e.to_str()) {
                        Some("html") => "text/html",
                        Some("js") => "application/javascript",
                        Some("css") => "text/css",
                        Some("json") => "application/json",
                        _ => "application/octet-stream",
                    };
                    ("200 OK", ct, body)
                } else {
                    ("404 Not Found", "text/plain", b"not found".to_vec())
                };

            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\n\r\n",
                body.len()
            );
            let mut stream = stream;
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.write_all(&body);
        });
    }
}

fn find_web_dir() -> std::path::PathBuf {
    // Try relative to CWD first (common when running with cargo run).
    let candidates = [
        std::path::PathBuf::from("crates/socle-bridge/web"),
        std::path::PathBuf::from("web"),
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("web")))
            .unwrap_or_default(),
    ];
    for c in &candidates {
        if c.join("index.html").exists() {
            return c.clone();
        }
    }
    log::warn!("web/ directory not found, HTTP server won't serve files");
    std::path::PathBuf::from("web")
}

/// Serve a local tileset directory on port 9090 so the HTTP-based tile loader can reach it.
fn serve_data_dir(data_dir: std::path::PathBuf) {
    use std::io::{BufRead, BufReader, Write};

    let listener = match TcpListener::bind("127.0.0.1:9090") {
        Ok(l) => l,
        Err(e) => {
            log::error!("could not start data server on :9090: {e}");
            return;
        }
    };
    log::info!("data HTTP serving {}", data_dir.display());

    for stream in listener.incoming().flatten() {
        let data_dir = data_dir.clone();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(&stream);
            let mut request_line = String::new();
            if reader.read_line(&mut request_line).is_err() {
                return;
            }
            let path = request_line.split_whitespace().nth(1).unwrap_or("/");
            let path = if path == "/" { "/index.html" } else { path };
            // URL-decode the path
            let decoded = urlencoding_decode(path);

            // Drain headers
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
                    break;
                }
            }

            let file_path = data_dir.join(decoded.trim_start_matches('/'));
            let (status, content_type, body) =
                if file_path.exists() && file_path.starts_with(&data_dir) {
                    let body = std::fs::read(&file_path).unwrap_or_default();
                    let ct = match file_path.extension().and_then(|e| e.to_str()) {
                        Some("json") => "application/json",
                        Some("glb") => "application/octet-stream",
                        Some("gltf") => "model/gltf+json",
                        Some("b3dm") => "application/octet-stream",
                        Some("cmpt") => "application/octet-stream",
                        Some("pnts") => "application/octet-stream",
                        Some("i3dm") => "application/octet-stream",
                        Some("bin") => "application/octet-stream",
                        Some("png") => "image/png",
                        Some("jpg") | Some("jpeg") => "image/jpeg",
                        _ => "application/octet-stream",
                    };
                    ("200 OK", ct, body)
                } else {
                    log::warn!("data 404: {}", file_path.display());
                    ("404 Not Found", "text/plain", b"not found".to_vec())
                };

            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\n\r\n",
                body.len()
            );
            let mut stream = stream;
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.write_all(&body);
        });
    }
}

/// Simple percent-decoding for URL paths.
fn urlencoding_decode(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let hi = chars.next().unwrap_or(b'0');
            let lo = chars.next().unwrap_or(b'0');
            let val =
                u8::from_str_radix(&format!("{}{}", hi as char, lo as char), 16).unwrap_or(b'?');
            result.push(val as char);
        } else {
            result.push(b as char);
        }
    }
    result
}
