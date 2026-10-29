//! Headless-ish Ruffle server: renders Brawlhalla offscreen and streams JPEG
//! frames over MJPEG so the browser only needs a tiny viewer page.
//!
//! This is the native alternative to the wasm build: the game gets the full
//! depot and host memory, while the browser sees a normal `<img>` stream.

use std::any::Any;
use std::io::{Cursor, Read};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use image::RgbaImage;
use ruffle_core::backend::audio::NullAudioBackend;
use ruffle_core::backend::fs::DiskFileSystem;
use ruffle_core::events::{
    KeyDescriptor, KeyLocation, LogicalKey, NamedKey, PhysicalKey,
};
use ruffle_core::limits::ExecutionLimit;
use ruffle_core::tag_utils::movie_from_path;
use ruffle_core::{LoadBehavior, Player, PlayerBuilder, PlayerEvent, PlayerRuntime};
use ruffle_render_wgpu::backend::{
    WgpuRenderBackend, create_wgpu_instance, request_adapter_and_device,
};
use ruffle_render_wgpu::descriptors::Descriptors;
use ruffle_render_wgpu::target::TextureTarget;
use ruffle_render_wgpu::wgpu;
use tiny_http::{Header, Method, Response, Server, StatusCode};

/// Input commands queued by the HTTP server for the player thread.
struct InputCommand {
    down: bool,
    key: KeyDescriptor,
}

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;
const JPEG_QUALITY: u8 = 80;

struct LatestFrame {
    jpeg: Mutex<Vec<u8>>,
    generation: Mutex<u64>,
    condvar: Condvar,
}

impl LatestFrame {
    fn new() -> Self {
        Self {
            jpeg: Mutex::new(Vec::new()),
            generation: Mutex::new(0),
            condvar: Condvar::new(),
        }
    }

    fn publish(&self, jpeg: Vec<u8>) {
        *self.jpeg.lock().unwrap() = jpeg;
        let mut generation = self.generation.lock().unwrap();
        *generation += 1;
        self.condvar.notify_all();
    }
}

struct MjpegStream {
    frame: Arc<LatestFrame>,
    seen: u64,
    pending: Vec<u8>,
}

impl Read for MjpegStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.pending.is_empty() {
            let mut generation = self.frame.generation.lock().unwrap();
            while *generation == self.seen {
                generation = self.frame.condvar.wait(generation).unwrap();
            }
            self.seen = *generation;
            drop(generation);

            let jpeg = self.frame.jpeg.lock().unwrap();
            if !jpeg.is_empty() {
                let header = format!(
                    "--frame\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                    jpeg.len()
                );
                self.pending.extend_from_slice(header.as_bytes());
                self.pending.extend_from_slice(&jpeg);
                self.pending.extend_from_slice(b"\r\n");
            }
        }

        let amount = self.pending.len().min(buf.len());
        buf[..amount].copy_from_slice(&self.pending[..amount]);
        self.pending.drain(..amount);
        Ok(amount)
    }
}

fn encode_jpeg(image: &RgbaImage) -> Result<Vec<u8>> {
    let mut cursor = Cursor::new(Vec::new());
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, JPEG_QUALITY);
    encoder
        .encode_image(image)
        .map_err(|error| anyhow!("JPEG encode failed: {error}"))?;
    Ok(cursor.into_inner())
}

fn capture(player: &Arc<Mutex<Player>>) -> Option<RgbaImage> {
    let mut player = player.lock().unwrap();
    player.render();
    let renderer =
        <dyn Any>::downcast_mut::<WgpuRenderBackend<TextureTarget>>(player.renderer_mut())?;
    renderer.capture_frame()
}

fn serve_page(
    server: &Server,
    frame: Arc<LatestFrame>,
    input: Sender<InputCommand>,
) -> Result<()> {
    let html = r#"<!doctype html>
<html>
<head><meta charset="utf-8"><title>Brawlhalla (native stream)</title>
<style>html,body{margin:0;height:100%;background:#000;overflow:hidden}img{width:100vw;height:100vh;object-fit:contain}</style>
</head>
<body tabindex="0"><img src="/stream.mjpg" alt="stream">
<script>
const keys = new Set(["KeyW","KeyA","KeyS","KeyD","KeyZ","KeyX","KeyC","KeyV","Space","Enter","Escape","ArrowUp","ArrowDown","ArrowLeft","ArrowRight","ShiftLeft","ShiftRight","ControlLeft","Tab","Backspace","KeyE","KeyQ","KeyR","KeyF","KeyG","KeyH","KeyJ","KeyK","KeyL","KeyM","KeyN","KeyO","KeyP","KeyT","KeyU","KeyI","KeyY","KeyB","Digit1","Digit2","Digit3","Digit4"]);
for (const type of ["keydown","keyup"]) {
  window.addEventListener(type, (event) => {
    if (!keys.has(event.code)) return;
    event.preventDefault();
    fetch("/input", {method:"POST", headers:{"Content-Type":"application/json"}, body: JSON.stringify({type, code: event.code})}).catch(()=>{});
  });
}
window.addEventListener("click", () => window.focus());
window.focus();
</script>
</body>
</html>"#;

    for mut request in server.incoming_requests() {
        let path = request.url().to_owned();
        match (request.method(), path.as_str()) {
            (Method::Post, "/input") => {
                let mut body = String::new();
                let _ = request.as_reader().read_to_string(&mut body);
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&body) {
                    let down = value.get("type").and_then(|v| v.as_str()) == Some("keydown");
                    if let Some(key) = value
                        .get("code")
                        .and_then(|v| v.as_str())
                        .and_then(key_descriptor)
                    {
                        let _ = input.send(InputCommand { down, key });
                    }
                }
                let _ = request.respond(Response::empty(StatusCode(204)));
            }
            (Method::Get, "/" | "/index.html") => {
                let response = Response::from_string(html).with_header(
                    Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap(),
                );
                let _ = request.respond(response);
            }
            (Method::Get, "/stream.mjpg") => {
                let headers = vec![
                    Header::from_bytes("Content-Type", "multipart/x-mixed-replace; boundary=frame")
                        .unwrap(),
                    Header::from_bytes("Cache-Control", "no-store").unwrap(),
                ];
                let stream = MjpegStream {
                    frame: frame.clone(),
                    seen: 0,
                    pending: Vec::new(),
                };
                let response = Response::new(StatusCode(200), headers, stream, None, None);
                let _ = request.respond(response);
            }
            _ => {
                let _ = request.respond(Response::empty(StatusCode(404)));
            }
        }
    }
    Ok(())
}

fn key_descriptor(code: &str) -> Option<KeyDescriptor> {
    let (physical_key, logical_key, key_location) = match code {
        "KeyA" => (PhysicalKey::KeyA, LogicalKey::Character('a'), KeyLocation::Standard),
        "KeyB" => (PhysicalKey::KeyB, LogicalKey::Character('b'), KeyLocation::Standard),
        "KeyC" => (PhysicalKey::KeyC, LogicalKey::Character('c'), KeyLocation::Standard),
        "KeyD" => (PhysicalKey::KeyD, LogicalKey::Character('d'), KeyLocation::Standard),
        "KeyE" => (PhysicalKey::KeyE, LogicalKey::Character('e'), KeyLocation::Standard),
        "KeyF" => (PhysicalKey::KeyF, LogicalKey::Character('f'), KeyLocation::Standard),
        "KeyG" => (PhysicalKey::KeyG, LogicalKey::Character('g'), KeyLocation::Standard),
        "KeyH" => (PhysicalKey::KeyH, LogicalKey::Character('h'), KeyLocation::Standard),
        "KeyI" => (PhysicalKey::KeyI, LogicalKey::Character('i'), KeyLocation::Standard),
        "KeyJ" => (PhysicalKey::KeyJ, LogicalKey::Character('j'), KeyLocation::Standard),
        "KeyK" => (PhysicalKey::KeyK, LogicalKey::Character('k'), KeyLocation::Standard),
        "KeyL" => (PhysicalKey::KeyL, LogicalKey::Character('l'), KeyLocation::Standard),
        "KeyM" => (PhysicalKey::KeyM, LogicalKey::Character('m'), KeyLocation::Standard),
        "KeyN" => (PhysicalKey::KeyN, LogicalKey::Character('n'), KeyLocation::Standard),
        "KeyO" => (PhysicalKey::KeyO, LogicalKey::Character('o'), KeyLocation::Standard),
        "KeyP" => (PhysicalKey::KeyP, LogicalKey::Character('p'), KeyLocation::Standard),
        "KeyQ" => (PhysicalKey::KeyQ, LogicalKey::Character('q'), KeyLocation::Standard),
        "KeyR" => (PhysicalKey::KeyR, LogicalKey::Character('r'), KeyLocation::Standard),
        "KeyS" => (PhysicalKey::KeyS, LogicalKey::Character('s'), KeyLocation::Standard),
        "KeyT" => (PhysicalKey::KeyT, LogicalKey::Character('t'), KeyLocation::Standard),
        "KeyU" => (PhysicalKey::KeyU, LogicalKey::Character('u'), KeyLocation::Standard),
        "KeyV" => (PhysicalKey::KeyV, LogicalKey::Character('v'), KeyLocation::Standard),
        "KeyW" => (PhysicalKey::KeyW, LogicalKey::Character('w'), KeyLocation::Standard),
        "KeyX" => (PhysicalKey::KeyX, LogicalKey::Character('x'), KeyLocation::Standard),
        "KeyY" => (PhysicalKey::KeyY, LogicalKey::Character('y'), KeyLocation::Standard),
        "KeyZ" => (PhysicalKey::KeyZ, LogicalKey::Character('z'), KeyLocation::Standard),
        "Digit0" => (PhysicalKey::Digit0, LogicalKey::Character('0'), KeyLocation::Standard),
        "Digit1" => (PhysicalKey::Digit1, LogicalKey::Character('1'), KeyLocation::Standard),
        "Digit2" => (PhysicalKey::Digit2, LogicalKey::Character('2'), KeyLocation::Standard),
        "Digit3" => (PhysicalKey::Digit3, LogicalKey::Character('3'), KeyLocation::Standard),
        "Digit4" => (PhysicalKey::Digit4, LogicalKey::Character('4'), KeyLocation::Standard),
        "Digit5" => (PhysicalKey::Digit5, LogicalKey::Character('5'), KeyLocation::Standard),
        "Digit6" => (PhysicalKey::Digit6, LogicalKey::Character('6'), KeyLocation::Standard),
        "Digit7" => (PhysicalKey::Digit7, LogicalKey::Character('7'), KeyLocation::Standard),
        "Digit8" => (PhysicalKey::Digit8, LogicalKey::Character('8'), KeyLocation::Standard),
        "Digit9" => (PhysicalKey::Digit9, LogicalKey::Character('9'), KeyLocation::Standard),
        "Space" => (PhysicalKey::Space, LogicalKey::Character(' '), KeyLocation::Standard),
        "Enter" => (PhysicalKey::Enter, LogicalKey::Named(NamedKey::Enter), KeyLocation::Standard),
        "Escape" => (PhysicalKey::Escape, LogicalKey::Named(NamedKey::Escape), KeyLocation::Standard),
        "Tab" => (PhysicalKey::Tab, LogicalKey::Named(NamedKey::Tab), KeyLocation::Standard),
        "Backspace" => (PhysicalKey::Backspace, LogicalKey::Named(NamedKey::Backspace), KeyLocation::Standard),
        "ArrowUp" => (PhysicalKey::ArrowUp, LogicalKey::Named(NamedKey::ArrowUp), KeyLocation::Standard),
        "ArrowDown" => (PhysicalKey::ArrowDown, LogicalKey::Named(NamedKey::ArrowDown), KeyLocation::Standard),
        "ArrowLeft" => (PhysicalKey::ArrowLeft, LogicalKey::Named(NamedKey::ArrowLeft), KeyLocation::Standard),
        "ArrowRight" => (PhysicalKey::ArrowRight, LogicalKey::Named(NamedKey::ArrowRight), KeyLocation::Standard),
        "ShiftLeft" => (PhysicalKey::ShiftLeft, LogicalKey::Named(NamedKey::Shift), KeyLocation::Left),
        "ShiftRight" => (PhysicalKey::ShiftRight, LogicalKey::Named(NamedKey::Shift), KeyLocation::Right),
        "ControlLeft" => (PhysicalKey::ControlLeft, LogicalKey::Named(NamedKey::Control), KeyLocation::Left),
        _ => return None,
    };
    Some(KeyDescriptor {
        physical_key,
        logical_key,
        key_location,
    })
}

fn main() -> Result<()> {
    let mut swf = PathBuf::from(r"D:/gimped-work/scratch/BrawlhallaAir.standalone.swf");
    let mut depot = std::env::var_os("RUFFLE_DEPOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var("APPDATA").unwrap_or_default())
                .join(".gimped/patches/7887499569625607934/depot")
        });
    let mut port = 8090u16;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--swf" => swf = PathBuf::from(args.next().ok_or_else(|| anyhow!("--swf PATH"))?),
            "--depot" => {
                depot = PathBuf::from(args.next().ok_or_else(|| anyhow!("--depot PATH"))?)
            }
            "--port" => port = args.next().ok_or_else(|| anyhow!("--port PORT"))?.parse()?,
            other => return Err(anyhow!("unknown argument {other}")),
        }
    }

    let instance = create_wgpu_instance(
        wgpu::Backends::PRIMARY,
        wgpu::BackendOptions::default(),
        None,
    );
    let (adapter, device, queue) = futures::executor::block_on(request_adapter_and_device(
        wgpu::Backends::PRIMARY,
        &instance,
        None,
        wgpu::PowerPreference::HighPerformance,
    ))
    .map_err(|error| anyhow!("wgpu adapter: {error}"))?;
    let descriptors = Arc::new(Descriptors::new(instance, adapter, device, queue));

    let target = TextureTarget::new(&descriptors.device, (WIDTH, HEIGHT))
        .map_err(|error| anyhow!("offscreen target: {error}"))?;
    let movie = movie_from_path(&swf, None).map_err(|error| anyhow!("load swf: {error}"))?;

    let player = PlayerBuilder::new()
        .with_renderer(
            WgpuRenderBackend::new(descriptors.clone(), target)
                .map_err(|error| anyhow!("wgpu backend: {error}"))?,
        )
        .with_audio(NullAudioBackend::new())
        .with_filesystem(Box::new(DiskFileSystem::new(depot)))
        .with_movie(movie)
        .with_player_runtime(PlayerRuntime::AIR)
        .with_player_version(Some(51))
        .with_viewport_dimensions(WIDTH, HEIGHT, 1.0)
        .with_load_behavior(LoadBehavior::Streaming)
        .build();

    let frame = Arc::new(LatestFrame::new());

    let server = Server::http(("127.0.0.1", port))
        .map_err(|error| anyhow!("http server: {error}"))?;
    println!("http://127.0.0.1:{port}/");

    let frame_for_server = frame.clone();
    let (input_tx, input_rx) = channel();
    std::thread::spawn(move || {
        if let Err(error) = serve_page(&server, frame_for_server, input_tx) {
            eprintln!("server error: {error:#}");
        }
    });

    let mut next = Instant::now();
    let mut capture_counter = 0u64;
    let mut frame_index = 0u64;
    let auto_script = [
        (1800u64, "Enter"),
        (1900, "Enter"),
        (2000, "Space"),
        (2200, "KeyC"),
        (2400, "KeyZ"),
        (2500, "KeyX"),
        (2600, "KeyC"),
        (2700, "KeyV"),
        (2800, "KeyW"),
        (2900, "KeyA"),
        (3000, "KeyD"),
        (3100, "KeyZ"),
        (3200, "KeyX"),
        (3300, "Enter"),
        (3400, "KeyZ"),
        (3500, "KeyX"),
    ];
    loop {
        if let Some((_, code)) = auto_script.iter().find(|(f, _)| *f == frame_index) {
            if let Some(key) = key_descriptor(code) {
                let mut p = player.lock().unwrap();
                p.handle_event(PlayerEvent::KeyDown { key });
                p.handle_event(PlayerEvent::KeyUp { key });
            }
        }
        frame_index += 1;
        while let Ok(command) = input_rx.try_recv() {
            let event = if command.down {
                PlayerEvent::KeyDown { key: command.key }
            } else {
                PlayerEvent::KeyUp { key: command.key }
            };
            player.lock().unwrap().handle_event(event);
        }

        player
            .lock()
            .unwrap()
            .preload(&mut ExecutionLimit::none());

        player.lock().unwrap().run_frame();

        capture_counter += 1;
        if capture_counter % 2 == 0 {
            if let Some(image) = capture(&player) {
                if let Ok(jpeg) = encode_jpeg(&image) {
                    frame.publish(jpeg);
                }
            }
        }

        next += Duration::from_micros(16_666);
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        } else {
            next = now;
        }
    }
}
