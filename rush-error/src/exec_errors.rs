use std::fmt::Display;
use std::path::PathBuf;

use crate::RushError;

pub trait CommandErrorKind: Into<CommandErrorCategory> {}

#[derive(Debug)]
pub struct CommandError {
    kind: CommandErrorCategory,
    context: CommandErrorContext,
}

impl Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind_str = format!("[ERROR]:\n{:4}", self.kind);
        let context_str = format!("[CONTEXT]:\n{:4}", self.context);
        write!(f, "{}\n{}", kind_str, context_str)
    }
}

#[derive(Debug)]
struct CommandErrorContext {
    command_type: CommandType,
    command_name: String,
    command_args: Vec<String>,
}

impl Display for CommandErrorContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let type_str = format!("[TYPE]: {}", self.command_type);
        let name_str = format!("[COMMAND]: {}", self.command_name);
        let args_str = format!("[ARGUMENTS]: {}", self.command_args.join(" "));
        write!(f, "{}\n{}\n{}", name_str, args_str, type_str)
    }
}

impl RushError for CommandError {}

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
pub enum CommandErrorCategory {
    Argument(ArgumentError),
    Runtime(RuntimeError),
    Filesystem(FilesystemError),
    Terminal(TerminalError),
}

impl Display for CommandErrorCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use CommandErrorCategory::*;
        let category_str = format!("[CATEGORY]: {}", match self {
            Argument(_) => format!("Argument"),
            Runtime(_) => format!("Runtime"),
            Filesystem(_) => format!("Filesystem"),
            Terminal(_) => format!("Terminal"),
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

impl From<ArgumentError> for CommandErrorCategory {
    fn from(e: ArgumentError) -> Self {
        CommandErrorCategory::Argument(e)
    }
}

impl From<TerminalError> for CommandErrorCategory {
    fn from(e: TerminalError) -> Self {
        CommandErrorCategory::Terminal(e)
    }
}

impl From<FilesystemError> for CommandErrorCategory {
    fn from(e: FilesystemError) -> Self {
        CommandErrorCategory::Filesystem(e)
    }
}

impl From<RuntimeError> for CommandErrorCategory {
    fn from(e: RuntimeError) -> Self {
        CommandErrorCategory::Runtime(e)
    }
}

#[derive(Debug)]
pub enum ArgumentError {
    InvalidArgumentCount(usize, usize),
    InvalidArgument(String),
    InvalidValue(String),
}

impl CommandErrorKind for ArgumentError {}

impl Display for ArgumentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use ArgumentError::*;
        write!(f, "{}", match self {
            InvalidArgumentCount(expected, actual) => {
                format!("Expected {} arguments, got {}", expected, actual)
            }
            InvalidArgument(arg) => format!("Invalid argument: {}", arg),
            InvalidValue(value) => format!("Invalid value: {}", value),
        })
    }
}

#[derive(Debug)]
pub enum TerminalError {
    FailedToParseStdout(String),
    FailedToParseStderr(String),
}

impl CommandErrorKind for TerminalError {}

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

impl CommandErrorKind for FilesystemError {}

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

impl CommandErrorKind for RuntimeError {}

impl Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use RuntimeError::*;
        match self {
            FailedToExecute(code) => write!(f, "Executable failed to run with exit code: {}", code),
            FailedToRun => write!(f, "Failed to run builtin for some reason"),
        }
    }
}

impl CommandError {
    pub fn new(kind: impl CommandErrorKind, command_type: CommandType, command_name: &str, command_args: Vec<String>) -> Self {
        CommandError {
            kind: kind.into(),
            context: CommandErrorContext {
                command_type,
                command_name: command_name.to_owned(),
                command_args,
            },
        }
    }
}
