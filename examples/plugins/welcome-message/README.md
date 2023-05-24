# Welcome message plugin

See a friendly welcome message when starting up Rush! 😄

## Building

```rs
rustup target add wasm32-wasi
cargo build --release --target wasm32-wasi
```

Plugin will be located at: `./target/wasm32-wasi/release/welcome_message.wasm`
