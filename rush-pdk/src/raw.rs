//! Low-level bindings to functions provided by the Rush shell.
//!
//! These are used internally by the Rush PDK and use pointers to
//! shared memory as arguments instead of developer-friendly Rust types.

extern "C" {
    pub fn output_text(text: u64);

    // Functions for modifying the host's environment variables.
    // Each plugin has its own, isolated set so these can be used to sync between the host and plugin.

    /// Get a single environment variable as a JSON string or JSON null.
    pub fn env_get(var_name: u64) -> u64;
    pub fn env_set(var_name: u64, var_value: u64);
    pub fn env_delete(var_name: u64);
    /// Get all environment variables as a JSON object.
    pub fn env_vars() -> u64;

    pub fn fs_is_executable(path: u64) -> bool;
}
