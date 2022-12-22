use editor_state::EditorState;
use termion::{raw::{IntoRawMode, RawTerminal}, input::TermRead, event::{Event, Key}};
use std::{io::{Write, stdin, stdout, Error, Stdout, BufRead, BufReader}, collections::LinkedList, iter::once};
use std::fs::OpenOptions;

mod editor_state;
mod buffer;

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

    let mut editor_state = EditorState::new();
    editor_state.render(&mut stdout)?;
    
    for c in stdin.events() {
        let evt = c?;
        let (_term_width, term_height) = termion::terminal_size()?;
        editor_state.update(evt, term_height)?;
        if editor_state.exit {
            stdout.write(termion::clear::All.as_ref())?;
            stdout.flush()?;
            break;
        }
        editor_state.render(&mut stdout)?;
    }

    Ok(())
}

pub fn log(msg: &str) {
    // TODO create file if not exists
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .append(true)
        // TODO get log location from config or something
        .open("quecto.log")
        .unwrap();

    if let Err(_e) = file.write_all(format!("{}\n", msg).as_bytes()) {
        panic!("couldn't write to log file")
    }
}
