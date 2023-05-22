use anyhow::Result;

use rush_eval::dispatcher::Dispatcher;
use rush_state::console::{restore_terminal, Console};
use rush_state::shell::Shell;
use rush_state::showln;
use rush_error::RushError;
use rush_error::eval_errors::{EvalError, EvalErrorCategory, DispatchError};
use rush_error::exec_errors::ExecError;

fn main() -> Result<()> {
    // The Shell type stores all of the state for the shell, including its configuration,
    // its environment, and other miscellaneous data like command history
    let mut shell = Shell::new()?;
    // The Console type is responsible for reading and writing to the terminal (TUI),
    // and providing an interface for any commands that need to produce output and/or take input
    let mut console = Console::new()?;
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        default_panic(info);
    }));
    // The Dispatcher type is responsible for resolving command names to actual function calls,
    // or executables if needed, and then invoking them with the given arguments
    let dispatcher = Dispatcher::default();

    console.enter()?;

    loop {
        let line = console.read_line(&shell)?;
        let status = dispatcher.eval(&mut shell, &mut console, &line);
        handle_error(status, &mut shell, &mut console);

        shell.history_add(line);
    }
}

// Prints an appropriate error message for the given error, if applicable
fn handle_error(error: Result<(), RushError>, shell: &mut Shell, console: &mut Console) {
    if let Err(e) = error {
        if let Some(command_name) = e.command_name_if_unknown() {
            showln!(console, "Unknown command: {}", command_name);
        }

        if shell.config().show_errors {
            showln!(console, "{}", e);
        }
    } else {
        shell.set_success(true);
    }
}
