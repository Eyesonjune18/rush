use std::collections::HashMap;
use std::fmt::Debug;
use std::fs;
use std::path::Path;
use std::string::FromUtf8Error;

use extism::manifest::Wasm;
use extism::{Context, CurrentPlugin, Manifest, Plugin};
use snafu::{ResultExt, Snafu};

use super::bindings::{
    ENV_DELETE_FN, ENV_GET_FN, ENV_SET_FN, ENV_VARS_FN, FS_IS_EXECUTABLE_FN, OUTPUT_TEXT_FN,
};

/// Implementations of functions that plugins can use.
#[allow(unused_variables)]
pub trait HostBindings: Send {
    fn output_text(&mut self, plugin: &mut CurrentPlugin, input: String) {}

    // Functions for modifying the host's environment variables.
    // Each plugin has its own, isolated set so these can be used to sync between the host and plugin.

    fn env_get(&mut self, plugin: &mut CurrentPlugin, var_name: String) -> Option<String> {
        None
    }
    fn env_set(&mut self, plugin: &mut CurrentPlugin, var_name: String, var_value: String) {}
    fn env_delete(&mut self, plugin: &mut CurrentPlugin, var_name: String) {}
    /// Get all environment variables as a JSON object.
    fn env_vars(&mut self, plugin: &mut CurrentPlugin) -> HashMap<String, String> {
        HashMap::new()
    }

    fn fs_is_executable(&mut self, plugin: &mut CurrentPlugin, path: String) -> bool {
        false
    }
}

/// A struct implementing [`HostBindings`] with only no-op methods.
pub struct NoOpHostBindings;
impl HostBindings for NoOpHostBindings {}

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum RushPluginError {
    #[snafu(display("plugin returned error ({name}, hook: {hook:?}): {source}"))]
    Extism {
        source: extism::Error,
        name: String,
        hook: Option<String>,
    },
    #[snafu(display("i/o error ({name}): {source}"))]
    Io {
        source: std::io::Error,
        name: String,
    },
    #[snafu(display("json serialization/deserialization failed ({name}): {source}"))]
    Serde {
        source: serde_json::Error,
        name: String,
    },
    #[snafu(display("plugin hook returned invalid utf8 ({name}): {source}"))]
    Utf8 { source: FromUtf8Error, name: String },
}

pub(crate) struct RushPlugin<'a> {
    pub instance: extism::Plugin<'a>,
    name: String,
}

impl<'a> RushPlugin<'a> {
    /// Load a plugin without reading from a file.
    pub fn from_bytes(
        bytes: impl Into<Vec<u8>>,
        context: &'a Context,
        name: String,
        init_hook_params: &[u8],
    ) -> Result<Self, RushPluginError> {
        let manifest = Manifest::new([Wasm::data(bytes)]).with_allowed_path("/", "/");
        let mut plugin = RushPlugin {
            instance: Plugin::new_with_manifest(
                context,
                &manifest,
                [
                    &*OUTPUT_TEXT_FN,
                    &*ENV_GET_FN,
                    &*ENV_SET_FN,
                    &*ENV_DELETE_FN,
                    &*ENV_VARS_FN,
                    &*FS_IS_EXECUTABLE_FN,
                ],
                true,
            )
            .context(ExtismSnafu {
                name: &name,
                hook: None,
            })?,
            name,
        };
        plugin.init(init_hook_params)?;
        Ok(plugin)
    }

    /// Read and load a plugin from a file.
    ///
    /// # Panics
    ///
    /// - Panics if the path does not contain a valid file name (ex: `/`, `/path/to/file/..`)
    pub fn new(
        path: &Path,
        context: &'a Context,
        init_hook_params: &[u8],
    ) -> Result<Self, RushPluginError> {
        let path_display = path.display().to_string();

        let bytes = fs::read(path).context(IoSnafu {
            name: &path_display,
        })?;
        Self::from_bytes(bytes, context, path_display, init_hook_params)
    }

    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    /// Call an exported function from the plugin, returning `None` if it is not implemented.
    pub fn call_hook_if_exists(
        &mut self,
        hook: &str,
        data: &[u8],
    ) -> Result<Option<&[u8]>, RushPluginError> {
        if self.instance.has_function(hook) {
            Ok(Some(self.instance.call(hook, data).with_context(|_| {
                ExtismSnafu {
                    name: &self.name,
                    hook: Some(hook.to_string()),
                }
            })?))
        } else {
            Ok(None)
        }
    }

    /// Perform any initialization required by the plugin implementation.
    fn init(&mut self, params: &[u8]) -> Result<(), RushPluginError> {
        self.call_hook_if_exists("rush_plugin_init", params)?;
        Ok(())
    }
}

impl Debug for RushPlugin<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RushPlugin")
            .field("name", &self.name)
            .finish()
    }
}
