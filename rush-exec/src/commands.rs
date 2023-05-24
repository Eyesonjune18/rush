use std::io::{BufRead, BufReader};
use std::process::{Command as Process, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use anyhow::Result;
use rush_state::console::Console;
use rush_state::path::Path;
use rush_state::shell::Shell;
use rush_state::showln;

use crate::errors::ExecutableError;

// Represents either a builtin (internal command) or an executable (external command)
// A Runnable may be executed by calling its .run() method
pub trait Runnable {
    fn run(&self, shell: &mut Shell, console: &mut Console, arguments: Vec<&str>) -> Result<()>;
}

// Wrapper type for Vec<String> that makes it easier to read code related to Builtins
pub struct Aliases {
    aliases: Vec<String>,
}

// * This implementation is here to make it easier to define aliases using string literals
impl From<Vec<&str>> for Aliases {
    fn from(aliases: Vec<&str>) -> Self {
        Self {
            aliases: aliases.iter().map(|a| a.to_string()).collect(),
        }
    }
}

impl Aliases {
    pub fn contains(&self, alias: &str) -> bool {
        self.aliases.contains(&alias.to_string())
    }
}

// Represents a builtin function, its name and its aliases
pub struct Builtin {
    pub true_name: String,
    pub aliases: Aliases,
    function: Box<dyn Fn(&mut Shell, &mut Console, Vec<&str>) -> Result<()>>,
}

impl Builtin {
    pub fn new<F: Fn(&mut Shell, &mut Console, Vec<&str>) -> Result<()> + 'static>(
        true_name: &str,
        aliases: Vec<&str>,
        function: F,
    ) -> Self {
        let true_name = true_name.to_string();
        let aliases = Aliases::from(aliases);
        let function = Box::new(function);

        Self {
            true_name,
            aliases,
            function,
        }
    }
}

impl Runnable for Builtin {
    fn run(&self, shell: &mut Shell, console: &mut Console, arguments: Vec<&str>) -> Result<()> {
        (self.function)(shell, console, arguments)
    }
}

// Represents an external binary/executable
pub struct Executable {
    path: Path,
}

impl Executable {
    // * This constructor is used in two cases:
    // * 1. When the user invokes an external binary using the run-executable builtin (explicit invocation)
    // * 2. When the user invokes an external binary that is in the PATH without using the run-executable builtin (implicit invocation)
    // * The Path wrapper type must be validated before construction, so it can be assumed that the path is valid
    pub fn new(path: Path) -> Self {
        Self { path }
    }
}

impl Runnable for Executable {
    // * Executables do not have access to the shell state, but the context argument is required by the Runnable trait
    // TODO: Remove as many .unwrap() calls as possible here
    fn run(&self, _shell: &mut Shell, console: &mut Console, arguments: Vec<&str>) -> Result<()> {
        // Create the Process, pass the provided arguments to it, and execute it
        let Ok(mut process) = Process::new(self.path.path())
            .args(arguments)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        else {
            return Err(ExecutableError::PathNoLongerExists(self.path.path().clone()).into())
        };

        // Create channels for communication between threads
        let (tx_stdout, rx_stdout) = mpsc::channel::<Result<String>>();
        let (tx_stderr, rx_stderr) = mpsc::channel::<Result<String>>();

        // Spawn a thread to read stdout
        let stdout_thread = {
            let stdout = process.stdout.take().unwrap();
            thread::spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines() {
                    // If the line is Ok, send it to the main thread
                    match line {
                        Ok(line) => {
                            // If sending the line fails, return an error
                            if let Err(e) = tx_stdout.send(Ok(line)) {
                                return Err(ExecutableError::FailedToParseStdout(e.to_string()));
                            }
                        }
                        // If reading the line fails, return an error
                        Err(e) => {
                            return Err(ExecutableError::FailedToParseStdout(e.to_string()));
                        }
                    }
                }
                Ok(())
            })
        };

        let stderr_thread = {
            let stderr = process.stderr.take().unwrap();
            thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines() {
                    match line {
                        Ok(line) => {
                            if let Err(e) = tx_stderr.send(Ok(line)) {
                                return Err(ExecutableError::FailedToParseStderr(e.to_string()));
                            }
                        }
                        Err(e) => {
                            return Err(ExecutableError::FailedToParseStderr(e.to_string()));
                        }
                    }
                }
                Ok(())
            })
        };

        let read_timeout = Duration::from_millis(100);
        let sleep_timeout = Duration::from_millis(10);

        let mut stdout_done = false;
        let mut stderr_done = false;
        let mut process_done = false;

        while !stdout_done || !stderr_done || !process_done {
            if let Ok(packet) = rx_stdout.recv_timeout(read_timeout) {
                // If the packet is Ok, unpack it and print it
                if let Ok(line) = packet {
                    showln!(console, "{}", &line);
                // If the packet is Err, propagate err up the stack
                } else {
                    packet?;
                }
            } else {
                stdout_done = true;
            }
            if let Ok(packet) = rx_stderr.recv_timeout(read_timeout) {
                if let Ok(line) = packet {
                    showln!(console, "{}", &line);
                } else {
                    packet?;
                }
            } else {
                stderr_done = true;
            }

            if !process_done {
                match process.try_wait() {
                    Ok(Some(_)) => {
                        process_done = true;
                        // Set these to false so we do at least one more check on both - since the
                        // program may terminate and not have had anything printed recently.
                        stdout_done = false;
                        stderr_done = false;
                    }
                    Ok(None) => {
                        // Child process is still running
                        // Add a small sleep to prevent high CPU usage in the loop
                        thread::sleep(sleep_timeout);
                    }
                    Err(e) => {
                        eprintln!("Error while waiting for child process: {}", e);
                        break;
                    }
                }
            }
        }

        // Wait for the threads to finish, if err, push it up the stack
        stdout_thread.join().unwrap()?;
        stderr_thread.join().unwrap()?;

        let status = process.wait().expect("Failed to wait on child process");

        match status.success() {
            true => Ok(()),
            false => {
                // * 126 is a special exit code that means that the command was found but could not be executed
                // * as per https://tldp.org/LDP/abs/html/exitcodes.html
                // * It can be assumed that the command was found here because the External path must have been validated already
                // * Otherwise it could be a 127 for "command not found"
                Err(ExecutableError::FailedToExecute(status.code().unwrap_or(126) as isize).into())
            }
        }
    }
}
