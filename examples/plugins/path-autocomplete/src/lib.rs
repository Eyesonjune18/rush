use rush_pdk::*;
use std::fs::DirEntry;

/// Toggle for checking if completions are executable files before suggesting them.
const CHECK_EXECUTE_PERMISSION: bool = true;

/// Check if the line buffer is still on its first argument (the command name). If it is, return the partial command name and whether there were quotes.
fn parse_line_buffer(mut line_buffer: &str) -> Option<(&str, bool)> {
    let is_in_quotes = line_buffer.starts_with('"');
    if is_in_quotes {
        line_buffer = &line_buffer[1..];
    }

    for character in line_buffer.chars() {
        match character {
            '"' if is_in_quotes => return None,
            ' ' if !is_in_quotes => return None,
            _ => {}
        }
    }

    Some((line_buffer, is_in_quotes))
}

/// Check if an entry in the PATH is a valid completion based on what the user has typed so far.
fn try_file_completion(first_arg: &str, entry: &DirEntry) -> Option<String> {
    let file_name = entry.file_name().into_string().ok()?;

    if !file_name.starts_with(first_arg)
        || (CHECK_EXECUTE_PERMISSION && !fs::is_executable(entry.path().to_str()?))
    {
        return None;
    }

    Some(file_name[first_arg.len()..].to_string())
}

/// Search the PATH for a completion that starts with `first_arg`.
fn create_completion(first_arg: &str) -> Option<String> {
    for path_dir in env::get("PATH").0?.split(':') {
        let Ok(contents) = std::fs::read_dir(path_dir) else {
            continue;
        };
        for entry in contents {
            let Ok(entry) = entry else {
                continue;
            };

            if let Some(completion) = try_file_completion(first_arg, &entry) {
                return Some(completion);
            }
        }
    }

    None
}

#[plugin_fn]
pub fn provide_autocomplete<'a>(line_buffer: String) -> FnResult<String> {
    let Some((first_arg, is_in_quotes)) = parse_line_buffer(&line_buffer) else {
        return Ok(String::new());
    };

    let mut completion = create_completion(first_arg);
    if is_in_quotes {
        if let Some(ref mut completion) = completion {
            completion.push('"');
        }
    }

    Ok(completion.unwrap_or(String::new()))
}
