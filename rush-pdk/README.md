# Rush Plugin Development Kit

> Write plugins that extend the behavior of the Rush shell

## Installation

```sh
cargo add https://github.com/Eyesonjune18/rush.git rush-pdk
```

## Overview

Plugins are WebAssembly (`wasm`) modules that can provide unique functionality to the Rush shell. The Rush PDK allows your plugins to react to events broadcasted by Rush and affect the shell.


Rust 🦀 compiled to one of the `wasm32-wasi` (recommended) or `wasm32-unknown-unknown` targets is the officially supported method for writing plugins.

```sh
rustup target add wasm32-wasi
```

## Usage

Rush PDK provides a `plugin_fn` attribute which enables the Rush shell to run your event listeners. The name of the function you apply it to determines when it will be run.

```rs
use rush_pdk::*;

#[plugin_fn]
pub fn rush_plugin_init(init_info: Json<InitHookParams>) -> FnResult<Json<()>> {
    // print a welcome message on startup
    let msg = format!("Welcome to Rush v{}!", init_info.0.rush_version);
    output_text(msg);

    Ok(Json(()))
}
```

When building your plugins, make sure to use the correct target:

```sh
cargo build --release --target wasm32-wasi
```

### Hooks

Hook implementations must have the `#[plugin_fn]` attribute to correctly interact with the shell. The shell runs hooks sequentially, with the plugins loaded earliest getting a higher priority.

- `rush_plugin_init(Json<InitHookParams>) -> FnResult<()>`
  - Called once after the plugin has been loaded
- `rush_plugin_deinit() -> FnResult<()>`
  - May be called before the plugin is unloaded
- `provide_autocomplete(String) -> FnResult<String>`
  - Called as the user types to request autocomplete
  - The hook takes the partial command as an argument and returns what it thinks the user will type next. If your plugin returns an empty string or doesn't implement this hook, the shell will request a completion from a different plugin (if any).

## Plugin Authoring Tips

If your plugin starts getting too big or takes too long to start up, consider enabling LTO:

```toml
[profile.release]
lto = true
```
