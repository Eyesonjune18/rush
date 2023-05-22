use std::fmt::Display;

use crate::error_fmt;

pub trait EvalErrorKind: Into<EvalErrorCategory> {}

#[derive(Debug)]
pub struct EvalError {
    // ? Should this be a getter method required by RushError?
    pub kind: EvalErrorCategory,
    context: EvalErrorContext,
}

impl Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_fmt(f, &self.kind, &self.context)
    }
}

#[derive(Debug)]
struct EvalErrorContext {
    command_name: String,
    command_args: Vec<String>,
}

impl Display for EvalErrorContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name_str = format!("[COMMAND]: {}", self.command_name);
        let args_str = format!("[ARGUMENTS]: {}", self.command_args.join(" "));
        write!(f, "{}\n{}", name_str, args_str)
    }
}

#[derive(Debug)]
pub enum EvalErrorCategory {
    Dispatch(DispatchError),
}

impl Display for EvalErrorCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use EvalErrorCategory::*;
        let category_str = format!("[CATEGORY]: {}", match self {
            Dispatch(_) => "Dispatch",
        });

        let message_str = format!("[MESSAGE]: {}", match self {
            Dispatch(e) => format!("{}", e),
        });

        write!(f, "{}\n{}", category_str, message_str)
    }
}

impl From<DispatchError> for EvalErrorCategory {
    fn from(e: DispatchError) -> Self {
        EvalErrorCategory::Dispatch(e)
    }
}

#[derive(Debug)]
pub enum DispatchError {
    UnknownCommand(String),
    NotAnExecutable(u32),
    FailedToReadMetadata(String),
}

impl EvalErrorKind for DispatchError {}

impl Display for DispatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use DispatchError::*;
        write!(f, "{}", match self {
            UnknownCommand(name) => format!("Command name '{}' not found as a builtin or an executable in PATH", name),
            NotAnExecutable(perms) => format!("File lacks executable permissions. Current permissions: {}", perms),
            FailedToReadMetadata(name) => format!("Command metadata for '{}' could not be read", name),
        })
    }
}

impl EvalError {
    pub fn new(kind: impl EvalErrorKind, command_name: &str, command_args: Vec<String>) -> Self {
        EvalError {
            kind: kind.into(),
            context: EvalErrorContext {
                command_name: command_name.to_owned(),
                command_args,
            },
        }
    }
}
