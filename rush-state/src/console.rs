use std::fmt::Debug;
use std::io::{stdout, Stdout};
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use bitflags::bitflags;
use crossterm::cursor;
use crossterm::event::{self, DisableMouseCapture, Event, KeyCode, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Span, Line, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::{Frame, Terminal};

use crate::shell::Shell;

// Macros for printing to the TUI console
#[macro_export]
macro_rules! show {
    ($console:expr, $($arg:tt)*) => {
        $console.print(&::std::format!($($arg)*))
    };
}

#[macro_export]
macro_rules! showln {
    ($console:expr $(,)?) => {
        $console.println("")
    };

    ($console:expr, $($arg:tt)*) => {
        $console.println(&::std::format!($($arg)*))
    };
}

// Represents an action that the handler instructs the REPL (Console.read_line()) to perform
// Allows for some actions to be performed in the handler and some to be performed in the REPL
enum ReplAction {
    // Instruction to return the line buffer to the shell and perform any necessary cleanup
    Return,
    // Instruction to exit the shell
    Exit,
    // Instruction to do nothing except update the TUI
    RedrawFrame,
    // Instruction to do nothing
    Ignore,
}

// Switch between "BACKSPACE" and "DELETE" keypresses for ConsoleData.remove_char()
#[derive(PartialEq)]
enum RemoveMode {
    Backspace,
    Delete,
}

// Represents a variety of switchable modes for clearing the TUI console/frame
// * Not to be confused with crossterm::terminal::ClearType
bitflags! {
    struct ClearMode: u8 {
        const OUTPUT = 0b00000001;
        const LINE = 0b00000010;
    }
}

// Represents either a "history up" or "history down" keypress (arrow keys)
#[derive(PartialEq)]
enum HistoryDirection {
    Up,
    Down,
}

// Represents the TUI console
pub struct Console<'a> {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    data: ConsoleData<'a>,
}

// Represents all data stored in the TUI console, excluding the Terminal
// * This is done because most methods do not need to access the Console.terminal and it can cause issues with borrowing
struct ConsoleData<'a> {
    // ? Should this be an Option<Line>?
    prompt: Line<'a>,
    // ? What is the actual name of this?
    prompt_tick: Span<'a>,
    // An index to the Span of the tick next to the most recently executed command
    // Used to recolor the tick based on the success of the command
    // * If the tick index is None, then no command has been executed yet
    success_tick_index: Option<usize>,
    // The line buffer for the prompt panel
    line_buffer: String,
    // The framebuffer for the output panel
    output_buffer: Text<'a>,
    // The framebuffer for the debug panel
    debug_buffer: Text<'a>,
    // The index of the cursor in the line buffer
    cursor_index: usize,
    // If the line buffer can autocomplete to a command from the history, this stores the characters that will be added if the user presses TAB
    autocomplete_buffer: Option<String>,
    // If the user is scrolling through the command history, this stores the original line buffer and cursor position so they can be restored if needed
    history_buffer: Option<(String, usize)>,
    // The history index stored when the user is scrolling through the command history
    history_index: Option<usize>,
    // The number of lines that have been scrolled down in the output panel
    scroll: usize,
    // Whether or not to show the debug panel
    debug_mode: bool,
}

pub static RAW_MODE: AtomicBool = AtomicBool::new(false);
pub fn restore_terminal() {
    if !RAW_MODE.load(Ordering::Acquire) {
        return;
    }
    disable_raw_mode().unwrap();
    execute!(
        stdout(),
        LeaveAlternateScreen,
        cursor::MoveTo(0, 0),
        cursor::Show,
        Clear(ClearType::All)
    )
    .unwrap();
    RAW_MODE.store(false, Ordering::Release);
}

impl<'a> Console<'a> {
    pub fn new() -> Result<Self> {
        let backend = CrosstermBackend::new(stdout());
        let terminal = Terminal::new(backend)?;

        Ok(Self {
            terminal,
            data: ConsoleData::new(),
        })
    }

    // Enters the TUI console
    pub fn enter(&mut self) -> Result<()> {
        enable_raw_mode()?;
        // ? Is mouse capture enabled by default?
        execute!(
            self.terminal.backend_mut(),
            EnterAlternateScreen,
            DisableMouseCapture
        )?;
        self.terminal.show_cursor()?;

        RAW_MODE.store(true, Ordering::Release);
        self.clear(ClearMode::LINE)
    }

    // Closes the TUI console and exits the program
    // * Error handling here is unnecessary because the program is exiting
    // TODO: This assumption may need to be reevaluated in the future
    pub fn exit(&mut self, code: i32) {
        restore_terminal();
        // TODO: Exiting ignores drops, which may be problematic
        std::process::exit(code);
    }

    // Reads a line of input from the user
    // Handles all TUI interaction between the user and the prompt
    pub fn read_line(&mut self, shell: &Shell) -> Result<String> {
        self.data.update_output_tick(shell);
        self.data.update_prompt(shell);
        self.data.update_debug(shell);
        self.draw_frame(true)?;

        loop {
            let event = event::read()?;
            let action = self.handle_event(event, shell)?;

            match action {
                ReplAction::Return => {
                    // Make sure that there is an extra line of space between the last line of output and the command output
                    self.data.enforce_spacing();

                    // Save the line buffer for returning and reset it to make way for the next Console.read_line() call
                    let line = self.data.line_buffer.clone();
                    self.data.reset_line_buffer();

                    // Clear the history buffer and index
                    self.data.history_buffer = None;
                    self.data.history_index = None;

                    // Clear the autocomplete buffer
                    self.data.autocomplete_buffer = None;

                    // Save the line buffer as part of the output buffer, along with a tick which will be colored grey at first
                    // while the command is executing, and then green or red depending on the eventual success or failure of the command
                    self.data.success_tick_index = Some(self.data.output_buffer.lines.len());
                    let mut input_line = Line::from(vec![
                        Span::styled(
                            "❯ ",
                            Style::default()
                                .fg(Color::DarkGray)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(line.clone(), Style::default().fg(Color::LightYellow)),
                    ]);

                    input_line.patch_style(Style::default().add_modifier(Modifier::ITALIC));
                    self.data.append_line_with_newline(input_line);

                    // Draw the frame with the new output
                    self.data.update_debug(shell);
                    self.draw_frame(true)?;

                    return Ok(line);
                }
                ReplAction::Exit => self.exit(0),
                ReplAction::RedrawFrame => {
                    self.data.update_autocomplete(shell);
                    self.data.update_debug(shell);
                    self.draw_frame(false)?;
                }
                ReplAction::Ignore => (),
            }
        }
    }

    // Handles a key event by queueing appropriate commands based on the given keypress
    fn handle_event(&mut self, event: Event, shell: &Shell) -> Result<ReplAction> {
        // TODO: Break up event handling into separate functions for different event categories
        match event {
            Event::Key(event) => {
                match (event.modifiers, event.code) {
                    (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => {
                        self.data.insert_char(c)
                    }
                    (KeyModifiers::NONE, KeyCode::Backspace) => {
                        self.data.remove_char(RemoveMode::Backspace)
                    }
                    (KeyModifiers::NONE, KeyCode::Delete) => {
                        self.data.remove_char(RemoveMode::Delete)
                    }
                    (KeyModifiers::NONE, KeyCode::Left) => self.data.move_cursor_left(),
                    (KeyModifiers::NONE, KeyCode::Right) => self.data.move_cursor_right(),
                    (KeyModifiers::ALT, KeyCode::Left) => self.data.seek_cursor_left(),
                    (KeyModifiers::ALT, KeyCode::Right) => self.data.seek_cursor_right(),
                    (KeyModifiers::NONE, KeyCode::Enter) if !self.data.line_buffer.is_empty() => {
                        return Ok(ReplAction::Return)
                    }
                    (KeyModifiers::SHIFT, KeyCode::Up) => self.data.scroll_up(),
                    (KeyModifiers::SHIFT, KeyCode::Down) => self.data.scroll_down(),
                    (KeyModifiers::NONE, KeyCode::Up) => {
                        self.data.scroll_history(HistoryDirection::Up, shell)?
                    }
                    (KeyModifiers::NONE, KeyCode::Down) => {
                        self.data.scroll_history(HistoryDirection::Down, shell)?
                    }
                    (KeyModifiers::NONE, KeyCode::Tab) => self.data.autocomplete_line(),
                    (KeyModifiers::CONTROL, KeyCode::Char('c')) => return Ok(ReplAction::Exit),
                    (KeyModifiers::CONTROL, KeyCode::Char('l')) => self.clear(ClearMode::OUTPUT)?,
                    (KeyModifiers::CONTROL, KeyCode::Char('u')) => self.clear(ClearMode::LINE)?,
                    // TODO: Make this a toggle method
                    (KeyModifiers::CONTROL, KeyCode::Char('d')) => {
                        self.data.debug_mode = !self.data.debug_mode
                    }
                    _ => return Ok(ReplAction::Ignore),
                }
            }
            // $ This seems like a crappy solution to prevent the Resize event from being ignored
            Event::Resize(_, _) => (),
            _ => return Ok(ReplAction::Ignore),
        }

        Ok(ReplAction::RedrawFrame)
    }

    // Updates the TUI frame
    // ? Should the autoscroll parameter use a custom type for readability?
    pub fn draw_frame(&mut self, autoscroll: bool) -> Result<()> {
        self.terminal
            .draw(|f| self.data.generate_frame(f, autoscroll))?;
        Ok(())
    }

    // Clears the screen and the line buffer and reprompts the user
    fn clear(&mut self, mode: ClearMode) -> Result<()> {
        // Clear the output panel
        if mode.contains(ClearMode::OUTPUT) {
            self.data.output_buffer = Text::default();
        }

        if mode.contains(ClearMode::LINE) {
            self.data.reset_line_buffer();
            self.data.cursor_index = 0;
        }

        Ok(())
    }

    // Clears the output panel
    // * This is a public wrapper for the clear() method
    pub fn clear_output(&mut self) -> Result<()> {
        self.clear(ClearMode::OUTPUT)
    }

    // Prints a line of text to the console
    // TODO: Probably make this a macro in the future, but for now just make it use &str or String
    // TODO: Make lazy execution version of this, or a lazy execution mode
    pub fn println(&mut self, text: &str) {
        self.data.append_str_with_newline(text);
        _ = self.draw_frame(true)
    }

    // Prints a line of text to the console without a newline
    pub fn print(&mut self, text: &str) {
        self.data.append_str_without_newline(text);
        _ = self.draw_frame(true)
    }
}

impl Drop for Console<'_> {
    fn drop(&mut self) {
        restore_terminal();
    }
}

impl<'a> ConsoleData<'a> {
    fn new() -> Self {
        Self {
            prompt: Line::default(),
            prompt_tick: Span::styled(
                "❯ ",
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::LightGreen),
            ),
            success_tick_index: None,
            line_buffer: String::new(),
            output_buffer: Text::default(),
            debug_buffer: Text::default(),
            cursor_index: 0,
            autocomplete_buffer: None,
            history_buffer: None,
            history_index: None,
            scroll: 0,
            debug_mode: false,
        }
    }

    // Recolors the command output tick based on the command's exit status
    fn update_output_tick(&mut self, shell: &Shell) {
        // Get the tick from the output buffer
        // If the tick exists, it will be the first Span in the indexed Line
        let tick = {
            if let Some(index) = self.success_tick_index {
                if let Some(line) = self.output_buffer.lines.get_mut(index) {
                    line.spans.first_mut()
                } else {
                    return;
                }
            } else {
                return;
            }
        };

        let color = match shell.success() {
            true => Color::LightGreen,
            false => Color::LightRed,
        };

        // * If the tick is None, this is an extraneous call made before a command has been executed, and should be ignored
        if let Some(tick) = tick {
            tick.style = tick.style.fg(color);
        }
    }

    // Updates the prompt panel header based on the current shell state (USER, CWD, etc)
    // TODO: This will eventually need to not be hard-coded to allow for user customization
    fn update_prompt(&mut self, shell: &Shell) {
        let mut span_list = Vec::new();

        let home = shell.env().HOME();
        let truncation = shell.config().truncation_factor;
        // $ RGB values do not work on some terminals
        let user = Span::styled(
            shell.env().USER().clone(),
            Style::default()
                .fg(Color::Rgb(0, 150, 255))
                .add_modifier(Modifier::BOLD),
        );
        let cwd = Span::styled(
            shell.env().CWD().collapse(home, truncation),
            Style::default()
                .fg(Color::Rgb(0, 255, 0))
                .add_modifier(Modifier::BOLD),
        );

        span_list.push(user);
        span_list.push(Span::from(" on "));
        span_list.push(cwd);

        self.prompt = Line::from(span_list);

        // Color the prompt tick based on the last shell command's exit status
        match shell.success() {
            true => self.prompt_tick.style = self.prompt_tick.style.fg(Color::LightGreen),
            false => self.prompt_tick.style = self.prompt_tick.style.fg(Color::LightRed),
        }
    }

    // Updates the debug panel header based on the current shell state (USER, CWD, etc)
    fn update_debug(&mut self, shell: &Shell) {
        // Tracked items:
        // Console.line_buffer
        // Console.cursor_index
        // Console.autocomplete_buffer
        // Console.history_buffer
        // Console.history_index
        // Console.output_buffer.length
        // Console.scroll

        // Shell.config.truncation_factor
        // Shell.config.history_limit
        // Shell.config.show_errors

        // Shell.environment.USER
        // Shell.environment.HOME
        // Shell.environment.CWD

        let key_style = Style::default().add_modifier(Modifier::BOLD);
        let value_style = Style::default().fg(Color::LightGreen);

        let get_line = |key, value: &dyn Debug| {
            Line::from(vec![
                Span::styled(key, key_style),
                Span::styled(format!(" {:?}", value), value_style),
            ])
        };

        let line_buffer = get_line("LINE BUFFER:", &self.line_buffer);
        let cursor_index = get_line("CURSOR INDEX:", &self.cursor_index);
        let autocomplete_buffer = get_line("AUTOCOMPLETE BUFFER:", &self.autocomplete_buffer);
        let history_buffer = get_line("HISTORY BUFFER:", &self.history_buffer);
        let history_index = get_line("HISTORY INDEX:", &self.history_index);
        let output_buffer_length =
            get_line("OUTPUT BUFFER LENGTH:", &self.output_buffer.lines.len());
        let scroll = get_line("SCROLL:", &self.scroll);

        let truncation = get_line("PROMPT TRUNCATION:", &shell.config().truncation_factor);
        let history_limit = get_line("HISTORY LIMIT:", &shell.config().history_limit);
        let show_errors = get_line("SHOW ERRORS:", &shell.config().show_errors);

        let user = get_line("USER:", &shell.env().USER());
        let home = get_line("HOME:", &shell.env().HOME());
        let cwd = get_line("CWD:", &shell.env().CWD());

        self.debug_buffer = Text::from(vec![
            line_buffer,
            cursor_index,
            autocomplete_buffer,
            history_buffer,
            history_index,
            output_buffer_length,
            scroll,
            Line::default(),
            truncation,
            history_limit,
            show_errors,
            Line::default(),
            user,
            home,
            cwd,
        ])
    }

    // Updates the autocomplete buffer based on the current line buffer and the command history
    fn update_autocomplete(&mut self, shell: &Shell) {
        // If the current line buffer matches any of the commands in the history, put the rest of the command in the autocomplete buffer
        // Otherwise, clear the autocomplete buffer
        if !self.line_buffer.is_empty() {
            for command in &shell.command_history {
                if command.starts_with(&self.line_buffer) && command != &self.line_buffer {
                    let rest_of_command = command.strip_prefix(&self.line_buffer).unwrap();
                    self.autocomplete_buffer = Some(rest_of_command.to_string());
                    return;
                }
            }
        }

        self.autocomplete_buffer = None;
    }

    // Generates a TUI frame based on the prompt/line buffer and output buffer
    // ? Is there a way to make this a method to avoid passing in a ton of parameters?
    fn generate_frame(&mut self, f: &mut Frame<CrosstermBackend<Stdout>>, autoscroll: bool) {
        let prompt_borders = Block::default()
            .borders(Borders::ALL)
            .title(self.prompt.clone());
        let output_borders = |title| {
            Block::default()
                .borders(Borders::ALL ^ Borders::BOTTOM)
                .title(Span::styled(
                    title,
                    Style::default()
                        .fg(Color::LightCyan)
                        .add_modifier(Modifier::BOLD),
                ))
        };

        let mut line = Line::from(vec![
            self.prompt_tick.clone(),
            Span::from(self.line_buffer.clone()),
        ]);
        if let Some(autocomplete) = &self.autocomplete_buffer {
            line.spans.push(Span::styled(
                autocomplete.clone(),
                Style::default().add_modifier(Modifier::ITALIC | Modifier::DIM),
            ));
        }

        // Create a Paragraph widget for the prompt panel
        let prompt_widget = Paragraph::new(line)
            .block(prompt_borders)
            .style(Style::default())
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: false });

        // Create a Paragraph widget for the output panel
        let output_widget = Paragraph::new(self.output_buffer.clone())
            .block(output_borders("Output"))
            .style(Style::default())
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: false });

        // Split the terminal into two windows, one for the command output, and one for the prompt
        // The output window takes up the top 85% of the terminal, and the prompt window takes up the bottom 15%
        // If the debug panel is enabled, the output window will be split in 60/40 sections
        let (mut output_area, prompt_area) = {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(85), Constraint::Percentage(15)])
                .split(f.size());
            (chunks[0], chunks[1])
        };

        // If autoscroll is enabled, scroll to the bottom of the output buffer
        if autoscroll {
            self.scroll_to_bottom(output_area.height as usize)
        }

        // If the debug panel is enabled, subdivide the output window
        if self.debug_mode {
            let (new_output_area, debug_area) = {
                let chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                    .split(output_area);
                (chunks[0], chunks[1])
            };

            output_area = new_output_area;

            // Create a Paragraph widget for the debug panel
            let debug_widget = Paragraph::new(self.debug_buffer.clone())
                .block(output_borders("Debug"))
                .style(Style::default())
                .alignment(Alignment::Left)
                .wrap(Wrap { trim: false });

            // Render the debug panel widget
            if self.debug_mode {
                f.render_widget(debug_widget, debug_area)
            }
        }

        // Render the default widgets
        f.render_widget(prompt_widget, prompt_area);
        f.render_widget(output_widget.scroll((self.scroll as u16, 0)), output_area);

        // Render the cursor
        let (cursor_x, cursor_y) = Self::cursor_coord(self.cursor_index, prompt_area);
        f.set_cursor(cursor_x, cursor_y);
    }

    // Scrolls down the output panel by one line
    fn scroll_down(&mut self) {
        let max_scroll = self.output_buffer.lines.len();
        if self.scroll < max_scroll {
            self.scroll = self.scroll.saturating_add(1);
        }
    }

    // Scrolls up the output panel by one line
    fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    // Automatically scrolls to the bottom of the output panel text
    fn scroll_to_bottom(&mut self, output_panel_height: usize) {
        // * The -3 for is a bottom margin
        // TODO: Make the bottom margin configurable
        let output_panel_height = output_panel_height.saturating_sub(3);
        self.scroll = self
            .output_buffer
            .lines
            .len()
            .saturating_sub(output_panel_height);
    }

    // Scrolls through the Shell's command history
    // $ This might be confused with scrolling the output panel, so maybe rename it?
    fn scroll_history(&mut self, direction: HistoryDirection, shell: &Shell) -> Result<()> {
        use HistoryDirection::*;
        let history = shell.history();
        // If the history is empty, do nothing
        if history.is_empty() {
            return Ok(());
        }

        let history_get = |index: usize| {
            history
                .get(index)
                .expect("Tried to access non-existent command history")
        };

        let history_len = history.len();
        let history_last_index = history_len - 1;

        match self.history_index {
            // If the user is already scrolling through the history, move the index in the appropriate direction
            // If they attempt to scroll past the end of the history, restore the original line buffer
            Some(index) => {
                match direction {
                    Up => {
                        // Prevent the user from scrolling out of bounds
                        let new_index = index.saturating_sub(1);
                        self.history_index = Some(new_index);
                        self.cursor_index = history_get(new_index).len();
                    }
                    Down => {
                        // If the user scrolls back past the start of the history, restore the original line buffer
                        // Otherwise, keep scrolling down as normal
                        if index == history_last_index {
                            // TODO: Change this to an actual error
                            let history_buffer = self
                                .history_buffer
                                .take()
                                .expect("History buffer was not found when it should exist");
                            self.line_buffer = history_buffer.0;
                            self.cursor_index = history_buffer.1;
                            self.history_buffer = None;
                            self.history_index = None;
                        } else {
                            let new_index = index + 1;
                            self.history_index = Some(new_index);
                            self.cursor_index = history_get(new_index).len();
                        }
                    }
                }
            }
            // If the user is just starting to scroll through the history, start at the most recent history
            // If they attempt to scroll past the end of the history, do nothing
            None => {
                match direction {
                    Up => {
                        // * Bounds check is not needed in this case because it is guaranteed that history
                        // * contains at least one element due to the .is_empty() check
                        self.history_index = Some(history_last_index);
                        self.history_buffer = Some((self.line_buffer.clone(), self.cursor_index));
                        self.cursor_index = history_get(history_last_index).len();
                    }
                    Down => return Ok(()),
                }
            }
        }

        // TODO: Change this to an actual error
        if let Some(index) = self.history_index {
            self.line_buffer = history_get(index).to_owned();
        }

        Ok(())
    }

    // Inserts a character at the cursor position
    fn insert_char(&mut self, c: char) {
        self.line_buffer.insert(self.cursor_index, c);
        self.move_cursor_right();
    }

    // Removes a character from the line buffer at the cursor position
    fn remove_char(&mut self, mode: RemoveMode) {
        use RemoveMode::*;
        match mode {
            Backspace => {
                if self.cursor_index > 0 {
                    self.line_buffer.remove(self.cursor_index - 1);
                    self.move_cursor_left();
                }
            }
            Delete => {
                if self.cursor_index < self.line_buffer.len() {
                    self.line_buffer.remove(self.cursor_index);
                }
            }
        }
    }

    // Moves the cursor left by one character, checking for bounds
    fn move_cursor_left(&mut self) {
        self.cursor_index = self.cursor_index.saturating_sub(1);
    }

    // Moves the cursor right by one character, checking for bounds
    fn move_cursor_right(&mut self) {
        if self.cursor_index < self.line_buffer.len() {
            self.cursor_index += 1;
        }
    }

    // Moves the cursor left by one word, checking for bounds
    fn seek_cursor_left(&mut self) {
        for i in (0..self.cursor_index).rev() {
            if self.is_word_boundary(i) {
                self.cursor_index = i;
                return;
            }
        }
    }

    // Moves the cursor right by one word, checking for bounds
    fn seek_cursor_right(&mut self) {
        for i in (self.cursor_index..=self.line_buffer.len()).skip(1) {
            if self.is_word_boundary(i) {
                self.cursor_index = i;
                return;
            }
        }
    }

    // Checks if the given index in the line buffer is a word boundary
    fn is_word_boundary(&self, index: usize) -> bool {
        let buffer = &self.line_buffer;
        if index == 0 || index == buffer.len() {
            return true;
        }

        if !buffer.is_char_boundary(index) {
            return false;
        }

        if buffer[index..]
            .chars()
            .next()
            .map_or(false, |c| !c.is_whitespace())
        {
            return false;
        }

        if let Some(c) = buffer[..index].chars().next_back() {
            if c != ' ' {
                return true;
            }
        }

        false
    }

    // Clears the line buffer and resets the cursor position
    fn reset_line_buffer(&mut self) {
        self.line_buffer.clear();
        self.cursor_index = 0;
    }

    // Appends a string to the output buffer, splitting it into Line by newline characters so it is rendered properly
    fn append_str_without_newline(&mut self, string: &str) {
        // Return early on an empty string to allow for safely unwrapping the first line
        if string.is_empty() {
            return;
        }

        // This code is awful so I will try to give my best description of it
        // First, we have to split the string into lines and convert them into Line, because the Text type
        // does not render newline characters; instead, it requires that every line must be a separate Line
        let mut line = string.split('\n').map(str::to_owned).map(Line::from);
        // To avoid automatically creating a new line before the text is printed (which would effectively forbid print!()-type behavior),
        // we have to append directly to the last Line in the output buffer
        // So this line basically grabs the Vec<Span> from the first Line
        let first_line = line.next().unwrap().spans;

        // If the output buffer has any lines, we append the first line of the new text to the last line of the output buffer
        // Otherwise, we just push the first line of the new text to the output buffer in the form of a Line,
        // so the first line of the new text isn't just skipped on an empty output buffer
        if let Some(last_line) = self.output_buffer.lines.last_mut() {
            last_line.spans.extend(first_line);
        } else {
            self.output_buffer.lines.push(Line::from(first_line));
        }

        // The rest of the lines can then be appended to the output buffer as normal
        self.output_buffer.extend(line)
    }

    // Appends a string to the next line of the output buffer
    fn append_str_with_newline(&mut self, string: &str) {
        self.append_str_without_newline(string);
        self.append_newline()
    }

    // Appends a Line to the output buffer
    #[allow(dead_code)]
    fn append_line_without_newline(&mut self, line: Line<'a>) {
        self.output_buffer.lines.extend([line]);
    }

    // Appends a Line to the output buffer, adding a newline after it
    fn append_line_with_newline(&mut self, line: Line<'a>) {
        // TODO: Come up with a better name for this or merge it with append_newline() somehow
        self.output_buffer.lines.extend([line, Line::default()]);
    }

    // Appends a newline to the output buffer
    fn append_newline(&mut self) {
        self.output_buffer.lines.push(Line::default());
    }

    // Ensures that there is an empty line at the end of the output buffer
    // * This is used to make the prompt always appear one line below the last line of output, just for cosmetic purposes
    fn enforce_spacing(&mut self) {
        if let Some(last_line) = self.output_buffer.lines.last_mut() {
            // TODO: Find a less ugly way to do this
            if !last_line.spans.is_empty() && last_line.spans.last() != Some(&Span::raw("")) {
                self.output_buffer.lines.push(Line::default());
            }
        }
    }

    // Autocompletes the line buffer
    fn autocomplete_line(&mut self) {
        if let Some(autocompletion) = &self.autocomplete_buffer {
            self.line_buffer.push_str(autocompletion);
            self.cursor_index = self.line_buffer.len();
            self.autocomplete_buffer = None;
        }
    }

    // Given the cursor index and the Rect of the prompt panel, returns the terminal cursor position
    // $ This only works on the first line due to soft-wrapping
    fn cursor_coord(cursor_index: usize, prompt_area: Rect) -> (u16, u16) {
        // Get the prompt panel width to determine the starting y-position of the cursor
        // * The -2 is to account for the left and right borders
        let prompt_width = prompt_area.width as usize - 2;
        // * The +1 is to account for the top border
        let prompt_y_coord = (prompt_area.y + 1) as usize;

        // Find the x and y offsets based on the cursor index
        let y_offset = cursor_index / prompt_width + prompt_y_coord;
        // * The +2 is to account for the prompt tick, and the space after the tick
        let x = (cursor_index + 3) % prompt_width;

        (x as u16, y_offset as u16)
    }
}
