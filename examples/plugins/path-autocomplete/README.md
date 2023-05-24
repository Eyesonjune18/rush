# PATH Autocomplete Plugin

Bare minimum plugin for providing PATH autocompletion in Rush.

## Ways this plugin could be improved

- Filesystem operations take a while; cache all available commands.
- Sort suggestions based on usage.
  (track when the user executes a command and increase the likelyhood to suggest it to them again)

## Building

```rs
rustup target add wasm32-wasi
cargo build --release --target wasm32-wasi
```

Plugin will be located at: `./target/wasm32-wasi/release/path_autocomplete.wasm`
