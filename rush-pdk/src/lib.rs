pub mod raw;

use std::collections::HashMap;

pub use extism_pdk::{self, *};
use rush_pdk_derive::bindings;
pub use rush_plugins::api::{self as plugins_api, *};

bindings! {
    output_text(text: &str) -> ();
}

/// Send and revieve the host's environment variables.
///
/// Each plugin has its own, isolated environment variables, but it may be neccesary to access the application's
/// (e.g. to get the PATH variable). This module contains function for sending and retrieving the
/// application's environment variables.
pub mod env {
    use super::*;

    bindings! {
        /// Get a single environment variable from the application
        env::get(key: &str) -> box Json<Option<String>>;
        /// Set a single environment variable for the application
        env::set(key: &str, value: &str) -> ();
        /// Delete a single environment variable from the application
        env::delete(key: &str) -> ();
        /// Get a map of all environment variables from the application
        env::vars() -> box Json<HashMap<String, String>>;
    }

    /// Replace the plugin's environment variables with the application's.
    ///
    /// ```rs
    /// rusk_pdk::env::load_host_vars();
    /// std::env::var("PATH").unwrap();
    /// ```
    pub fn load_host_vars() {
        let host_env = self::vars().0;
        let old_env_vars = std::env::vars().map(|(name, _)| name);

        // remove environment variables that won't be replaced by the host
        for var in old_env_vars {
            if !host_env.contains_key(&var) {
                std::env::remove_var(var);
            }
        }

        for (key, value) in host_env {
            std::env::set_var(key, value);
        }
    }

    /// Send all of the plugin's environment variables to the application's environment.
    pub fn send_host_vars() {
        for (key, value) in std::env::vars() {
            self::set(&key, &value);
        }
    }
}

pub mod fs {
    use super::*;

    bindings! {
        fs::is_executable(path: &str) -> bool;
    }
}
