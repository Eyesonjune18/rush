use std::fmt::{Display, Formatter};
use std::fs::canonicalize;
use std::path::PathBuf;

use anyhow::Result;

use crate::errors::ShellError;

// Wrapper class for a directory path string
pub struct Path {
    absolute_path: PathBuf,
    home_directory: PathBuf,
    shortened_path: String,
    truncation_factor: Option<usize>,
}

impl Display for Path {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.absolute_path.display())
    }
}

impl Path {
    // Safely constructs a new Path from a given directory, taking into account
    // the user's home directory so it can be collapsed into a shorthand '~'
    pub fn new(absolute_path: PathBuf, home_directory: &PathBuf) -> Result<Self> {
        let home_directory = home_directory.clone();
        let mut path = Self {
            absolute_path,
            home_directory,
            shortened_path: String::new(),
            truncation_factor: None,
        };

        path.update_shortened_path()?;
        Ok(path)
    }

    // Attempts to construct a new Path from a given path string by resolving it to an absolute path
    pub fn from_str_path(path: &str, home_directory: &PathBuf) -> Result<Self> {
        match resolve(path, home_directory) {
            Some(absolute_path) => Ok(Self::new(absolute_path, home_directory)?),
            None => Err(ShellError::UnknownDirectory.into()),
        }
    }

    // Gets the absolute path, with all directory names included
    pub fn absolute(&self) -> &PathBuf {
        &self.absolute_path
    }

    // Gets the shortened version of the path
    // If truncation is enabled, the path will be truncated
    // The shortened path will always have the home directory collapsed
    pub fn short(&self) -> &String {
        &self.shortened_path
    }

    // Sets the Path truncation factor
    pub fn set_truncation(&mut self, factor: usize) -> Result<()> {
        self.truncation_factor = Some(factor);
        self.update_shortened_path()
    }

    // Disables Path truncation
    pub fn disable_truncation(&mut self) -> Result<()> {
        self.truncation_factor = None;
        self.update_shortened_path()
    }

    // Re-generates the shortened path based on the current settings
    fn update_shortened_path(&mut self) -> Result<()> {
        // ? Is there a less redundant way to write this?
        let path = match self.absolute_path.strip_prefix(&self.home_directory) {
            Ok(path) => {
                let mut path_string = path.to_string_lossy().to_string();
                // ? Is this really necessary? Wouldn't it be fine to just have '~/'?
                path_string = match path_string.len() {
                    0 => String::from("~"),
                    _ => format!("~/{}", path_string),
                };

                path_string
            }
            Err(_) => self.absolute_path.to_string_lossy().to_string(),
        };

        // ! This might cause a bug with directories that have a '/' in their name
        // ! Also might cause a bug with non-unicode characters (paths use OsString which is not guaranteed to be valid unicode)
        let directories: Vec<String> = path.split("/").map(|d| d.to_string()).collect();
        let mut truncated_directories = Vec::new();

        if let Some(factor) = self.truncation_factor {
            for dir in directories {
                let mut truncated_dir = dir.clone();
                if dir.len() > factor {
                    truncated_dir.truncate(factor);
                }

                truncated_directories.push(truncated_dir);
            }
        } else {
            truncated_directories = directories;
        }

        let truncated_directories = truncated_directories.join("/");
        Ok(self.shortened_path = truncated_directories)
    }

    // Updates the Path using a new absolute path
    pub fn set_path(&mut self, new_path: &str) -> Result<()> {
        // ? Should this be a FailedToCanonicalizePath error?
        let new_absolute_path =
            resolve(new_path, &self.home_directory).ok_or_else(|| ShellError::UnknownDirectory)?;
        self.absolute_path = new_absolute_path;
        self.update_shortened_path()
    }
}

// Attempts to convert a path string into a canonicalized absolute path
// ? Should this be a Result instead of an Option?
pub fn resolve(path: &str, home_directory: &PathBuf) -> Option<PathBuf> {
    // The home directory shorthand must be expanded before resolving the path,
    // because PathBuf is not user-aware and only uses absolute and relative paths
    let expanded_path = match expand_home(path, home_directory) {
        Ok(path) => path,
        Err(_) => return None,
    };

    // Canonicalizing a path will resolve any relative or absolute paths
    let absolute_path = match canonicalize(expanded_path) {
        Ok(path) => path,
        Err(_) => return None,
    };

    // If the file system can canonicalize the path, it most likely exists,
    // but this is added just in case
    if !absolute_path.exists() {
        None
    } else {
        Some(absolute_path)
    }
}

// ! This is a temporary function whose functionality should potentially be shared with resolve()
pub fn resolve_executable(name: &str, path: &Vec<PathBuf>) -> Option<PathBuf> {
    for dir in path {
        let mut path = dir.clone();
        path.push(name);

        if path.exists() {
            return Some(path);
        }
    }

    None
}

fn expand_home(path: &str, home_directory: &PathBuf) -> Result<String> {
    if path.starts_with("~") {
        Ok(path.replace(
            "~",
            match home_directory.to_str() {
                Some(path) => path,
                None => return Err(ShellError::FailedToConvertPathBufToString.into()),
            },
        ))
    } else {
        Ok(path.to_string())
    }
}
