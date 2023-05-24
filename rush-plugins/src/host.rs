mod bindings;
pub mod plugin;

use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::thread;

pub use bindings::HOST_BINDINGS;
use extism::Context;
pub use extism::CurrentPlugin;
use plugin::{IoSnafu, RushPlugin, RushPluginError};
use snafu::ResultExt;

use crate::api::InitHookParams;

enum PluginRunnerMessage {
    Hook {
        hook_name: String,
        hook_params: Vec<u8>,
        callback: HookBroadcastCallback,
    },
    Load {
        path: PathBuf,
        init_params: Vec<u8>,
        callback: oneshot::Sender<Result<(), RushPluginError>>,
    },
}

struct PluginHookResponse {
    payload: Vec<u8>,
    plugin_name: String,
}

enum HookBroadcastCallback {
    NoPayload(oneshot::Sender<()>),
    WithPayloads(oneshot::Sender<Vec<PluginHookResponse>>),
}

pub type ErrorReporter = Arc<dyn Fn(RushPluginError) + Send + Sync>;

/// Runs the error reporter in a new thread to prevent blocking the plugin host thread.
fn call_error_reporter(report_error: ErrorReporter, err: RushPluginError) {
    thread::spawn(move || report_error(err));
}

/// A struct that can be used to communicate with a plugin host thread.
///
/// Once plugins are loaded, they can be interacted with via the methods on this struct.
#[derive(Clone)]
pub struct PluginHost {
    tx: mpsc::Sender<PluginRunnerMessage>,
    report_error: ErrorReporter,
}

impl PluginHost {
    /// Spawn a new plugin host thread and return a struct that can be used to communicate with it.
    ///
    /// # Arguments
    ///
    /// - `report_error`: A function that will be called whenever a plugin returns an error.
    ///    If the error is due to the plugin panicking or misusing the API, the plugin will removed after this function is called.
    pub fn new(report_error: ErrorReporter) -> Self {
        let (tx, rx) = mpsc::channel();
        {
            let report_error = Arc::clone(&report_error);
            thread::spawn(move || {
                let ctx = Box::leak(Box::new(Context::new()));
                let mut plugins = Vec::new();

                loop {
                    let Ok(msg) = rx.recv() else {
                    break;
                };

                    match msg {
                        PluginRunnerMessage::Load {
                            path,
                            init_params,
                            callback,
                        } => {
                            callback
                                .send(
                                    RushPlugin::new(&path, ctx, &init_params)
                                        .map(|plugin| plugins.push(plugin)),
                                )
                                .unwrap();
                        }
                        PluginRunnerMessage::Hook {
                            hook_name,
                            hook_params,
                            callback,
                        } => {
                            let return_values = plugins
                                .iter_mut()
                                .map(|plugin| {
                                    let name = plugin.name().to_owned();
                                    (
                                        plugin
                                            .call_hook_if_exists(&hook_name, &hook_params)
                                            .map(|payload| payload.map(|payload| payload.to_vec())),
                                        name,
                                    )
                                })
                                .collect::<Vec<_>>()
                                .into_iter()
                                .enumerate()
                                .rev() // removing higher indexes first won't mess with the position of lower ones
                                .filter_map(|(index, (result, plugin_name))| match result {
                                    Ok(payload) => payload.map(|payload| PluginHookResponse {
                                        payload,
                                        plugin_name,
                                    }),
                                    Err(err) => {
                                        call_error_reporter(Arc::clone(&report_error), err);
                                        plugins.remove(index);
                                        None
                                    }
                                });

                            match callback {
                                HookBroadcastCallback::NoPayload(cb) => cb.send(()).unwrap(),
                                HookBroadcastCallback::WithPayloads(cb) => {
                                    cb.send(return_values.collect()).unwrap()
                                }
                            }
                        }
                    }
                }
            });
        }

        Self { tx, report_error }
    }

    /// Load a plugin from a file, with the init parameters pre-serialized.
    fn load_file_raw(&mut self, path: &Path, init_params: Vec<u8>) -> Result<(), RushPluginError> {
        let (callback, rx) = oneshot::channel();
        self.tx
            .send(PluginRunnerMessage::Load {
                path: path.to_owned(),
                init_params,
                callback,
            })
            .unwrap();
        rx.recv().unwrap()
    }

    /// Load a plugin from a file
    pub fn load_file(
        &mut self,
        path: &Path,
        init_params: &InitHookParams,
    ) -> Result<(), RushPluginError> {
        let serialized = serde_json::to_vec(init_params).with_context(|_| plugin::SerdeSnafu {
            name: path.display().to_string(),
        })?;

        self.load_file_raw(path, serialized)
    }

    /// Load a plugin from a file, with the init parameters pre-serialized.
    fn load_raw(&mut self, path: &Path, init_params: Vec<u8>) -> Result<(), RushPluginError> {
        if path.is_file() {
            self.load_file_raw(path, init_params)?;
        } else {
            let path_display = path.display().to_string();
            let subitems = path.read_dir().context(IoSnafu {
                name: &path_display,
            })?;

            for entry in subitems {
                let entry = entry.context(IoSnafu {
                    name: &path_display,
                })?;

                // if it's a file not ending in .wasm (i.e. not a plugin), skip it
                if !entry.file_name().to_string_lossy().ends_with(".wasm")
                    && entry
                        .file_type()
                        .context(IoSnafu {
                            name: &path_display,
                        })?
                        .is_file()
                {
                    continue;
                }

                self.load_raw(&entry.path(), init_params.clone())?;
            }
        }

        Ok(())
    }

    /// Load plugins from a file or directory.
    ///
    /// If the path is a directory, it will be recursively searched for files ending in `.wasm`, which will be loaded as plugins.
    pub fn load(
        &mut self,
        path: &Path,
        init_params: &InitHookParams,
    ) -> Result<(), RushPluginError> {
        let serialized = serde_json::to_vec(init_params).with_context(|_| plugin::SerdeSnafu {
            name: path.display().to_string(),
        })?;

        self.load_raw(path, serialized)
    }

    fn broadcast_hook_with_raw_outputs(
        &mut self,
        hook_name: String,
        hook_params: Vec<u8>,
    ) -> Vec<PluginHookResponse> {
        let (callback, rx) = oneshot::channel();
        self.tx
            .send(PluginRunnerMessage::Hook {
                hook_name,
                hook_params,
                callback: HookBroadcastCallback::WithPayloads(callback),
            })
            .unwrap();
        rx.recv().unwrap()
    }

    fn broadcast_hook_with_string_outputs(
        &mut self,
        hook_name: String,
        hook_params: Vec<u8>,
    ) -> Vec<String> {
        self.broadcast_hook_with_raw_outputs(hook_name, hook_params)
            .into_iter()
            .filter_map(|res| {
                match String::from_utf8(res.payload).with_context(|_| plugin::Utf8Snafu {
                    name: res.plugin_name.clone(),
                }) {
                    Ok(s) => Some(s),
                    Err(err) => {
                        call_error_reporter(Arc::clone(&self.report_error), err);
                        None
                    }
                }
            })
            .collect()
    }

    fn broadcast_hook(&mut self, hook_name: String, hook_params: Vec<u8>) {
        let (callback, rx) = oneshot::channel();
        self.tx
            .send(PluginRunnerMessage::Hook {
                hook_name,
                hook_params,
                callback: HookBroadcastCallback::NoPayload(callback),
            })
            .unwrap();
        rx.recv().unwrap();
    }

    /// Call the `rush_plugin_deinit` hook on all plugins so that they can perform any deinitialization they need to.
    ///
    /// This is called automatically when the PluginHost is dropped.
    pub fn deinit_plugins(&mut self) {
        self.broadcast_hook("rush_plugin_deinit".to_string(), Vec::new());
    }

    // TODO: might be good to add a "priority" system where plugins can specify how good their completions are
    pub fn request_autocomplete(&mut self, line_buffer: String) -> Vec<String> {
        self.broadcast_hook_with_string_outputs(
            "provide_autocomplete".to_string(),
            line_buffer.into(),
        )
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect()
    }
}

impl Drop for PluginHost {
    fn drop(&mut self) {
        self.deinit_plugins();
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use extism::CurrentPlugin;
    use is_executable::is_executable;
    use lazy_static::lazy_static;

    use super::*;
    use crate::plugin::HostBindings;

    struct TestHostBindings {
        env_vars: HashMap<String, String>,
    }

    impl HostBindings for TestHostBindings {
        fn output_text(&mut self, _plugin: &mut CurrentPlugin, text: String) {
            println!("{text}");
        }
        fn env_vars(&mut self, _plugin: &mut CurrentPlugin) -> HashMap<String, String> {
            self.env_vars.clone()
        }
        fn env_get(&mut self, _plugin: &mut CurrentPlugin, var_name: String) -> Option<String> {
            self.env_vars.get(&var_name).cloned()
        }
        fn fs_is_executable(&mut self, _plugin: &mut CurrentPlugin, path: String) -> bool {
            is_executable(path)
        }
    }

    #[ctor::ctor]
    fn set_test_bindings() {
        *bindings::HOST_BINDINGS.lock().unwrap() = Box::new(TestHostBindings {
            env_vars: HashMap::from([
                ("PATH".to_string(), "/usr/bin:/bin".to_string()),
                ("RUSH_TEST".to_string(), "1".to_string()),
            ]),
        });
    }

    lazy_static! {
        static ref ERROR_REPORTER: ErrorReporter = Arc::new(|err| {
            panic!("Plugin errored: {}", err);
        });
    }

    #[test]
    #[ignore = "requires welcome message plugin to be built"]
    fn load_example_plugin() {
        let mut registry = PluginHost::new(ERROR_REPORTER.clone());

        registry
            .load(
                Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../examples/plugins/welcome-message/target/wasm32-wasi/release/welcome_message.wasm")),
                &InitHookParams {
                    rush_version: "v1".to_string(),
                },
            )
            .unwrap();
    }

    #[test]
    #[ignore = "requires path autocomplete plugin to be built"]
    fn autocomplete() {
        let path = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../examples/plugins/path-autocomplete/target/wasm32-wasi/release/path_autocomplete.wasm")).canonicalize().unwrap();
        let mut registry = PluginHost::new(ERROR_REPORTER.clone());

        registry
            .load(
                &path,
                &InitHookParams {
                    rush_version: "v1".to_string(),
                },
            )
            .unwrap();

        assert_eq!(
            registry
                .request_autocomplete("realp".to_string())
                .get(0)
                .map(String::as_str),
            Some("ath"),
        );
    }
}
