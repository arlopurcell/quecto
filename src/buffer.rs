use std::{collections::LinkedList, iter::once, io::{Stdout, Error, Write}, fs::File};

use termion::raw::RawTerminal;

use crate::log;

pub struct Buffer {
    pre: LinkedList<String>,
    current_front: String,
    current_back: String,
    post: LinkedList<String>,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            pre: LinkedList::new(),
            current_front: "".to_string(),
            current_back: "".to_string(),
            post: LinkedList::new(),
        }
    }

    pub fn from_lines<L>(mut lines: L) -> Self
        where L: Iterator<Item = String>,
    {
        let current_front = lines.next().unwrap_or_else(|| "".to_string());
        let post: LinkedList<String> = lines.collect();
        Buffer {
            pre: LinkedList::new(), 
            current_front, 
            current_back: "".to_string(),
            post
        }
    }

    fn merge_current_to_front(&mut self) {
        self.current_front.push_str(&self.current_back);
        self.current_back.clear();
    }

    pub fn up(&mut self) -> bool {
        if let Some(mut line) = self.pre.pop_back() {
            self.merge_current_to_front();
            std::mem::swap(&mut line, &mut self.current_front);
            self.post.push_front(line);
            //self.current = line;
            true
        } else {
            false
        }
    }

    pub fn down(&mut self) -> bool {
        if let Some(mut line) = self.post.pop_front() {
            self.merge_current_to_front();
            std::mem::swap(&mut line, &mut self.current_front);
            self.pre.push_back(line);
            //self.current = line;
            true
        } else {
            false
        }
    }

    fn rebalance_at(&mut self, position: usize) {
        let current_front_len = self.current_front.len();
        match position {
            p if p > current_front_len => {
                let new_back = self.current_back.split_off(position - current_front_len);
                self.current_front.push_str(&self.current_back);
                self.current_back = new_back;
            }
            p if p < current_front_len => {
                let mut new_back = self.current_front.split_off(position);
                new_back.push_str(&self.current_back);
                self.current_back = new_back;
            }
            _ => () // position is already at the end of front
        }
    }

    pub fn insert(&mut self, position: usize, c: char) {
        self.rebalance_at(position);
        self.current_front.push(c);
    }

    pub fn delete(&mut self, position: usize) {
        self.rebalance_at(position + 1);
        self.current_front.pop();
    }

    pub fn new_empty_line(&mut self) {
        self.merge_current_to_front();
        let mut line = "".to_string();
        std::mem::swap(&mut line, &mut self.current_front);
        self.pre.push_back(line);
    }

    pub fn new_line(&mut self, position: usize) {
        self.rebalance_at(position);
        
        let mut holder = "".to_string();
        std::mem::swap(&mut holder, &mut self.current_back);
        std::mem::swap(&mut holder, &mut self.current_front);
        self.pre.push_back(holder)
        
        /*
        self.pre.push_back(self.current_front);
        self.current_front = self.current_back;
        self.current_back = "".to_string();
        */
    }
    
    // Merges the current line with the previous line, makes that the current line, and returns the
    // length of the previous line (i.e. where the cursor should be)
    pub fn merge_line_to_prev(&mut self) -> Option<usize> {
        self.pre.pop_back().map(|prev_line| {
            self.current_front.push_str(&self.current_back);
            std::mem::swap(&mut self.current_front, &mut self.current_back);
            self.current_front = prev_line;

            self.current_front.len()
        })
    }
    
    pub fn current_line_len(&self) -> usize {
        self.current_front.len() + self.current_back.len()
    }

    pub fn render_line(&self, term: &mut RawTerminal<Stdout>, current_line_offset: i32) -> Result<(), Error> {
        match current_line_offset {
            0 => {
                term.write(self.current_front.as_bytes())?;
                term.write(self.current_back.as_bytes())?;
            }
            clo if clo < 0 => {
                if let Some(line) = self.pre.iter().rev().nth((clo * -1) as usize - 1) {
                    term.write(line.as_bytes())?;
                }
            }
            clo => { // must be positive
                if let Some(line) = self.post.iter().nth(clo as usize - 1) {
                    term.write(line.as_bytes())?;
                }
            }
        }
        Ok(())
    }

    pub fn write_to_file(&self, mut file: File) -> Result<(), Error> {
        for line in self.pre.iter() {
            write!(&mut file, "{}\n", line)?;
        }
        write!(&mut file, "{}{}\n", self.current_front, self.current_back)?;
        for line in self.post.iter() {
            write!(&mut file, "{}\n", line)?;
        }
        Ok(())
    }
}

