use termion::{raw::{IntoRawMode, RawTerminal}, input::TermRead, event::{Event, Key}};
use std::{io::{Write, stdin, stdout, Error, Stdout, BufRead, BufReader}, collections::LinkedList, iter::once};
use std::fs::OpenOptions;

pub struct Buffer {
    // TODO make these not pub
    pub pre: LinkedList<String>,
    pub current: String,
    pub post: LinkedList<String>,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            pre: LinkedList::new(),
            current: "".to_string(),
            post: LinkedList::new(),
        }
    }

    pub fn from_lines<L>(mut lines: L) -> Self
        where L: Iterator<Item = String>,
    {
        let current = lines.next().unwrap_or_else(|| "".to_string());
        let post: LinkedList<String> = lines.collect();
        Buffer{pre: LinkedList::new(), current, post}
    }

    pub fn up(&mut self) -> bool {
        if let Some(mut line) = self.pre.pop_back() {
            std::mem::swap(&mut line, &mut self.current);
            self.post.push_front(line);
            //self.current = line;
            true
        } else {
            false
        }
    }

    pub fn down(&mut self) -> bool {
        if let Some(mut line) = self.post.pop_front() {
            std::mem::swap(&mut line, &mut self.current);
            self.pre.push_back(line);
            //self.current = line;
            true
        } else {
            false
        }
    }

    pub fn insert(&mut self, position: usize, c: char) {
        self.current.insert(position, c);
    }

    pub fn delete(&mut self, position: usize) {
        self.current.remove(position);
    }

    pub fn new_empty_line(&mut self) {
        let mut line = "".to_string();
        std::mem::swap(&mut line, &mut self.current);
        self.pre.push_back(line);
    }

    pub fn new_line(&mut self, position: usize) {
        let mut line = self.current.split_off(position);
        std::mem::swap(&mut line, &mut self.current);
        self.pre.push_back(line);
    }
    
    // Merges the current line with the previous line, makes that the current line, and returns the
    // length of the previous line (i.e. where the cursor should be)
    pub fn merge_line_to_prev(&mut self) -> Option<usize> {
        self.pre.pop_back().map(|prev_line| {
            self.current.insert_str(0, &prev_line);
            prev_line.len()
        })
    }
    
    pub fn current_line_len(&self) -> usize {
        self.current.len()
    }
}

