use std::io::{stdout, Stdout};

use anyhow::Result;
use crossterm::terminal::{enable_raw_mode, disable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen, Clear, ClearType};
use crossterm::event::{self, Event, KeyCode, KeyModifiers, DisableMouseCapture};
use crossterm::cursor;
use crossterm::execute;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Layout, Direction, Constraint, Alignment};
use ratatui::text::{Span, Spans, Text};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::{Terminal, Frame};
use bitflags::bitflags;

use crate::shell::Shell;

// Represents an action that the handler instructs the REPL (Console.read()) to perform
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

// More readable variant of a switch between "backspace" and "delete" keypresses for Console.remove_char()
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
        const RESET_LINE = 0b00000010;
    }
}

// Represents the TUI console
pub struct Console<'a> {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    // ? Should this be an Option<Spans>?
    prompt: Spans<'a>,
    // ? What is the actual name of this?
    prompt_tick: Span<'a>,
    line_buffer: String,
    output_buffer: Text<'a>,
    debug_buffer: Text<'a>,
    // The index of the cursor in the line buffer
    // ? Should this be an Option<usize>?
    cursor_index: usize,
    // The number of lines that have been scrolled up
    scroll: usize,
    // Whether or not to show the debug panel
    debug_mode: bool,
}

impl<'a> Console<'a> {
    pub fn new() -> Result<Self> {
        let backend = CrosstermBackend::new(stdout());
        let terminal = Terminal::new(backend)?;

        Ok(Self {
            terminal,
            prompt: Spans::default(),
            prompt_tick: Span::styled("❯ ", Style::default().add_modifier(Modifier::BOLD | Modifier::SLOW_BLINK).fg(Color::LightGreen)),
            line_buffer: String::new(),
            output_buffer: Text::default(),
            debug_buffer: Text::default(),
            cursor_index: 0,
            scroll: 0,
            debug_mode: false,
        })
    }

    // Enters the TUI console
    pub fn enter(&mut self) -> Result<()> {
        enable_raw_mode()?;
        // ? Is mouse capture enabled by default?
        execute!(self.terminal.backend_mut(), EnterAlternateScreen, DisableMouseCapture)?;

        self.clear(ClearMode::RESET_LINE)
    }

    // Closes the TUI console
    pub fn close(&mut self) -> Result<()> {
        disable_raw_mode()?;
        execute!(self.terminal.backend_mut(), LeaveAlternateScreen, cursor::MoveTo(0, 0), cursor::Show, Clear(ClearType::All))?;
        Ok(())
    }

    // Reads a line of input from the user
    // Handles all TUI interaction between the user and the prompt
    pub fn read_line(&mut self, shell: &Shell) -> Result<String> {
        // The line buffer must be reset manually because Console.prompt() does not clear it
        self.reset_line_buffer();
        self.update_prompt(shell);
        self.update_debug(shell);
        self.draw()?;

        loop {
            let event = event::read()?;
            let action = self.handle_event(event)?;

            match action {
                ReplAction::Return => {
                    // Make sure that there is an extra line of space between the last line of output and the command output
                    self.enforce_spacing();

                    // Save the line buffer for returning and clear it to make way for the next Console.read_line() call
                    let line = self.line_buffer.clone();
                    self.line_buffer.clear();
                    
                    // Save the line buffer as part of the output buffer
                    self.append_newline(&line);
                    
                    return Ok(line)
                },
                ReplAction::Exit => {
                    self.close()?;
                    std::process::exit(0);
                },
                ReplAction::RedrawFrame => {
                    self.draw()?;
                },
                ReplAction::Ignore => (),
            }
        }
    }

    // Handles a key event by queueing appropriate commands based on the given keypress
    fn handle_event(&mut self, event: Event) -> Result<ReplAction> {
        // TODO: Break up event handling into separate functions for different event categories
        match event {
            Event::Key(event) => {
                match (event.modifiers, event.code) {
                    (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(c)) => self.insert_char(c),
                    (KeyModifiers::NONE, KeyCode::Backspace) => self.remove_char(RemoveMode::Backspace),
                    (KeyModifiers::NONE, KeyCode::Delete) => self.remove_char(RemoveMode::Delete),
                    (KeyModifiers::NONE, KeyCode::Left) => self.move_cursor_left(),
                    (KeyModifiers::NONE, KeyCode::Right) => self.move_cursor_right(),
                    (KeyModifiers::NONE, KeyCode::Enter) if !self.line_buffer.is_empty() => return Ok(ReplAction::Return),
                    (KeyModifiers::SHIFT, KeyCode::Up) => self.scroll = self.scroll.saturating_sub(1),
                    (KeyModifiers::SHIFT, KeyCode::Down) => self.scroll = self.scroll.saturating_add(1),
                    // (KeyModifiers::NONE, KeyCode::Up) => self.scroll_history(HistoryDirection::Up, context)?,
                    // (KeyModifiers::NONE, KeyCode::Down) => self.scroll_history(HistoryDirection::Down, context)?,
                    (KeyModifiers::CONTROL, KeyCode::Char('c')) => return Ok(ReplAction::Exit),
                    (KeyModifiers::CONTROL, KeyCode::Char('l')) => self.clear(ClearMode::OUTPUT)?,
                    // TODO: Make this a toggle method
                    (KeyModifiers::CONTROL, KeyCode::Char('d')) => self.debug_mode = !self.debug_mode,
                    _ => return Ok(ReplAction::Ignore),
                }
            }
            // $ This seems like a crappy solution to prevent the Resize event from being ignored
            Event::Resize(_, _) => (),
            _ => return Ok(ReplAction::Ignore),
        }

        Ok(ReplAction::RedrawFrame)
    }

    // Updates the prompt panel header based on the current shell state (USER, CWD, etc)
    // TODO: This will eventually need to not be hard-coded to allow for user customization
    fn update_prompt(&mut self, shell: &Shell) {
        let mut span_list = Vec::new();

        let home = shell.env().HOME();
        let truncation = shell.config().truncation_factor;
        // $ RGB values do not work on some terminals
        let user = Span::styled(shell.env().USER().clone(), Style::default().fg(Color::Rgb(0, 150, 255)).add_modifier(Modifier::BOLD));
        let cwd = Span::styled(shell.env().CWD().collapse(home, truncation), Style::default().fg(Color::Rgb(0, 255, 0)).add_modifier(Modifier::BOLD));

        span_list.push(user);
        span_list.push(Span::from(" on "));
        span_list.push(cwd);

        self.prompt = Spans::from(span_list);

        // Color the prompt tick based on the last shell command's exit status
        match shell.success() {
            true => self.prompt_tick.style = self.prompt_tick.style.fg(Color::LightGreen),
            false => self.prompt_tick.style = self.prompt_tick.style.fg(Color::LightRed),
        }
    }

    // Updates the debug panel header based on the current shell state (USER, CWD, etc)
    fn update_debug(&mut self, shell: &Shell) {
        let success = Spans::from(format!("Success: {}", shell.success()));
        self.debug_buffer.extend(Text::from(success));
    }

    // Draws a TUI frame
    pub fn draw(&mut self) -> Result<()> {
        self.terminal.draw(|f| Self::generate_frame(f, self.debug_mode, &self.debug_buffer, &self.prompt, &self.prompt_tick, &self.line_buffer, &self.output_buffer, self.scroll))?;
        Ok(())
    }

    // Generates a TUI frame based on the prompt/line buffer and output buffer
    // ? Is there a way to make this a method to avoid passing in a ton of parameters?
    fn generate_frame(f: &mut Frame<CrosstermBackend<Stdout>>, debug_mode: bool, debug_buffer: &Text<'a>, prompt: &Spans, prompt_tick: &Span, line_buffer: &str, output_buffer: &Text, scroll: usize) {
        let prompt_borders = Block::default().borders(Borders::ALL).title(prompt.clone());
        let frame_borders = |title| Block::default().borders(Borders::ALL ^ Borders::BOTTOM).title(Span::styled(title, Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD)));

        let line = Spans::from(vec![prompt_tick.clone(), Span::from(line_buffer)]);
        
        // Create a Paragraph widget for the prompt panel
        let prompt_widget = Paragraph::new(line)
            .block(prompt_borders)
            .style(Style::default())
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: false });

        // Create a Paragraph widget for the output panel
        let frame_widget = Paragraph::new(output_buffer.clone())
            .block(frame_borders("Output"))
            .style(Style::default())
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: false });

        // Split the terminal into two windows, one for the command output, and one for the prompt
        // The output window takes up the top 80% of the terminal, and the prompt window takes up the bottom 20%
        // If the debug panelt is enabled, the output window will be split in 60/40 sections
        let (mut output_area, prompt_area) = {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(80), Constraint::Percentage(20)])
                .split(f.size());
            (chunks[0], chunks[1])
        };

        // If the debug panel is enabled, subdivide the output window
        if debug_mode {
            let (new_output_area, debug_area) = {
                let chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                    .split(output_area);
                (chunks[0], chunks[1])
            };

            output_area = new_output_area;

            // Create a Paragraph widget for the debug panel
            let debug_widget = Paragraph::new(debug_buffer.clone())
                .block(frame_borders("Debug"))
                .style(Style::default())
                .alignment(Alignment::Left)
                .wrap(Wrap { trim: false });

            // Render the debug panel widget
            if debug_mode { f.render_widget(debug_widget, debug_area) }
        }

        // Render the default widgets
        f.render_widget(prompt_widget, prompt_area);
        f.render_widget(frame_widget.scroll((scroll as u16, 0)), output_area);
    }

    // Clears the screen and the line buffer and reprompts the user
    fn clear(&mut self, mode: ClearMode) -> Result<()> {
        // Clear the output panel
        if mode.contains(ClearMode::OUTPUT) {
            self.output_buffer = Text::default();
        }

        if mode.contains(ClearMode::RESET_LINE) {
            self.reset_line_buffer();
            self.cursor_index = 0;
        }

        Ok(())
    }

    // Clears the output panel
    // * This is a public wrapper for the clear() method
    pub fn clear_output(&mut self) -> Result<()> {
        self.clear(ClearMode::OUTPUT)
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
            },
            Delete => {
                if self.cursor_index < self.line_buffer.len() {
                    self.line_buffer.remove(self.cursor_index);
                }
            },
        }
    }

    // Moves the cursor left by one character, checking for bounds
    fn move_cursor_left(&mut self) {
        if self.cursor_index > 0 {
            self.cursor_index -= 1;
        }
    }

    // Moves the cursor right by one character, checking for bounds
    fn move_cursor_right(&mut self) {
        if self.cursor_index < self.line_buffer.len() {
            self.cursor_index += 1;
        }
    }

    // Clears the line buffer and resets the cursor position
    fn reset_line_buffer(&mut self) {
        self.line_buffer.clear();
        self.cursor_index = 0;
    }

    // Prints a line of text to the console
    // TODO: Probably make this a macro in the future, but for now just make it use &str or String
    // TODO: Make lazy execution version of this, or a lazy execution mode
    pub fn println(&mut self, text: &str) {
        self.append_newline(text);
        _ = self.draw()
    }

    // Appends a string to the output buffer, splitting it into Spans by newline characters so it is rendered properly
    fn append_str(&mut self, string: &str) {
        // Return early on an empty string to allow for safely unwrapping the first line
        if string.is_empty() {
            return
        }

        // This code is awful so I will try to give my best description of it
        // First, we have to split the string into lines and convert them into Spans, because the Text type
        // does not render newline characters; instead, it requires that every line must be a separate Spans
        let mut spans = string.split('\n').map(str::to_owned).map(Spans::from);
        // To avoid automatically creating a new line before the text is printed (which would effectively forbid print!()-type behavior),
        // we have to append directly to the last Spans in the output buffer
        // So this line basically grabs the Vec<Span> from the first Spans (first line)
        let first_spans = spans.next().unwrap().0;

        // If the output buffer has any lines, we append the first line of the new text to the last line of the output buffer
        // Otherwise, we just push the first line of the new text to the output buffer in the form of a Spans,
        // so the first line of the new text isn't just skipped on an empty output buffer
        if let Some(last_line) = self.output_buffer.lines.last_mut() {
            last_line.0.extend(first_spans);
        } else {
            self.output_buffer.lines.push(Spans::from(first_spans));
        }

        // The rest of the lines (Spans) can then be appended to the output buffer as normal
        self.output_buffer.extend(spans)
    }

    // Appends a string to the next line of the output buffer
    fn append_newline(&mut self, string: &str) {
        self.append_str(&format!("{}\n", string))
    }

    // Ensures that there is an empty line at the end of the output buffer
    // * This is used to make the prompt always appear one line below the last line of output, just for cosmetic purposes
    fn enforce_spacing(&mut self) {
        if let Some(last_line) = self.output_buffer.lines.last_mut() {
            if !last_line.0.is_empty() {
                self.output_buffer.lines.push(Spans::default());
            }
        }
    }
}
