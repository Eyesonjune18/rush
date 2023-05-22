use std::os::unix::prelude::PermissionsExt;

extern crate clap;

use rush_exec::builtins;
use rush_exec::commands::{Builtin, Executable, Runnable};
use rush_state::console::Console;
use rush_state::path::Path;
use rush_state::shell::Shell;
use rush_error::RushError;
use rush_error::eval_errors::{EvalError, DispatchError};

use crate::parser;

// Convenience macro for creating and returning an EvalError
// ? Should this just expand to the error value so it can be returned explicitly?
macro_rules! eval_error {
    ($kind:expr, $name:expr, $args:expr) => {
        return Err(RushError::from(EvalError::new($kind, $name, $args.clone())))
    }
}

// Represents a collection of builtin commands
// Allows for command resolution and execution through aliases
pub struct Dispatcher {
    commands: Vec<Builtin>,
}

impl Default for Dispatcher {
    // Initializes the Dispatcher with the default shell commands and aliases
    #[rustfmt::skip]
    fn default() -> Self {
        let mut dispatcher = Self::new();

        dispatcher.add_builtin("test", ["t"], builtins::test);
        dispatcher.add_builtin("exit", ["quit", "q"], builtins::exit);
        dispatcher.add_builtin("working-directory", ["pwd", "wd"], builtins::working_directory);
        dispatcher.add_builtin("change-directory", ["cd"], builtins::change_directory);
        dispatcher.add_builtin("list-directory", ["directory", "list", "ls", "dir"], builtins::list_directory);
        dispatcher.add_builtin("previous-directory", ["back", "b", "prev", "pd"], builtins::previous_directory);
        dispatcher.add_builtin("next-directory", ["forward", "f", "next", "nd"], builtins::next_directory);
        dispatcher.add_builtin("clear-terminal", ["clear", "cls"], builtins::clear_terminal);
        dispatcher.add_builtin("make-file", ["create", "touch", "new", "mf"], builtins::make_file);
        dispatcher.add_builtin("make-directory", ["mkdir", "md"], builtins::make_directory);
        dispatcher.add_builtin("delete-file", ["delete", "remove", "rm", "del", "df"], builtins::delete_file);
        dispatcher.add_builtin("read-file", ["read", "cat", "rf"], builtins::read_file);
        dispatcher.add_builtin("run-executable", ["run", "exec", "re"], builtins::run_executable);
        dispatcher.add_builtin("configure", ["config", "conf"], builtins::configure);
        dispatcher.add_builtin("environment-variable", ["environment", "env", "ev"], builtins::environment_variable);
        dispatcher.add_builtin("edit-path", ["path", "ep"], builtins::edit_path);

        dispatcher
    }
}

impl Dispatcher {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    // Adds a builtin to the Dispatcher
    fn add_builtin<F: Fn(&mut Shell, &mut Console, Vec<String>) -> Result<(), RushError> + 'static, const N: usize>(
        &mut self,
        true_name: &str,
        aliases: [&str; N],
        function: F,
    ) {
        let aliases = aliases.into_iter().map(str::to_owned).collect();
        self.commands
            .push(Builtin::new(true_name, aliases, function))
    }

    // Finds a builtin command by name or alias
    // Returns None if the builtin does not exist
    fn resolve(&self, command_name: &str) -> Option<&Builtin> {
        for command in &self.commands {
            if command.true_name == command_name {
                return Some(command);
            }

            if command.aliases.contains(command_name) {
                return Some(command);
            }
        }

        None
    }

    // Evaluates and executes a command from a string
    pub fn eval(&self, shell: &mut Shell, console: &mut Console, line: &String) -> Result<(), RushError> {
        let commands = parser::parse(line);
        let mut results: Vec<Result<(), RushError>> = Vec::new();

        for (command_name, command_args) in commands {
            // Dispatch the command to the Dispatcher
            let result = self.dispatch(shell, console, &command_name, command_args.into());
            results.push(result);
        }

        for result in results {
            if result.is_err() {
                return Err(result.err().unwrap());
            }
        }

        Ok(())
    }

    // Resolves and dispatches a command to the appropriate function or external binary
    // If the command does not exist, returns None
    fn dispatch(
        &self,
        shell: &mut Shell,
        console: &mut Console,
        command_name: &str,
        command_args: Vec<String>,
    ) -> Result<(), RushError> {
        // If the command resides in the Dispatcher (generally means it is a builtin) run it
        if let Some(command) = self.resolve(command_name) {
            command.run(shell, console, command_args)
        } else {
            // If the command is not in the Dispatcher, try to run it as an executable from the PATH
            let path = Path::from_path_var(command_name, shell.env().PATH());
            if let Ok(path) = path {
                // Check if the file is executable (has the executable bit set)
                if let Ok(metadata) = fs_err::metadata(path.path()) {
                    let permission_code = metadata.permissions().mode();
                    // 0o111 is the octal representation of 73, which is the executable bit
                    if permission_code & 0o111 == 0 {
                        eval_error!(DispatchError::NotAnExecutable(permission_code), command_name, command_args)
                    } else {
                        Executable::new(path).run(shell, console, command_args)
                    }
                } else {
                    // If the file cannot be read, return an error
                    eval_error!(DispatchError::FailedToReadMetadata(path.to_string()), command_name, command_args)
                }
            } else {
                eval_error!(DispatchError::UnknownCommand(command_name.to_string()), command_name, command_args)
            }
        }
    }
}
