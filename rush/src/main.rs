use std::path::Path;
use std::sync::{Arc, RwLock};

use anyhow::Result;
use is_executable::is_executable;
use rush_eval::dispatcher::Dispatcher;
use rush_eval::errors::DispatchError;
use rush_plugins::api::InitHookParams;
use rush_plugins::plugin::HostBindings;
use rush_plugins::{CurrentPlugin, PluginHost, HOST_BINDINGS};
use rush_state::console::{restore_terminal, Console};
use rush_state::shell::Shell;
use rush_state::showln;

fn main() -> Result<()> {
    // The Console type is responsible for reading and writing to the terminal (TUI),
    // and providing an interface for any commands that need to produce output and/or take input
    let console = Arc::new(RwLock::new(Console::<'static>::new()?));

    // The PluginHost type is responsible for loading and managing plugins
    let plugins = PluginHost::new({
        let console = console.clone();
        Arc::new(move |err| {
            showln!(console.write().unwrap(), "Plugin error: {}", err);
        })
    });

    // The Shell type stores all of the state for the shell, including its configuration,
    // its environment, and other miscellaneous data like command history
    let mut shell = Shell::new(plugins)?;
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        default_panic(info);
    }));
    // The Dispatcher type is responsible for resolving command names to actual function calls,
    // or executables if needed, and then invoking them with the given arguments
    let dispatcher = Dispatcher::default();

    console.write().unwrap().enter()?;

    // Create bindings so that plugins can communicate with the shell
    *HOST_BINDINGS.lock().unwrap() = Box::new(RushBindings {
        console: console.clone(),
    });

    // Load plugins from the config file
    let init_params = InitHookParams {
        rush_version: env!("CARGO_PKG_VERSION").to_string(),
    };
    for plugin_path in shell.config().plugins.clone() {
        let path = Path::new(&plugin_path);
        if let Err(err) = shell.plugins.load(path, &init_params) {
            showln!(console.write().unwrap(), "Failed to load plugin: {}", err);
        }
    }

    loop {
        let line = Console::read_line(&console, &mut shell)?;
        let status = dispatcher.eval(&mut shell, &mut console.write().unwrap(), &line);
        handle_error(status, &mut shell, &mut console.write().unwrap());

        shell.history_add(line);
    }
}

// Prints an appropriate error message for the given error, if applicable
fn handle_error(error: Result<()>, shell: &mut Shell, console: &mut Console) {
    match error {
        Ok(_) => shell.set_success(true),
        Err(e) => {
            match e.downcast_ref::<DispatchError>() {
                Some(DispatchError::UnknownCommand(command_name)) => {
                    showln!(console, "Unknown command: {}", command_name);
                }
                _ => {
                    if shell.config().show_errors {
                        // TODO: This is sort of a "magic" formatting string, it should be changed to a method or something
                        showln!(console, "Error: {:#?}: {}", e, e);
                    }
                }
            }

            shell.set_success(false);
        }
    }
}

struct RushBindings {
    console: Arc<RwLock<Console<'static>>>,
}

impl HostBindings for RushBindings {
    fn env_delete(&mut self, _: &mut CurrentPlugin, var_name: String) {
        std::env::remove_var(var_name);
    }
    fn env_get(&mut self, _: &mut CurrentPlugin, var_name: String) -> Option<String> {
        std::env::var(var_name).ok()
    }
    fn env_set(&mut self, _: &mut CurrentPlugin, var_name: String, var_value: String) {
        std::env::set_var(var_name, var_value);
    }
    fn env_vars(&mut self, _: &mut CurrentPlugin) -> std::collections::HashMap<String, String> {
        std::env::vars().collect()
    }
    fn fs_is_executable(&mut self, _: &mut CurrentPlugin, path: String) -> bool {
        is_executable(path)
    }
    fn output_text(&mut self, _: &mut CurrentPlugin, input: String) {
        showln!(self.console.write().unwrap(), "{}", input);
    }
}
