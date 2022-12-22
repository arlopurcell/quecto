use termion::{raw::{IntoRawMode, RawTerminal}, input::TermRead, event::{Event, Key}};
use std::{io::{Write, stdin, stdout, Error, Stdout, BufRead, BufReader}, collections::LinkedList, iter::once};
use std::fs::OpenOptions;

use crate::{buffer::Buffer, log};

enum EditorMode {
    Normal,
    Insert,
    Command,
    // TODO Menu
    // TODO Dialog
    // TODO TreeNav
}

pub struct EditorState {
    mode: EditorMode,
    file_name: Option<String>,
    pub exit: bool,

    buffer: Buffer,
    cursor: RelativePosition,

    current_line_screen_pos: usize,

    command_buffer: String,
    command_cursor: usize,

    status_message: String,
}

impl EditorState {
    pub fn new () -> Self {
        Self {
            mode: EditorMode::Normal,
            file_name: None,
            //term,
            exit: false,

            buffer: Buffer::new(),
            cursor: RelativePosition::new(),
            
            current_line_screen_pos: 0,

            command_buffer: "".to_string(),
            command_cursor: 0,

            status_message: "".to_string(),
        }
    }

    pub fn update(&mut self, evt: Event, term_height: u16) -> Result<(), Error> {
        let drawable_height = term_height - 2;
        match self.mode {
            EditorMode::Normal => match evt {
                    Event::Key(Key::Char('j')) | Event::Key(Key::Down) => self.down(drawable_height),
                    Event::Key(Key::Char('k')) | Event::Key(Key::Up) => self.up(),
                    Event::Key(Key::Char('h')) | Event::Key(Key::Left) => self.left(),
                    Event::Key(Key::Char('l')) | Event::Key(Key::Right) => self.right(),
                    Event::Key(Key::Char('i')) => self.mode = EditorMode::Insert,
                    Event::Key(Key::Char('a')) => {
                        self.right();
                        self.mode = EditorMode::Insert;
                    }
                    // TODO A, I
                    Event::Key(Key::Char('o')) => {
                        self.buffer.new_empty_line();
                        //self.cursor.down();
                        self.cursor_down(drawable_height);
                        self.cursor.full_left();
                        self.mode = EditorMode::Insert;
                    }
                    // TODO O
                    Event::Key(Key::Char(':')) => {
                        self.mode = EditorMode::Command;
                        self.command_buffer.clear();
                        self.command_cursor = 0;
                    }
                    _ => ()
                }
            EditorMode::Insert => match evt {
                Event::Key(Key::Esc) => {
                    self.mode = EditorMode::Normal;
                    self.trim_cursor();
                }
                Event::Key(Key::Up) => self.up(),
                Event::Key(Key::Down) => self.down(drawable_height),
                Event::Key(Key::Left) => self.left(),
                Event::Key(Key::Right) => self.right(),
                Event::Key(Key::Char('\n')) => {
                    self.buffer.new_line(self.cursor.x as usize);
                    //self.cursor.down();
                    self.cursor_down(drawable_height);
                    self.cursor.full_left();
                    self.mode = EditorMode::Insert;
                }
                Event::Key(Key::Char(c)) => {
                    self.buffer.insert(self.cursor.x as usize, c);
                    self.cursor.right();
                }
                Event::Key(Key::Backspace) => {
                    if self.cursor.x != 0 {
                        self.cursor.left();
                        self.buffer.delete(self.cursor.x as usize);
                    } else {
                        if let Some(new_cursor_x) = self.buffer.merge_line_to_prev() {
                            //self.cursor.up();
                            self.cursor_up();
                            self.cursor.go_to_x(new_cursor_x);
                        }
                    }
                }
                Event::Key(Key::Delete) => {
                    self.buffer.delete(self.cursor.x as usize);
                    // TODO implement deleting next new line
                }
                _ => ()
            }
            EditorMode::Command => match evt {
                Event::Key(Key::Esc) => self.mode = EditorMode::Normal,
                Event::Key(Key::Char('\n')) => {
                    self.execute_command()?;
                    self.mode = EditorMode::Normal;
                }
                Event::Key(Key::Char(c)) => {
                    self.command_buffer.insert(self.command_cursor, c);
                    self.command_cursor += 1;
                }
                Event::Key(Key::Backspace) => {
                    if self.command_cursor != 0 {
                        self.command_cursor -= 1;
                        self.command_buffer.remove(self.command_cursor);
                    }
                }
                Event::Key(Key::Delete) => {
                    if self.command_buffer.len() < self.command_cursor {
                        self.command_buffer.remove(self.command_cursor);
                    }
                }
                // TODO implement readline movement Ctrl-A, Ctrl-E etc.
                // TODO up/down for history
                _ => ()
            }
        }
        Ok(())
    }

    fn up(&mut self) {
        if self.buffer.up() {
            self.cursor_up();
        }
    }

    fn cursor_up(&mut self) {
        if self.current_line_screen_pos != 0 {
            self.current_line_screen_pos -= 1;
            self.cursor.up();
        }
        self.trim_cursor();
    }

    fn down(&mut self, drawable_height: u16) {
        if self.buffer.down() {
            self.cursor_down(drawable_height);
        }
    }

    fn cursor_down(&mut self, drawable_height: u16) {
        if self.current_line_screen_pos != drawable_height as usize - 1 {
            self.current_line_screen_pos += 1;
            log(format!("new clsp: {}", self.current_line_screen_pos).as_ref());
            self.cursor.down();
        }
        self.trim_cursor();
    }

    fn left(&mut self) {
        self.cursor.left();
    }

    fn right(&mut self) {
        self.cursor.right();
        self.trim_cursor();
    }

    fn trim_cursor(&mut self) {
        if self.buffer.current_line_len() == 0 {
            self.cursor.x = 0;
        } else {
            self.cursor.x = u16::min(self.cursor.x, self.buffer.current_line_len() as u16 - match self.mode {
                EditorMode::Insert => 0,
                _ => 1,
            });
        }
    }
     
    pub fn render(&mut self, term: &mut RawTerminal<Stdout>) -> Result<(), Error> {
        let (_term_width, term_height) = termion::terminal_size()?;
        let drawable_height = term_height - 2;
        term.write(termion::clear::All.as_ref())?;
        term.write(match self.mode {
            EditorMode::Insert => termion::cursor::BlinkingBar.as_ref(),
            EditorMode::Command => termion::cursor::BlinkingBar.as_ref(),
            EditorMode::Normal => termion::cursor::SteadyBlock.as_ref(),
        })?;


        let on_screen_lines = self.buffer.visible_lines(drawable_height as usize, self.current_line_screen_pos);
        for (index, line) in on_screen_lines.iter().enumerate() {
            //log(&format!("writing line {} at index {}", line, index));
            term.write(format!(
                    "{}~ {}", 
                    termion::cursor::Goto(1, index as u16 + 1),
                    //&line[0..(term_width - 2) as usize]
                    line
            ).as_bytes())?;
        }
        //log("drew lines");

        // draw status bar
        self.draw_status_bar(term, term_height)?;
        self.draw_command_line(term, term_height)?;
        self.render_cursor(term, term_height)?;

        term.flush()?;
        //log("flushed");
        Ok(())
    }

    fn draw_status_bar(&mut self, term: &mut RawTerminal<Stdout>, term_height: u16) -> Result<(), Error> {
        term.write(format!(
            "{}{}",
            termion::cursor::Goto(1, term_height - 1),
            match self.mode {
                EditorMode::Normal => "NORMAL",
                EditorMode::Insert => "INSERT",
                EditorMode::Command => "COMMAND",
            }
        ).as_bytes())?;
        Ok(())
    }

    fn draw_command_line(&mut self, term: &mut RawTerminal<Stdout>, term_height: u16) -> Result<(), Error> {
        match self.mode {
            EditorMode::Command => {
                term.write(format!("{}:{}", termion::cursor::Goto(1, term_height), self.command_buffer).as_bytes())?;
            }
            _ => (),
        }
        Ok(())
    }
    
    fn render_cursor(&mut self, term: &mut RawTerminal<Stdout>, term_height: u16) -> Result<(), Error> {
        match self.mode {
            EditorMode::Command => {
                term.write(format!("{}", termion::cursor::Goto(self.command_cursor as u16 + 2, term_height)).as_bytes())?;
            }
            _ => self.cursor.goto(term)?
        }
        Ok(())
    }

    fn execute_command(&mut self) -> Result<(), Error> {
        // TODO split command_buffer by white space to allow commands with args
        let args: Vec<&str> = self.command_buffer.split(' ').collect();
        let command = args.get(0).unwrap(); // TODO handle empty command
        // TODO re-write to consume strings off args rather than reference. this should avoid some
        // allocations

        if "quit".starts_with(command) {
            self.exit = true;
        } else if "write".starts_with(command) {
            if let Some(file_name) = args.get(1) {
                self.file_name = Some(file_name.to_string());
            }
            if let Some(file_name) = &self.file_name {
                let mut file = OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open(file_name)
                    .unwrap(); // TODO handle failure to open file

                for line in self.buffer.pre.iter() {
                    write!(&mut file, "{}\n", line)?;
                }
                write!(&mut file, "{}\n", self.buffer.current)?;
                for line in self.buffer.post.iter() {
                    write!(&mut file, "{}\n", line)?;
                }
            } else {
                // TODO handle no file name
            }
        } else if "edit".starts_with(command) {
            // TODO share some of the file name logic with "write"
            if let Some(file_name) = args.get(1) {
                self.file_name = Some(file_name.to_string());
            }
            if let Some(file_name) = &self.file_name {
                let file = OpenOptions::new()
                    .read(true)
                    .open(file_name)
                    .unwrap(); // TODO handle failure to open file
                let lines = BufReader::new(file)
                    .lines()
                    .map(|line| line.unwrap()); // TODO handle failure to read file

                // TODO handle failure to read file
                self.buffer = Buffer::from_lines(lines);
                self.cursor = RelativePosition::new();

            } else {
                // TODO handle no file name
            }
        }
        self.command_buffer.clear();
        self.command_cursor = 0;
        Ok(())
    }
}

struct RelativePosition {
    x: u16,
    y: u16,
}

impl RelativePosition {
    fn new() -> Self {
        RelativePosition { x: 0, y: 0 }
    }

    fn goto(&self, term: &mut RawTerminal<Stdout>) -> Result<(), Error> {
        term.write(format!("{}", termion::cursor::Goto(self.x + 3, self.y + 1)).as_bytes())?;
        Ok(())
    }

    fn up(&mut self) {
        self.y -= if self.y == 0 { 0 } else { 1 };
    }

    fn down(&mut self) {
        self.y += 1; // TODO limit to screen size
        // TODO implement scrolling
    }

    fn left(&mut self) {
        self.x -= if self.x == 0 { 0 } else { 1 };
    }

    fn full_left(&mut self) {
        self.x = 0;
    }

    fn right(&mut self) {
        self.x += 1; // TODO limit to screen size
        // TODO implement scrolling
    }

    fn go_to_x(&mut self, new_x: usize) {
        self.x = new_x as u16;
    }
}
