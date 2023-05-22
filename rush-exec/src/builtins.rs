/*
A quick write-up on Rush builtins:
Builtins are commands that are included with the shell. They are not able to be removed or modified without recompiling the shell.
Normally, a child process, such as a shell command, does not have direct access to the parent process's environment variables and other state.
However, the builtins are an exception to this rule. They are able to access the data because they are trusted to safely modify it.
Users are free to create their own builtins if they wish to modify the source code, but it comes with an inherent risk.

An Executable will only have access to its arguments and environment variables, but not the shell's state, mostly for security reasons.
 */

use clap::Parser;
use fs_err::{self};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use crate::builtin_arguments::ListDirectoryArguments;
use rush_state::console::Console;
use rush_state::path::Path;
use rush_state::shell::Shell;
use rush_state::showln;
use rush_error::RushError;
use rush_error::exec_errors::{ExecError, CommandType, ArgumentError, FilesystemError, RuntimeError};

use crate::commands::{Executable, Runnable};

// Gets the name of the function that called this macro
macro_rules! fn_name {
    () => {{
        fn f() {}
        // Get the name of the function
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of(f);
        // Remove the path to the function
        name[..name.len() - 3].split("::").last().unwrap()
    }}
}

// Translates the name of the builtin function to the user-facing name of the builtin
macro_rules! builtin_name {
    () => {
        // Convert '_' to '-' for the user-facing name
        fn_name!().replace("_", "-")
    }
}

// Convenience macro for creating and returning an ExecError
// ? Should this just expand to the error value so it can be returned explicitly?
macro_rules! exec_error {
    ($kind:expr, $args:expr) => {
        return Err(RushError::from(ExecError::new($kind, CommandType::Builtin, &builtin_name!(), $args.clone())))
    }
}

// Convenience macro for exiting a builtin on invalid argument count
// $ This will probably be replaced by clap
macro_rules! check_args {
    ($console:expr, $args:expr, $expected:literal, $usage:literal) => {
        if $args.len() != $expected {
            let name = builtin_name!();
            showln!($console, "Usage: {} {}", name, $usage);
            exec_error!(ArgumentError::InvalidArgumentCount($expected, $args.len()), $args)
        }
    };
    ($console:expr, $args:expr, $expected:literal) => {
        check_args!($console, $args, $expected, "")
    };
}

pub fn test(
    _shell: &mut Shell,
    console: &mut Console,
    args: Vec<String>,
) -> Result<(), RushError> {
    check_args!(console, args, 0);
    showln!(console, "Test command!");
    Ok(())
}

pub fn exit(
    _shell: &mut Shell,
    console: &mut Console,
    args: Vec<String>,
) -> Result<(), RushError> {
    check_args!(console, args, 0);
    console.exit(0);
    Ok(())
}

pub fn working_directory(
    shell: &mut Shell,
    console: &mut Console,
    args: Vec<String>,
) -> Result<(), RushError> {
    check_args!(console, args, 0);
    showln!(console, "{}", shell.env().CWD());
    Ok(())
}

pub fn change_directory(
    shell: &mut Shell,
    console: &mut Console,
    args: Vec<String>,
) -> Result<(), RushError> {
    check_args!(console, args, 1, "<path>");
    let history_limit = shell.config_mut().history_limit;
    if let Err(_) = shell.env_mut().set_CWD(&args[0], history_limit) {
        showln!(console, "Invalid path: '{}'", args[0]);
        exec_error!(RuntimeError::FailedToRun, args)
    }

    Ok(())
}

pub fn list_directory(
    shell: &mut Shell,
    console: &mut Console,
    args: Vec<String>,
) -> Result<(), RushError> {
    let arguments = ListDirectoryArguments::parse_from(&args);
    let show_hidden = arguments.all;
    let path_to_read = match arguments.path {
        Some(path) => PathBuf::from(path),
        None => shell.env().CWD().path().to_owned(),
    };

    let read_dir_result = match fs_err::read_dir(&path_to_read) {
        Ok(v) => v,
        Err(_) => exec_error!(FilesystemError::FailedToReadDirectory(path_to_read), args),
    };

    let mut directories = Vec::new();
    let mut files = Vec::new();

    for dir_entry in read_dir_result {
        let fs_object = match dir_entry {
            Ok(v) => v,
            Err(_) => exec_error!(FilesystemError::FailedToReadDirectory(path_to_read), args),
        };

        let fs_object_name = match fs_object.file_name().to_str() {
            Some(v) => String::from(v),
            None => exec_error!(FilesystemError::FailedToReadDirectory(path_to_read), args),
        };

        let fs_object_type = match fs_object.file_type() {
            Ok(v) => v,
            Err(_) => exec_error!(FilesystemError::FailedToReadDirectory(path_to_read), args),
        };

        if fs_object_name.starts_with('.') && !show_hidden {
            continue;
        }

        if fs_object_type.is_dir() {
            directories.push(format!("{}/", fs_object_name));
        } else {
            files.push(fs_object_name);
        };
    }

    directories.sort();
    files.sort();

    for directory in directories {
        showln!(console, "{}", &directory);
    }

    for file in files {
        showln!(console, "{}", &file);
    }

    Ok(())
}

pub fn previous_directory(
    shell: &mut Shell,
    console: &mut Console,
    args: Vec<String>,
) -> Result<(), RushError> {
    check_args!(console, args, 0);
    if shell.env_mut().go_back().is_err() {
        showln!(console, "Previous directory does not exist or is invalid");
        exec_error!(RuntimeError::FailedToRun, args)
    }

    Ok(())
}

pub fn next_directory(
    shell: &mut Shell,
    console: &mut Console,
    args: Vec<String>,
) -> Result<(), RushError> {
    check_args!(console, args, 0);
    if shell.env_mut().go_forward().is_err() {
        showln!(console, "Next directory does not exist or is invalid");
        exec_error!(RuntimeError::FailedToRun, args)
    }
    
    Ok(())
}

pub fn clear_terminal(
    _shell: &mut Shell,
    console: &mut Console,
    args: Vec<String>,
) -> Result<(), RushError> {
    check_args!(console, args, 0);
    // $ FIX
    console.clear_output();
    Ok(())
}

// TODO: Add prompt to confirm file overwrite
pub fn make_file(
    _shell: &mut Shell,
    console: &mut Console,
    args: Vec<String>,
) -> Result<(), RushError> {
    check_args!(console, args, 1, "<path>");
    // TODO: Map fs_err errors to FilesystemError
    if fs_err::File::create(&args[0]).is_err() {
        showln!(console, "Failed to create file: '{}'", args[0]);
        exec_error!(RuntimeError::FailedToRun, args)
    }

    Ok(())
}

pub fn make_directory(
    _shell: &mut Shell,
    console: &mut Console,
    args: Vec<String>,
) -> Result<(), RushError> {
    check_args!(console, args, 1, "<path>");
    if fs_err::create_dir(&args[0]).is_err() {
        showln!(console, "Failed to create directory: '{}'", args[0]);
        exec_error!(RuntimeError::FailedToRun, args)
    }

    Ok(())
}

pub fn delete_file(
    _shell: &mut Shell,
    console: &mut Console,
    args: Vec<String>,
) -> Result<(), RushError> {
    check_args!(console, args, 1, "<path>");
    if fs_err::remove_file(&args[0]).is_err() {
        showln!(console, "Failed to delete file: '{}'", args[0]);
        exec_error!(RuntimeError::FailedToRun, args)
    }

    Ok(())
}

pub fn read_file(
    _shell: &mut Shell,
    console: &mut Console,
    args: Vec<String>,
) -> Result<(), RushError> {
    check_args!(console, args, 1);
    let file_name = args[0].to_string();
    let Ok(file) = fs_err::File::open(&file_name) else {
        showln!(console, "Failed to open file: '{}'", file_name);
        exec_error!(RuntimeError::FailedToRun, args)
    };

    let reader = BufReader::new(file);
    for line in reader.lines() {
        let line = line.expect("Failed to read line");
        showln!(console, "{}", &line);
    }

    Ok(())
}

pub fn run_executable(
    shell: &mut Shell,
    console: &mut Console,
    mut args: Vec<String>,
) -> Result<(), RushError> {
    let executable_name = args[0].to_string();
    let Ok(executable_path) = Path::from_str(&executable_name, shell.env().HOME()) else {
        showln!(console, "Failed to resolve executable path: '{}'", executable_name);
        exec_error!(RuntimeError::FailedToRun, args)
    };

    // * Executable name is removed before running the executable because the std::process::Command
    // * process builder automatically adds the executable name as the first argument
    args.remove(0);
    Executable::new(executable_path).run(shell, console, args)
}

pub fn configure(
    shell: &mut Shell,
    console: &mut Console,
    args: Vec<String>,
) -> Result<(), RushError> {
    check_args!(console, args, 2);
    let key = args[0].clone();
    let value = args[1].clone();

    match key.as_str() {
        "truncation" => {
            if value == "false" {
                shell.config_mut().truncation_factor = None;
                return Ok(());
            }

            if let Ok(value) = value.parse::<usize>() {
                shell.config_mut().truncation_factor = Some(value);
                return Ok(());
            } else {
                showln!(console, "Invalid truncation length: '{}'", value);
                exec_error!(ArgumentError::InvalidValue(value), args);
            }
        }
        "history-limit" => {
            if value == "false" {
                shell.config_mut().history_limit = None;
                return Ok(());
            }

            if let Ok(limit) = value.parse::<usize>() {
                shell.config_mut().history_limit = Some(limit);
                return Ok(());
            } else {
                showln!(console, "Invalid history limit: '{}'", value);
                exec_error!(ArgumentError::InvalidValue(value), args);
            }
        }
        "show-errors" => {
            if let Ok(value) = value.parse::<bool>() {
                shell.config_mut().show_errors = value;
                return Ok(());
            } else {
                showln!(console, "Invalid value for show-errors: '{}'", value);
                exec_error!(ArgumentError::InvalidValue(value), args)
            }
        }
        _ => {
            showln!(console, "Invalid configuration key: '{}'", key);
            exec_error!(ArgumentError::InvalidArgument(key), args);
        }
    }
}

pub fn environment_variable(
    shell: &mut Shell,
    console: &mut Console,
    args: Vec<String>,
) -> Result<(), RushError> {
    check_args!(console, args, 1);
    match args[0].to_uppercase().as_str() {
        "PATH" => {
            for (i, path) in shell.env().PATH().iter().enumerate() {
                showln!(console, "[{i}]: {path}");
            }
        }
        "USER" => showln!(console, "{}", shell.env().USER()),
        "HOME" => showln!(console, "{}", shell.env().HOME().display()),
        "CWD" | "WORKING-DIRECTORY" => showln!(console, "{}", shell.env().CWD()),
        _ => {
            showln!(console, "Invalid environment variable: '{}'", args[0]);
            exec_error!(ArgumentError::InvalidArgument(args[0].clone()), args)
        }
    }

    Ok(())
}

pub fn edit_path(
    shell: &mut Shell,
    console: &mut Console,
    args: Vec<String>,
) -> Result<(), RushError> {
    check_args!(console, args, 2);
    let action = args[0].clone();
    let Ok(path) = Path::from_str(&args[1], shell.env().HOME()) else {
        showln!(console, "Invalid directory: '{}'", &args[1]);
        exec_error!(ArgumentError::InvalidArgument(args[1].clone()), args)
    };

    match action.as_str() {
        "append" => shell.env_mut().PATH_mut().push_front(path),
        "prepend" => shell.env_mut().PATH_mut().push_back(path),
        _ => {
            showln!(console, "Invalid action: '{}'", action);
            exec_error!(ArgumentError::InvalidArgument(action), args)
        }
    }

    Ok(())
}
