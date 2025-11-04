# video-buffer

A high-performance, modular rendering framework for Rust + WebAssembly with triple-buffering support.

## Demo

[**Live Demo**](https://postrational.github.io/video-buffer/)

## Technologies used in demo

- **WebAssembly with Web Workers**
- **tiny-skia** - Pure-Rust 2D graphics library
- **fontdue** - Fast font rasterization

## Features

- **Triple-buffering**
- **Parallel rendering**
- **Frame queue** - Out-of-order frame handling with HashMap-based buffering
- **60 FPS target** - Frame rate limit
- **Full-screen support** - Dynamic `<canvas>` sizing for any viewport

## Architecture

```
Main Thread                    Worker Threads (8x)
    │                              │
    ├─ Frame Queue                 ├─ tiny-skia rendering
    ├─ Triple Buffer               ├─ fontdue text
    ├─ Display (Canvas 2D)         └─ ARGB → RGBA conversion
    └─ FPS tracking
```

## Build

```bash
# Build main module
cargo build --target wasm32-unknown-unknown --example tiny_skia_wasm_main --release
wasm-bindgen --out-dir examples/tiny_skia_wasm/dist --target web \
  target/wasm32-unknown-unknown/release/examples/tiny_skia_wasm_main.wasm

# Build worker module
cargo build --target wasm32-unknown-unknown --example tiny_skia_wasm_worker --release
wasm-bindgen --out-dir examples/tiny_skia_wasm/dist --target web \
  target/wasm32-unknown-unknown/release/examples/tiny_skia_wasm_worker.wasm
```