use termion::{raw::{IntoRawMode, RawTerminal}, input::TermRead, event::{Event, Key}};
use std::{io::{Write, stdin, stdout, Error, Stdout, BufRead, BufReader}, collections::LinkedList, iter::once};
use std::fs::OpenOptions;


fn main() {
    log("starting quecto");
    if let Err(e) = run() {
        log("ERROR!");
        log(&e.to_string());
        std::process::exit(1);
    }
}

fn run() -> Result<(), Error> {
    let mut stdout = stdout().into_raw_mode().unwrap();
    let stdin = stdin();

    stdout.write(termion::clear::All.as_ref())?;
    stdout.flush()?;

    let mut editor_state = EditorState::new(stdout);
    editor_state.render()?;
    
    for c in stdin.events() {
        let evt = c?;
        editor_state.update(evt)?;
        if editor_state.exit {
            editor_state.term.write(termion::clear::All.as_ref())?;
            editor_state.term.flush()?;
            break;
        }
        editor_state.render()?;
    }

    Ok(())
}

struct EditorState {
    mode: EditorMode,
    file_name: Option<String>,
    term: RawTerminal<Stdout>,
    exit: bool,

    buffer: Buffer,
    cursor: RelativePosition,

    command_buffer: String,
    command_cursor: usize,

    status_message: String,
}

impl EditorState {
    fn new (term: RawTerminal<Stdout>) -> Self {
        Self {
            mode: EditorMode::Normal,
            file_name: None,
            term,
            exit: false,

            buffer: Buffer::new(),
            cursor: RelativePosition::new(),

            command_buffer: "".to_string(),
            command_cursor: 0,

            status_message: "".to_string(),
        }
    }

    fn update(&mut self, evt: Event) -> Result<(), Error> {
        match self.mode {
            EditorMode::Normal => match evt {
                    Event::Key(Key::Char('q')) => {
                        self.exit = true;
                    }
                    Event::Key(Key::Char('j')) | Event::Key(Key::Down) => self.down(),
                    Event::Key(Key::Char('k')) | Event::Key(Key::Up) => self.up(),
                    Event::Key(Key::Char('h')) | Event::Key(Key::Left) => self.left(),
                    Event::Key(Key::Char('l')) | Event::Key(Key::Right) => self.right(),
                    Event::Key(Key::Char('i')) => self.mode = EditorMode::Insert,
                    Event::Key(Key::Char('a')) => {
                        self.cursor.right();
                        self.mode = EditorMode::Insert;
                    }
                    // TODO A, I
                    Event::Key(Key::Char('o')) => {
                        self.buffer.new_empty_line();
                        self.cursor.down();
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
                Event::Key(Key::Down) => self.down(),
                Event::Key(Key::Left) => self.left(),
                Event::Key(Key::Right) => self.right(),
                Event::Key(Key::Char('\n')) => {
                    self.buffer.new_line(self.cursor.x as usize);
                    self.cursor.down();
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
                            self.cursor.up();
                            self.cursor.go_to_x(new_cursor_x);
                        }
                    }
                }
                Event::Key(Key::Delete) => {
                    self.buffer.delete(self.cursor.x as usize);
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
            self.cursor.up();
            self.trim_cursor();
        }
    }

    fn down(&mut self) {
        if self.buffer.down() {
            self.cursor.down();
            self.trim_cursor();
        }
    }

    fn left(&mut self) {
        self.cursor.left();
    }

    fn right(&mut self) {
        self.cursor.right();
        self.trim_cursor();
    }

    fn trim_cursor(&mut self) {
        self.cursor.x = u16::min(self.cursor.x, self.buffer.current.len() as u16 - match self.mode {
            EditorMode::Insert => 0,
            _ => 1,
        })
    }
     
    fn render(&mut self) -> Result<(), Error> {
        let (_term_width, term_height) = termion::terminal_size()?;
        self.term.write(termion::clear::All.as_ref())?;
        self.term.write(match self.mode {
            EditorMode::Insert => termion::cursor::BlinkingBar.as_ref(),
            EditorMode::Command => termion::cursor::BlinkingBar.as_ref(),
            EditorMode::Normal => termion::cursor::SteadyBlock.as_ref(),
        })?;

        // TODO move into Buffer impl
        let on_screen_lines = self.buffer.pre.iter().rev().take(self.cursor.y as usize).rev()
            .chain(once(&self.buffer.current))
            .chain(self.buffer.post.iter().take((term_height - self.cursor.y - 1) as usize));
        log("split lines");

        for (index, line) in on_screen_lines.enumerate() {
            log(&format!("writing line {} at index {}", line, index));
            self.term.write(format!(
                    "{}~ {}", 
                    termion::cursor::Goto(1, index as u16 + 1),
                    //&line[0..(term_width - 2) as usize]
                    line
            ).as_bytes())?;
        }
        log("drew lines");

        // draw status bar
        self.draw_status_bar(term_height)?;
        self.draw_command_line(term_height)?;
        self.render_cursor(term_height)?;

        self.term.flush()?;
        log("flushed");
        Ok(())
    }

    fn draw_status_bar(&mut self, term_height: u16) -> Result<(), Error> {
        self.term.write(format!(
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

    fn draw_command_line(&mut self, term_height: u16) -> Result<(), Error> {
        match self.mode {
            EditorMode::Command => {
                self.term.write(format!("{}:{}", termion::cursor::Goto(1, term_height), self.command_buffer).as_bytes())?;
            }
            _ => (),
        }
        Ok(())
    }
    
    fn render_cursor(&mut self, term_height: u16) -> Result<(), Error> {
        match self.mode {
            EditorMode::Command => {
                self.term.write(format!("{}", termion::cursor::Goto(self.command_cursor as u16 + 2, term_height)).as_bytes())?;
            }
            _ => self.cursor.goto(&mut self.term)?
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

struct Buffer {
    pre: LinkedList<String>,
    current: String,
    post: LinkedList<String>,
}

impl Buffer {
    fn new() -> Self {
        Self {
            pre: LinkedList::new(),
            current: "".to_string(),
            post: LinkedList::new(),
        }
    }

    fn from_lines<L>(mut lines: L) -> Self
        where L: Iterator<Item = String>,
    {
        let current = lines.next().unwrap_or_else(|| "".to_string());
        let post: LinkedList<String> = lines.collect();
        Buffer{pre: LinkedList::new(), current, post}
    }

    fn up(&mut self) -> bool {
        if let Some(mut line) = self.pre.pop_back() {
            std::mem::swap(&mut line, &mut self.current);
            self.post.push_front(line);
            //self.current = line;
            true
        } else {
            false
        }
    }

    fn down(&mut self) -> bool {
        if let Some(mut line) = self.post.pop_front() {
            std::mem::swap(&mut line, &mut self.current);
            self.pre.push_back(line);
            //self.current = line;
            true
        } else {
            false
        }
    }

    fn insert(&mut self, position: usize, c: char) {
        self.current.insert(position, c);
    }

    fn delete(&mut self, position: usize) {
        self.current.remove(position);
    }

    fn new_empty_line(&mut self) {
        let mut line = "".to_string();
        std::mem::swap(&mut line, &mut self.current);
        self.pre.push_back(line);
    }

    fn new_line(&mut self, position: usize) {
        let mut line = self.current.split_off(position);
        std::mem::swap(&mut line, &mut self.current);
        self.pre.push_back(line);
    }
    
    // Merges the current line with the previous line, makes that the current line, and returns the
    // length of the previous line (i.e. where the cursor should be)
    fn merge_line_to_prev(&mut self) -> Option<usize> {
        self.pre.pop_back().map(|prev_line| {
            self.current.insert_str(0, &prev_line);
            prev_line.len()
        })
    }
}

enum EditorMode {
    Normal,
    Insert,
    Command,
    // TODO Menu
    // TODO Dialog
    // TODO TreeNav
}

fn log(msg: &str) {
    // TODO create file if not exists
    let mut file = OpenOptions::new()
        .write(true)
        .append(true)
        // TODO get log location from config or something
        .open("quecto.log")
        .unwrap();

    if let Err(_e) = file.write_all(format!("{}\n", msg).as_bytes()) {
        panic!("couldn't write to log file")
    }
}