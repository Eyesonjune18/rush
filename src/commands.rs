#![allow(dead_code, unused_variables)]

use crate::path::Path;
use crate::shell::Shell;
use crate::builtins;

// Represents a command that can be run by the prompt
pub struct Command {
    true_name: String,
    aliases: Vec<String>,
    runnable: Runnable,
}

impl Command {
    fn new(true_name: &str, aliases: Vec<&str>, runnable: Runnable) -> Self {
        let true_name = true_name.to_string();
        let aliases = aliases.iter().map(|a| a.to_string()).collect();

        Self {
            true_name,
            aliases,
            runnable,
        }
    }

    pub fn true_name(&self) -> &String {
        &self.true_name
    }
}

// Represents either an internal command or an external binary that can be invoked by a command
enum Runnable {
    Internal(Box<dyn Fn(&mut Context, Vec<&str>) -> StatusCode>),
    // ? Should this be a PathBuf, or should Path have a conversion method?
    External(Path),
}

impl Runnable {
    // Constructs an Internal Runnable from a function
    fn internal<F>(function: F) -> Self
    where
        F: Fn(&mut Context, Vec<&str>) -> StatusCode,
    {
        Self::Internal(Box::new(function))
    }

    // Constructs an External Runnable from a path
    fn external(path: Path) -> Self {
        Self::External(path)
    }

    fn run(&self, context: Context) -> StatusCode {
        todo!()
    }
}

// Wrapper struct around all of the data that could be needed for any command to run
// For instance, a command like 'truncate' may need to access the working directory, whereas
// a command like 'exit' may not need any data at all, but the data needs to be available in all cases
// TODO: Add an example for a command that needs different information
pub struct Context<'a> {
    pub shell: &'a mut Shell,
}

impl<'a> Context<'a> {
    pub fn new(shell: &'a mut Shell) -> Self {
        Self {
            shell
        }
    }
}

// Represents the status/exit code of a command
pub struct StatusCode {
    code: i32
}

// TODO: Move this or reorganize blocks here
impl StatusCode {
    pub fn new(code: i32) -> Self {
        Self {
            code
        }
    }

    pub fn success() -> Self {
        Self::new(0)
    }

    fn is_success(&self) -> bool {
        self.code == 0
    }
}

// Represents a collection of commands
// Allows for command resolution through aliases
pub struct CommandManager {
    commands: Vec<Command>,
}

impl Default for CommandManager {
    // Initializes the command manager with the default shell commands and aliases
    fn default() -> Self {
        let mut manager = Self::new();

        manager.add_command("exit", vec!["quit", "q"], Runnable::internal(builtins::exit));
        manager.add_command("test", vec!["t"], Runnable::internal(builtins::test));
        manager.add_command("truncate", vec!["trunc"], Runnable::internal(builtins::truncate));
        manager.add_command("untruncate", vec!["untrunc"], Runnable::internal(builtins::untruncate));
        manager.add_command("directory", vec!["dir", "pwd", "wd"], Runnable::internal(builtins::directory));

        manager
    }
}

impl CommandManager {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    // Adds a command to the manager
    fn add_command(&mut self, true_name: &str, aliases: Vec<&str>, runnable: Runnable) {
        self.commands.push(Command::new(true_name, aliases, runnable));
    }

    // Resolves a command name to a command
    // Returns None if the command is not found
    fn resolve(&self, command_name: &str) -> Option<&Command> {
        for command in &self.commands {
            if command.true_name == command_name {
                return Some(command)
            }

            for alias in &command.aliases {
                if alias == command_name {
                    return Some(command)
                }
            }
        }

        None
    }

    // Resolves and dispatches a command to the appropriate function or external binary
    // If the command does not exist, returns None
    pub fn dispatch(&self, command_name: &str, command_args: Vec<&str>, context: Context) -> Option<StatusCode> {


        None
    }
}