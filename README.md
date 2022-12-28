# puce8
A chip8 emulator.

## Build

cargo build --release

quick start:
cargo run -- path/to/bin.ch8

wasm build:
cargo build --target=wasm32-unknown-emscripten --release && emrun index.html
