pub mod exec_errors;
pub mod eval_errors;

use std::fmt::Display;

use eval_errors::EvalError;
use exec_errors::ExecError;

pub enum RushError {
    Eval(EvalError),
    Exec(ExecError),
}

impl From<EvalError> for RushError {
    fn from(e: EvalError) -> Self {
        RushError::Eval(e)
    }
}

impl From<ExecError> for RushError {
    fn from(e: ExecError) -> Self {
        RushError::Exec(e)
    }
}

impl Display for RushError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use RushError::*;
        match self {
            Eval(e) => write!(f, "{}", e),
            Exec(e) => write!(f, "{}", e),
        }
    }
}

impl RushError {
    // Shortcut for getting the name of the requested command if the error is a DispatchError of type UnknownCommand
    pub fn command_name_if_unknown(&self) -> Option<&str> {
        match self {
            RushError::Eval(e) => match &e.kind {
                eval_errors::EvalErrorCategory::Dispatch(e) => match e {
                    eval_errors::DispatchError::UnknownCommand(name) => Some(name),
                    _ => None,
                },
            },
            _ => None,
        }
    }
}

fn error_fmt<K: Display, C: Display>(f: &mut std::fmt::Formatter<'_>, kind: K, context: C) -> std::fmt::Result {
    writeln!(f, "[ERROR]:")?;
    for line in kind.to_string().lines() {
        writeln!(f, "    {}", line)?;
    }

    writeln!(f, "[CONTEXT]:")?;
    for line in context.to_string().lines() {
        writeln!(f, "    {}", line)?;
    }

    Ok(())
}
