/*
A quick write-up on Rush builtins:
Builtins are commands that are included with the shell. They are not able to be removed or modified without recompiling the shell.
Normally, a child process, such as a shell command, does not have direct access to the parent process's environment variables and other state.
However, the builtins are an exception to this rule. They are able to access the data because they are trusted to safely modify it.
Users are free to create their own builtins if they wish to modify the source code, but it comes with an inherent risk.

An 'External' will only have access to its arguments and environment variables, but not the shell's state, mostly for security reasons.
 */

use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use fs_err::{self};
use rush_state::console::Console;
use rush_state::path::Path;
use rush_state::shell::Shell;
use rush_state::showln;

use crate::builtin_arguments::ListDirectoryArguments;
use crate::commands::{Executable, Runnable};
use crate::errors::BuiltinError;
use crate::errors::BuiltinError::{
    FailedReadingDir, FailedReadingFileName, FailedReadingFileType, FailedReadingPath,
};

pub fn test(_shell: &mut Shell, console: &mut Console, args: Vec<&str>) -> Result<()> {
    check_args(&args, 0, "test", console)?;
    showln!(console, "Test command!");
    Ok(())
}

pub fn exit(_shell: &mut Shell, console: &mut Console, args: Vec<&str>) -> Result<()> {
    check_args(&args, 0, "exit", console)?;
    console.exit(0);
    Ok(())
}

pub fn working_directory(shell: &mut Shell, console: &mut Console, args: Vec<&str>) -> Result<()> {
    check_args(&args, 0, "working-directory", console)?;
    showln!(console, "{}", shell.env().CWD());
    Ok(())
}

pub fn change_directory(shell: &mut Shell, console: &mut Console, args: Vec<&str>) -> Result<()> {
    check_args(&args, 1, "change-directory <path>", console)?;
    let history_limit = shell.config_mut().history_limit;
    shell
        .env_mut()
        .set_CWD(args[0], history_limit)
        .map_err(|_| {
            showln!(console, "Invalid path: '{}'", args[0]);
            BuiltinError::FailedToRun.into()
        })
}

pub fn list_directory(shell: &mut Shell, console: &mut Console, args: Vec<&str>) -> Result<()> {
    let arguments = ListDirectoryArguments::parse_from(&args);
    let show_hidden = arguments.all;
    let path_to_read = match arguments.path {
        Some(path) => PathBuf::from(path),
        None => shell.env().CWD().path().to_path_buf(),
    };

    let read_dir_result = match fs_err::read_dir(&path_to_read) {
        Ok(v) => v,
        Err(_) => return Err(FailedReadingPath(path_to_read).into()),
    };

    let mut directories = Vec::new();
    let mut files = Vec::new();

    for dir_entry in read_dir_result {
        let fs_object = match dir_entry {
            Ok(v) => v,
            Err(_) => return Err(FailedReadingDir(path_to_read).into()),
        };

        let fs_object_name = match fs_object.file_name().to_str() {
            Some(v) => String::from(v),
            None => return Err(FailedReadingFileName(path_to_read).into()),
        };

        let fs_object_type = match fs_object.file_type() {
            Ok(v) => v,
            Err(_) => return Err(FailedReadingFileType(path_to_read).into()),
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

// TODO: Find a better name for this
pub fn go_back(shell: &mut Shell, console: &mut Console, args: Vec<&str>) -> Result<()> {
    check_args(&args, 0, "go-back", console)?;
    shell.env_mut().go_back().map_err(|_| {
        showln!(console, "Previous directory does not exist or is invalid");
        BuiltinError::FailedToRun.into()
    })
}

pub fn go_forward(shell: &mut Shell, console: &mut Console, args: Vec<&str>) -> Result<()> {
    check_args(&args, 0, "go-forward", console)?;
    shell.env_mut().go_forward().map_err(|_| {
        showln!(console, "Next directory does not exist or is invalid");
        BuiltinError::FailedToRun.into()
    })
}

pub fn clear_terminal(_shell: &mut Shell, console: &mut Console, args: Vec<&str>) -> Result<()> {
    check_args(&args, 0, "clear-terminal", console)?;
    console.clear_output()
}

// TODO: Add prompt to confirm file overwrite
pub fn make_file(_shell: &mut Shell, console: &mut Console, args: Vec<&str>) -> Result<()> {
    if args.len() == 1 {
        fs_err::File::create(args[0]).map_err(|_| {
            showln!(console, "Failed to create file: '{}'", args[0]);
            BuiltinError::FailedToRun
        })?;
        Ok(())
    } else {
        showln!(console, "Usage: make-file <path>");
        Err(BuiltinError::InvalidArgumentCount(args.len()).into())
    }
}

pub fn make_directory(_shell: &mut Shell, console: &mut Console, args: Vec<&str>) -> Result<()> {
    if args.len() == 1 {
        fs_err::create_dir(args[0]).map_err(|_| {
            showln!(console, "Failed to create directory: '{}'", args[0]);
            BuiltinError::FailedToRun
        })?;
        Ok(())
    } else {
        showln!(console, "Usage: make-directory <path>");
        Err(BuiltinError::InvalidArgumentCount(args.len()).into())
    }
}

pub fn delete_file(_shell: &mut Shell, console: &mut Console, args: Vec<&str>) -> Result<()> {
    if args.len() == 1 {
        fs_err::remove_file(args[0]).map_err(|_| {
            showln!(console, "Failed to delete file: '{}'", args[0]);
            BuiltinError::FailedToRun
        })?;
        Ok(())
    } else {
        showln!(console, "Usage: delete-file <path>");
        Err(BuiltinError::InvalidArgumentCount(args.len()).into())
    }
}

pub fn read_file(_shell: &mut Shell, console: &mut Console, args: Vec<&str>) -> Result<()> {
    check_args(&args, 1, "read-file <path>", console)?;
    let file_name = args[0].to_string();
    let file = fs_err::File::open(&file_name).map_err(|_| {
        showln!(console, "Failed to open file: '{}'", file_name);
        BuiltinError::FailedToRun
    })?;

    let reader = BufReader::new(file);
    for line in reader.lines() {
        let line = line.expect("Failed to read line");
        showln!(console, "{}", &line);
    }

    Ok(())
}

pub fn run_executable(shell: &mut Shell, console: &mut Console, mut args: Vec<&str>) -> Result<()> {
    let executable_name = args[0].to_string();
    let executable_path = Path::from_str(&executable_name, shell.env().HOME()).map_err(|_| {
        showln!(
            console,
            "Failed to resolve executable path: '{}'",
            executable_name
        );
        BuiltinError::FailedToRun
    })?;

    // * Executable name is removed before running the executable because the std::process::Command
    // * process builder automatically adds the executable name as the first argument
    args.remove(0);
    Executable::new(executable_path).run(shell, console, args)
}

pub fn configure(shell: &mut Shell, console: &mut Console, args: Vec<&str>) -> Result<()> {
    check_args(&args, 2, "configure <key> <value>", console)?;
    let key = args[0];
    let value = args[1];

    match key {
        "truncation" => {
            if value == "false" {
                shell.config_mut().truncation_factor = None;
                return Ok(());
            }

            shell.config_mut().truncation_factor = Some(value.parse::<usize>().map_err(|_| {
                showln!(console, "Invalid truncation length: '{}'", value);
                BuiltinError::InvalidValue(value.to_string())
            })?)
        }
        "history-limit" => {
            if value == "false" {
                shell.config_mut().history_limit = None;
                return Ok(());
            }

            shell.config_mut().history_limit = Some(value.parse::<usize>().map_err(|_| {
                showln!(console, "Invalid history limit: '{}'", value);
                BuiltinError::InvalidValue(value.to_string())
            })?)
        }
        "show-errors" => {
            shell.config_mut().show_errors = value.parse::<bool>().map_err(|_| {
                showln!(console, "Invalid value for show-errors: '{}'", value);
                BuiltinError::InvalidValue(value.to_string())
            })?
        }
        _ => {
            showln!(console, "Invalid configuration key: '{}'", key);
            return Err(BuiltinError::InvalidArgument(key.to_string()).into());
        }
    }

    Ok(())
}

pub fn environment_variable(
    shell: &mut Shell,
    console: &mut Console,
    args: Vec<&str>,
) -> Result<()> {
    check_args(&args, 1, "environment-variable <var>", console)?;
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
            return Err(BuiltinError::InvalidArgument(args[0].to_string()).into());
        }
    }

    Ok(())
}

pub fn edit_path(shell: &mut Shell, console: &mut Console, args: Vec<&str>) -> Result<()> {
    check_args(&args, 2, "edit-path <append | prepend> <path>", console)?;
    let action = args[0];
    let path = Path::from_str(args[1], shell.env().HOME()).map_err(|_| {
        showln!(console, "Invalid directory: '{}'", args[1]);
        BuiltinError::FailedToRun
    })?;

    match action {
        "append" => shell.env_mut().PATH_mut().push_front(path),
        "prepend" => shell.env_mut().PATH_mut().push_back(path),
        _ => {
            showln!(console, "Invalid action: '{}'", action);
            return Err(BuiltinError::InvalidArgument(args[0].to_string()).into());
        }
    }

    Ok(())
}

// Convenience function for exiting a builtin on invalid argument count
fn check_args(
    args: &Vec<&str>,
    expected_args: usize,
    usage: &str,
    console: &mut Console,
) -> Result<()> {
    if args.len() == expected_args {
        Ok(())
    } else {
        showln!(console, "Usage: {}", usage);
        Err(BuiltinError::InvalidArgumentCount(args.len()).into())
    }
}
