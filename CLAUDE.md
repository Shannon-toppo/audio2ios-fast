# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Real-time Windows system audio → iPhone Safari speaker streaming over LAN. **Rust** rewrite of
the Python `audio2ios` project (https://github.com/Shannon-toppo/audio2ios, checked out as the
sibling directory `../audiotoios` in the author's working tree). A Rust server captures
WASAPI loopback audio via the `wasapi` crate and broadcasts raw PCM over a WebSocket. The Safari
client plays it through AudioWorklet (with ScriptProcessorNode fallback for iOS 18 bugs).

The client (`static/`) started as a verbatim copy of the Python version — the wire protocol is
byte-compatible, so only the server was rewritten. It has since diverged in one area: the iOS
screen-wake handling in `index.html` (NoSleep `<video>` attachment, visibility re-arming).

## Commands

```bash
cargo run --release      # Build + start server on 0.0.0.0:8080 (or PORT env var)
cargo build --release    # Build only → target/release/audio2ios-fast.exe
cargo build              # Fast debug build for iterating
```

No tests, no linter beyond `cargo clippy`. The `static/` files are served as-is.

## Architecture

```
WASAPI Loopback → wasapi capture thread → 960-frame chunker → tokio::broadcast
                                                                      ↓
iPhone Safari ← AudioWorklet (ring buffer, preroll) ← Int16→Float32 ← WebSocket client
```

### Server (src/main.rs)
- **Two execution contexts**: a dedicated OS thread runs the blocking WASAPI capture loop in a
  COM apartment (`initialize_mta`); the tokio runtime runs the HTTP/WebSocket server. They are
  bridged by a `tokio::sync::broadcast` channel (audio chunks) plus a one-shot `mpsc` that hands
  the negotiated audio format to the HTTP side before any client connects.
- `capture_loop()` opens the **default render device** in the **Capture** direction — the `wasapi`
  crate enables loopback automatically for this combination. Format is the device mix rate/channels
  re-requested as 16-bit `Int` with `autoconvert: true` so WASAPI down-converts from float for us.
- Event-driven: `set_get_eventhandle()` + `wait_for_event()`; each wake reads available bytes into
  a `VecDeque<u8>`, which is drained into fixed `CHUNK_FRAMES`-sized chunks and broadcast.
- A slow WebSocket client that lags the broadcast buffer gets `RecvError::Lagged` — we skip the
  dropped chunks and keep streaming (PCM tolerates gaps; this is the back-pressure policy).
- `GET /meta` returns JSON config; `GET /ws` upgrades to WebSocket and first sends the meta JSON as
  a text frame (parity with Python; the client ignores it). `static/` is served via `ServeDir`.

### Client (static/index.html + static/pcm-worklet.js)
Near-identical to the Python project (see the wake-lock note above). The full client write-up
lives in the `audio2ios` repo's `CLAUDE.md`. Summary:
- Two-phase init: preload AudioContext/Worklet on page load, `resume()` in the tap handler (iOS).
- AudioWorklet dual-channel ring buffer with preroll; ScriptProcessorNode fallback for iOS 18.
- `binaryType = 'arraybuffer'`, transferable ArrayBuffers (no SharedArrayBuffer in Safari).

### Key Constants
| Constant | Value | Meaning |
|---|---|---|
| `CHUNK_FRAMES` (server) | 960 | 20ms at 48kHz, server chunk size |
| `broadcast::channel(256)` | 256 | Buffered chunks before a lagging client drops |
| `preRoll` (client) | 7200 | 150ms buffered before playback starts |
| `bufferSize` (client) | 19200 | 400ms ring buffer capacity per channel |

## Porting Notes (vs. Python `server.py`)
- `pyaudiowpatch` callback + `asyncio.Queue` → dedicated thread + `tokio::broadcast`.
- `aiohttp` → `axum` 0.7 + `tower-http` `ServeDir`.
- The Python `_suppress_connection_reset` handler is unnecessary — axum/hyper handle abrupt client
  disconnects (Safari does this routinely) gracefully on their own.
- Wire format, chunk size, and `/meta` JSON are kept byte-identical so the client is untouched.

## Safari-Specific Constraints
Same as the Python project — see the `audio2ios` repo's `CLAUDE.md`. UI strings are Japanese
(target audience).

## Licensing
MIT (`LICENSE`). `static/nosleep.js` is vendored NoSleep.js v0.12.0 (MIT, Rich Tibbett) — its
attribution lives in `THIRD-PARTY-NOTICES.md`. Keep that file in sync if any vendored asset is
added, updated, or patched.
