use std::fmt::Display;
use std::path::PathBuf;

use crate::error_fmt;

pub trait ExecErrorKind: Into<ExecErrorCategory> {}

#[derive(Debug)]
pub struct ExecError {
    kind: ExecErrorCategory,
    context: ExecErrorContext,
}

impl Display for ExecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_fmt(f, &self.kind, &self.context)
    }
}

#[derive(Debug)]
struct ExecErrorContext {
    command_type: CommandType,
    command_name: String,
    command_args: Vec<String>,
}

impl Display for ExecErrorContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "[TYPE]: {}", self.command_type)?;
        writeln!(f, "[COMMAND]: {}", self.command_name)?;
        writeln!(f, "[ARGUMENTS]: {}", self.command_args.join(", "))
    }
}

#[derive(Debug)]
pub enum CommandType {
    Builtin,
    Executable,
}

impl Display for CommandType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use CommandType::*;
        write!(
            f,
            "{}",
            match self {
                Builtin => "Builtin",
                Executable => "Executable",
            }
        )
    }
}

#[derive(Debug)]
pub enum ExecErrorCategory {
    Argument(ArgumentError),
    Runtime(RuntimeError),
    Filesystem(FilesystemError),
    Terminal(TerminalError),
}

impl Display for ExecErrorCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use ExecErrorCategory::*;
        let category_str = format!("[CATEGORY]: {}", match self {
            Argument(_) => "Argument",
            Runtime(_) => "Runtime",
            Filesystem(_) => "Filesystem",
            Terminal(_) => "Terminal",
        });

        let message_str = format!("[MESSAGE]: {}", match self {
            Argument(e) => format!("{}", e),
            Runtime(e) => format!("{}", e),
            Filesystem(e) => format!("{}", e),
            Terminal(e) => format!("{}", e),
        });

        write!(f, "{}\n{}", category_str, message_str)
    }
}

impl From<ArgumentError> for ExecErrorCategory {
    fn from(e: ArgumentError) -> Self {
        ExecErrorCategory::Argument(e)
    }
}

impl From<TerminalError> for ExecErrorCategory {
    fn from(e: TerminalError) -> Self {
        ExecErrorCategory::Terminal(e)
    }
}

impl From<FilesystemError> for ExecErrorCategory {
    fn from(e: FilesystemError) -> Self {
        ExecErrorCategory::Filesystem(e)
    }
}

impl From<RuntimeError> for ExecErrorCategory {
    fn from(e: RuntimeError) -> Self {
        ExecErrorCategory::Runtime(e)
    }
}

#[derive(Debug)]
pub enum ArgumentError {
    InvalidArgumentCount(usize, usize),
    InvalidArgument(String),
    InvalidValue(String),
}

impl ExecErrorKind for ArgumentError {}

impl Display for ArgumentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use ArgumentError::*;
        match self {
            InvalidArgumentCount(expected, actual) => {
                write!(f, "Expected {} arguments, got {}", expected, actual)
            }
            InvalidArgument(arg) => write!(f, "Invalid argument: {}", arg),
            InvalidValue(value) => write!(f, "Invalid value: {}", value),
        }
    }
}

#[derive(Debug)]
pub enum TerminalError {
    FailedToParseStdout(String),
    FailedToParseStderr(String),
}

impl ExecErrorKind for TerminalError {}

impl Display for TerminalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use TerminalError::*;
        match self {
            FailedToParseStdout(stdout) => write!(f, "Failed to parse stdout: {}", stdout),
            FailedToParseStderr(stderr) => write!(f, "Failed to parse stderr: {}", stderr),
        }
    }
}

#[derive(Debug)]
pub enum FilesystemError {
    FailedToReadFileType(PathBuf),
    FailedToReadFileName(PathBuf),
    FailedToReadDirectory(PathBuf),
    PathNoLongerExists(PathBuf),
}

impl ExecErrorKind for FilesystemError {}

impl Display for FilesystemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use FilesystemError::*;
        match self {
            FailedToReadFileType(path) => {
                write!(f, "Failed to read file type: {}", path.display())
            }
            FailedToReadFileName(path) => {
                write!(f, "Failed to read file name: {}", path.display())
            }
            FailedToReadDirectory(path) => write!(f, "Failed to read directory: {}", path.display()),
            PathNoLongerExists(path) => write!(
                f,
                "Previously-valid path no longer exists: {}",
                path.display()
            ),
        }
    }
}

#[derive(Debug)]
pub enum RuntimeError {
    // TODO: Maybe add more info for known error codes?
    FailedToExecute(isize),
    // $ This is way too general - because we know the information about exactly how a builtin failed,
    // $ we should be able to provide a more specific error message
    FailedToRun,
}

impl ExecErrorKind for RuntimeError {}

impl Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use RuntimeError::*;
        match self {
            FailedToExecute(code) => write!(f, "Executable failed to run with exit code: {}", code),
            FailedToRun => write!(f, "Failed to run builtin for some reason"),
        }
    }
}

impl ExecError {
    pub fn new(kind: impl ExecErrorKind, command_type: CommandType, command_name: &str, command_args: Vec<String>) -> Self {
        ExecError {
            kind: kind.into(),
            context: ExecErrorContext {
                command_type,
                command_name: command_name.to_owned(),
                command_args,
            },
        }
    }
}
