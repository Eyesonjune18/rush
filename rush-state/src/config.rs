use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use anyhow::Result;
use fs_err::File;

use crate::errors::ShellError;

// Represents any settings for the shell, most of which can be configured by the user
#[derive(Debug)]
pub struct Configuration {
    // The truncation length for the prompt
    pub truncation_factor: Option<usize>,
    // How many directories to store in the back/forward history
    pub history_limit: Option<usize>,
    // Whether or not to print out full error messages and status codes when a command fails
    pub show_errors: bool,
    /// List of plugins to load. Can be paths to directories (will be searched for .wasm files) and files
    pub plugins: Vec<PathBuf>,
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            truncation_factor: None,
            history_limit: None,
            show_errors: true,
            plugins: Vec::new(),
        }
    }
}

impl Configuration {
    // Scans a configuration file for settings and updates the configuration accordingly
    pub fn from_file(filename: &str) -> Result<Self> {
        let filename = PathBuf::from(filename);

        let mut config = Self::default();
        let file = File::open(&filename)
            .map_err(|_| ShellError::FailedToOpenConfigFile(filename.clone()))?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line.map_err(|_| ShellError::FailedToOpenConfigFile(filename.clone()))?;
            let tokens = line.split(": ").collect::<Vec<&str>>();
            if tokens.len() != 2 {
                return Err(ShellError::FailedToReadConfigFile(filename).into());
            }

            let (key, value) = (tokens[0], tokens[1]);

            // ? Should these be underscores instead of hyphens?
            match key {
                "truncation-factor" => {
                    if let Ok(length) = value.parse::<usize>() {
                        config.truncation_factor = Some(length);
                    } else if value == "false" {
                        config.truncation_factor = None;
                    }
                }
                "history-limit" => {
                    if let Ok(limit) = value.parse::<usize>() {
                        config.history_limit = Some(limit);
                    } else if value == "false" {
                        config.history_limit = None;
                    }
                }
                "show-errors" => {
                    if let Ok(show) = value.parse::<bool>() {
                        config.show_errors = show;
                    }
                }
                "plugin" => {
                    let mut config_dir = filename.parent().unwrap().to_path_buf();
                    config_dir.push(value);
                    config.plugins.push(config_dir);
                }
                _ => return Err(ShellError::FailedToReadConfigFile(filename).into()),
            }
        }

        Ok(config)
    }
}
