//! Real-time Windows system audio -> iPhone Safari speaker streaming over LAN.
//!
//! Rust rewrite of the original Python `server.py` (audio2ios). Captures WASAPI
//! loopback audio on a dedicated thread and broadcasts raw 16-bit PCM over a
//! WebSocket. The Safari client (static/) is unchanged.
//!
//! ```text
//! WASAPI loopback -> capture thread -> 960-frame chunker -> broadcast channel
//!                                                                  |
//!   iPhone Safari <- AudioWorklet ring buffer <- WebSocket client <-/
//! ```

use std::collections::VecDeque;
use std::net::UdpSocket;
use std::path::PathBuf;
use std::sync::mpsc;

use anyhow::{anyhow, Result};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use bytes::Bytes;
use serde::Serialize;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tower_http::services::ServeDir;
use wasapi::{
    initialize_mta, DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat,
};

/// 960 frames = 20ms at 48kHz. Matches the server chunk size the client expects.
const CHUNK_FRAMES: usize = 960;

#[derive(Clone, Serialize)]
struct Meta {
    #[serde(rename = "sampleRate")]
    sample_rate: u32,
    channels: u16,
    #[serde(rename = "bitsPerSample")]
    bits_per_sample: u16,
    #[serde(rename = "chunkFrames")]
    chunk_frames: usize,
}

#[derive(Clone)]
struct AppState {
    meta: Meta,
    tx: broadcast::Sender<Bytes>,
}

fn main() -> Result<()> {
    // The WASAPI capture runs on its own OS thread with a COM apartment; it reports
    // the negotiated audio format back over `fmt_rx` so the HTTP server can answer
    // /meta before any client connects.
    let (audio_tx, _) = broadcast::channel::<Bytes>(256);
    let (fmt_tx, fmt_rx) = mpsc::channel::<(u32, u16)>();

    let capture_tx = audio_tx.clone();
    std::thread::Builder::new()
        .name("wasapi-capture".into())
        .spawn(move || {
            if let Err(e) = capture_loop(fmt_tx, capture_tx) {
                eprintln!("Capture thread error: {e:#}");
                std::process::exit(1);
            }
        })?;

    let (sample_rate, channels) = fmt_rx
        .recv()
        .map_err(|_| anyhow!("capture thread exited before reporting audio format"))?;

    let meta = Meta {
        sample_rate,
        channels,
        bits_per_sample: 16,
        chunk_frames: CHUNK_FRAMES,
    };

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(serve(AppState { meta, tx: audio_tx }))
}

/// Captures WASAPI loopback PCM and pushes fixed-size chunks into the broadcast
/// channel. Runs for the life of the process.
fn capture_loop(fmt_tx: mpsc::Sender<(u32, u16)>, tx: broadcast::Sender<Bytes>) -> Result<()> {
    initialize_mta().ok()?;

    // Loopback = capture the *render* (output) device. wasapi enables loopback
    // automatically when a render device is opened in the Capture direction.
    let enumerator = DeviceEnumerator::new()?;
    let device = enumerator.get_default_device(&Direction::Render)?;
    let mut audio_client = device.get_iaudioclient()?;

    let mix = audio_client.get_mixformat()?;
    let sample_rate = mix.get_samplespersec();
    let channels = mix.get_nchannels();

    // Request 16-bit signed PCM at the device's native rate/channels. `autoconvert`
    // lets WASAPI down-convert from the engine's float mix format for us.
    let desired = WaveFormat::new(16, 16, &SampleType::Int, sample_rate as usize, channels as usize, None);
    let (default_period, _min_period) = audio_client.get_device_period()?;
    let mode = StreamMode::EventsShared {
        autoconvert: true,
        buffer_duration_hns: default_period,
    };
    audio_client.initialize_client(&desired, &Direction::Capture, &mode)?;

    let event = audio_client.set_get_eventhandle()?;
    let capture_client = audio_client.get_audiocaptureclient()?;

    println!("Loopback: {}", device.get_friendlyname().unwrap_or_default());
    println!("Format: {sample_rate}Hz, {channels}ch, 16-bit");
    let _ = fmt_tx.send((sample_rate, channels));

    audio_client.start_stream()?;

    let chunk_bytes = CHUNK_FRAMES * channels as usize * 2;
    let mut queue: VecDeque<u8> = VecDeque::with_capacity(chunk_bytes * 8);

    loop {
        capture_client.read_from_device_to_deque(&mut queue)?;
        while queue.len() >= chunk_bytes {
            let chunk: Vec<u8> = queue.drain(..chunk_bytes).collect();
            // Err only means there are currently no subscribers — that's fine.
            let _ = tx.send(Bytes::from(chunk));
        }
        if event.wait_for_event(1_000_000).is_err() {
            audio_client.stop_stream()?;
            return Err(anyhow!("WASAPI event timed out"));
        }
    }
}

async fn serve(state: AppState) -> Result<()> {
    let static_dir = resolve_static_dir();

    let app = Router::new()
        .route("/meta", get(meta_handler))
        .route("/ws", get(ws_handler))
        // ServeDir serves index.html for "/" and the JS assets for everything else.
        .fallback_service(ServeDir::new(&static_dir))
        .with_state(state);

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let listener = TcpListener::bind(("0.0.0.0", port)).await?;

    println!("\nOpen in Safari: http://{}:{port}", local_ip());
    println!("Press Ctrl+C to stop\n");

    axum::serve(listener, app).await?;
    Ok(())
}

async fn meta_handler(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.meta)
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    // Mirror the Python server: send the meta JSON as the first (text) frame. The
    // client ignores string frames but this keeps the protocol identical.
    if let Ok(meta) = serde_json::to_string(&state.meta) {
        let _ = socket.send(Message::Text(meta)).await;
    }

    let mut rx = state.tx.subscribe();
    println!("Client connected");

    loop {
        tokio::select! {
            received = rx.recv() => match received {
                Ok(chunk) => {
                    if socket.send(Message::Binary(chunk.to_vec())).await.is_err() {
                        break;
                    }
                }
                // Slow client fell behind the broadcast buffer; skip the dropped
                // chunks and keep streaming the live audio (PCM tolerates gaps).
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            },
            // Drain incoming frames so close/ping are handled; the client never
            // sends application data.
            incoming = socket.recv() => match incoming {
                Some(Ok(_)) => {}
                _ => break,
            },
        }
    }

    println!("Client disconnected");
}

/// Locate the `static/` directory: next to the executable first (release layout),
/// then relative to the current dir (cargo run / dev). Falls back to "static".
fn resolve_static_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("static");
            if candidate.is_dir() {
                return candidate;
            }
        }
    }
    let cwd = PathBuf::from("static");
    if cwd.is_dir() {
        return cwd;
    }
    PathBuf::from("static")
}

/// Best-effort LAN IP discovery via the same UDP-connect trick the Python used.
fn local_ip() -> String {
    let sock = match UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(_) => return "127.0.0.1".into(),
    };
    if sock.connect("8.8.8.8:80").is_err() {
        return "127.0.0.1".into();
    }
    sock.local_addr()
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".into())
}
