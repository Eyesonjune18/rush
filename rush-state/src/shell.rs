use anyhow::Result;
use rush_plugins::PluginHost;

use crate::config::Configuration;
use crate::environment::Environment;

// Represents the shell, its state, and provides methods for interacting with it
// ? Should this be called ShellState or something like that?
// TODO: Miscellaneous shell state like command_success, command_history etc might be better off in some sort of bundle struct
pub struct Shell {
    pub(crate) environment: Environment,
    pub(crate) config: Configuration,
    pub(crate) command_success: bool,
    pub(crate) command_history: Vec<String>,
    pub plugins: PluginHost,
}

impl Shell {
    pub fn new(plugins: PluginHost) -> Result<Self> {
        let config =
            Configuration::from_file("config/config.rush").unwrap_or(Configuration::default());

        Ok(Self {
            environment: Environment::new()?,
            config,
            command_success: true,
            command_history: Vec::new(),
            plugins,
        })
    }

    pub fn env(&self) -> &Environment {
        &self.environment
    }

    pub fn env_mut(&mut self) -> &mut Environment {
        &mut self.environment
    }

    pub fn config(&self) -> &Configuration {
        &self.config
    }

    pub fn config_mut(&mut self) -> &mut Configuration {
        &mut self.config
    }

    pub fn success(&self) -> bool {
        self.command_success
    }

    pub fn set_success(&mut self, success: bool) {
        self.command_success = success;
    }

    pub fn history(&self) -> &Vec<String> {
        &self.command_history
    }

    // Adds a line of input to the command history
    // If it already exists in the history, brings the previous occurrence to the front
    pub fn history_add(&mut self, command: String) {
        match self.command_history.contains(&command) {
            true => {
                let index = self
                    .command_history
                    .iter()
                    .position(|c| c == &command)
                    .unwrap();
                self.command_history.remove(index);
            }
            false => (),
        }

        self.command_history.push(command)
    }
}
