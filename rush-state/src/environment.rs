use std::collections::{HashMap, HashSet, VecDeque};
use std::env;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;

use anyhow::Result;

use crate::errors::ShellError;
use crate::path::Path;

// Identifier enum for safely accessing environment variables
// ? What's a good name for this?
#[allow(non_snake_case)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnvVar {
    User,
    Home,
    Cwd,
    Path,
}

impl Display for EnvVar {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::User => "USER",
                Self::Home => "HOME",
                Self::Cwd => "CWD",
                Self::Path => "PATH",
            }
        )
    }
}

impl EnvVar {
    // Does the same thing as .to_string(), but uses legacy environment variable names
    fn to_legacy_string(self) -> String {
        match self {
            Self::User => "USER".to_string(),
            Self::Home => "HOME".to_string(),
            Self::Cwd => "PWD".to_string(),
            Self::Path => "PATH".to_string(),
        }
    }
}

// Convenience struct for passing around a set of environment variables without duplicates
struct EnvVarBundle {
    vars: HashSet<EnvVar>,
}

impl<const N: usize> From<[EnvVar; N]> for EnvVarBundle {
    fn from(vars: [EnvVar; N]) -> Self {
        Self {
            vars: vars.into_iter().collect(),
        }
    }
}

impl EnvVarBundle {
    fn contains(&self, var: EnvVar) -> bool {
        self.vars.contains(&var)
    }
}

// Represents the shell environment by encapsulating the environment variables
// * Environment variables are represented in all caps by convention,
// * any fields that are not actual environment variables are represented in the usual snake_case
#[allow(non_snake_case)]
pub struct Environment {
    USER: String,
    HOME: PathBuf,
    CWD: Path,
    backward_directories: VecDeque<Path>,
    forward_directories: VecDeque<Path>,
    // * PATH is not to be confused with the WORKING_DIRECTORY. PATH is a list of directories which
    // * the shell will search for executables in. WORKING_DIRECTORY is the current directory the user is in.
    PATH: VecDeque<Path>,
    #[allow(dead_code)]
    custom_variables: HashMap<String, String>,
}

#[allow(non_snake_case)]
impl Environment {
    pub fn new() -> Result<Self> {
        let USER = get_parent_env_var(EnvVar::User)?;
        let HOME = PathBuf::from(get_parent_env_var(EnvVar::Home)?);
        let CWD = Path::from_str(get_parent_env_var(EnvVar::Cwd)?.as_str(), &HOME)?;
        let PATH = convert_path(get_parent_env_var(EnvVar::Path)?.as_str(), &HOME);

        Ok(Self {
            USER,
            HOME,
            CWD,
            backward_directories: VecDeque::new(),
            forward_directories: VecDeque::new(),
            PATH,
            custom_variables: HashMap::new(),
        })
    }

    // Updates the shell process's environment variables to match the internal representation
    fn update_process_env_vars(&self, vars: EnvVarBundle) -> Result<()> {
        // TODO: How to detect errors here?
        if vars.contains(EnvVar::User) {
            env::set_var("USER", &self.USER);
        }

        if vars.contains(EnvVar::Home) {
            env::set_var("HOME", &self.HOME);
        }

        if vars.contains(EnvVar::Cwd) {
            env::set_current_dir(self.CWD.path())
                .map_err(|_| ShellError::FailedToUpdateEnvironmentVariable(EnvVar::Cwd))?;
        }

        Ok(())
    }

    pub fn USER(&self) -> &String {
        &self.USER
    }

    pub fn HOME(&self) -> &PathBuf {
        &self.HOME
    }

    pub fn CWD(&self) -> &Path {
        &self.CWD
    }

    pub fn CWD_mut(&mut self) -> &mut Path {
        &mut self.CWD
    }

    pub fn PATH(&self) -> &VecDeque<Path> {
        &self.PATH
    }

    pub fn PATH_mut(&mut self) -> &mut VecDeque<Path> {
        &mut self.PATH
    }

    // Sets the current working directory and stores the previous working directory
    pub fn set_CWD(&mut self, new_directory: &str, history_limit: Option<usize>) -> Result<()> {
        let starting_directory = self.CWD.clone();
        let new_directory = Path::from_str(new_directory, &self.HOME)?;

        // Add the old directory to the history, avoiding duplicates
        if new_directory != starting_directory {
            self.CWD = new_directory;
            self.backward_directories.push_back(starting_directory);
            self.forward_directories.clear();

            if let Some(limit) = history_limit {
                while self.backward_directories.len() > limit {
                    self.backward_directories.pop_front();
                }
            }

            self.update_process_env_vars([EnvVar::Cwd].into())?;
        }

        Ok(())
    }

    // Sets the current working directory to the previous working directory
    pub fn go_back(&mut self) -> Result<()> {
        let starting_directory = self.CWD.clone();
        if let Some(previous_path) = self.backward_directories.pop_back() {
            self.CWD = previous_path;
            self.forward_directories.push_front(starting_directory);
            self.update_process_env_vars([EnvVar::Cwd].into())
        } else {
            Err(ShellError::NoPreviousDirectory.into())
        }
    }

    // Sets the current working directory to the next working directory
    pub fn go_forward(&mut self) -> Result<()> {
        let starting_directory = self.CWD.clone();
        if let Some(next_path) = self.forward_directories.pop_front() {
            self.CWD = next_path;
            self.backward_directories.push_back(starting_directory);
            self.update_process_env_vars([EnvVar::Cwd].into())
        } else {
            Err(ShellError::NoNextDirectory.into())
        }
    }
}

// Gets the name of the user who invoked the shell (to be used when the shell is first initialized)
fn get_parent_env_var(variable: EnvVar) -> Result<String> {
    std::env::var(variable.to_legacy_string())
        .map_err(|_| ShellError::MissingExternalEnvironmentVariable(variable).into())
}

// Converts the PATH environment variable from a string to a vector of Paths
fn convert_path(path: &str, home: &PathBuf) -> VecDeque<Path> {
    let mut paths = VecDeque::new();

    let path_strings = path.split(':').collect::<Vec<&str>>();
    for path_string in path_strings {
        let path = Path::from_str(path_string, home);
        // TODO: Handle errors
        if let Ok(path) = path {
            paths.push_back(path);
        }
    }

    paths
}
