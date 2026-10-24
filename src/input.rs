#[derive(Debug)]
pub struct Editor {
    pub lines: Vec<String>,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub visible: bool,
    pub history: Vec<String>,
    pub history_idx: Option<usize>,
    pub scroll: usize,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            cursor_row: 0,
            cursor_col: 0,
            visible: false,
            history: Vec::new(),
            history_idx: None,
            scroll: 0,
        }
    }

    pub fn open(&mut self) {
        self.lines = vec![String::new()];
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.history_idx = None;
        self.scroll = 0;
        self.visible = true;
    }

    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn insert_char(&mut self, c: char) {
        let mut chars: Vec<char> = self.lines[self.cursor_row].chars().collect();
        chars.insert(self.cursor_col.min(chars.len()), c);
        self.lines[self.cursor_row] = chars.into_iter().collect();
        self.cursor_col += 1;
        self.history_idx = None;
    }

    pub fn newline(&mut self) {
        let chars: Vec<char> = self.lines[self.cursor_row].chars().collect();
        let at = self.cursor_col.min(chars.len());
        let head: String = chars[..at].iter().collect();
        let tail: String = chars[at..].iter().collect();
        self.lines[self.cursor_row] = head;
        self.lines.insert(self.cursor_row + 1, tail);
        self.cursor_row += 1;
        self.cursor_col = 0;
        self.history_idx = None;
    }

    pub fn backspace(&mut self) {
        if self.cursor_col > 0 {
            let mut chars: Vec<char> = self.lines[self.cursor_row].chars().collect();
            chars.remove(self.cursor_col - 1);
            self.lines[self.cursor_row] = chars.into_iter().collect();
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            let cur = self.lines.remove(self.cursor_row);
            self.cursor_row -= 1;
            self.cursor_col = self.lines[self.cursor_row].chars().count();
            self.lines[self.cursor_row].push_str(&cur);
        }
        self.history_idx = None;
    }

    pub fn delete_char(&mut self) {
        let len = self.lines[self.cursor_row].chars().count();
        if self.cursor_col < len {
            let mut chars: Vec<char> = self.lines[self.cursor_row].chars().collect();
            chars.remove(self.cursor_col);
            self.lines[self.cursor_row] = chars.into_iter().collect();
        } else if self.cursor_row + 1 < self.lines.len() {
            let next = self.lines.remove(self.cursor_row + 1);
            self.lines[self.cursor_row].push_str(&next);
        }
        self.history_idx = None;
    }

    pub fn move_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.lines[self.cursor_row].chars().count();
        }
    }

    pub fn move_right(&mut self) {
        let len = self.lines[self.cursor_row].chars().count();
        if self.cursor_col < len {
            self.cursor_col += 1;
        } else if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            self.cursor_col = 0;
        }
    }

    pub fn move_home(&mut self) {
        self.cursor_col = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor_col = self.lines[self.cursor_row].chars().count();
    }

    pub fn move_up(&mut self) {
        if self.cursor_row > 0 {
            self.cursor_row -= 1;
            let len = self.lines[self.cursor_row].chars().count();
            if self.cursor_col > len {
                self.cursor_col = len;
            }
        } else {
            self.recall_older();
        }
    }

    pub fn move_down(&mut self) {
        if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            let len = self.lines[self.cursor_row].chars().count();
            if self.cursor_col > len {
                self.cursor_col = len;
            }
        } else {
            self.recall_newer();
        }
    }

    fn recall_older(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let idx = match self.history_idx {
            Some(i) => i.saturating_sub(1),
            None => self.history.len() - 1,
        };
        self.history_idx = Some(idx);
        self.set_text(self.history[idx].clone());
    }

    fn recall_newer(&mut self) {
        if let Some(i) = self.history_idx {
            if i + 1 < self.history.len() {
                self.history_idx = Some(i + 1);
                let text = self.history[i + 1].clone();
                self.set_text(text);
            } else {
                self.history_idx = None;
                self.set_text(String::new());
            }
        }
    }

    pub fn set_text(&mut self, text: String) {
        self.lines = if text.is_empty() {
            vec![String::new()]
        } else {
            text.split('\n').map(|s| s.to_string()).collect()
        };
        self.cursor_row = self.lines.len() - 1;
        self.cursor_col = self.lines[self.cursor_row].chars().count();
    }

    pub fn push_history(&mut self, sql: String) {
        if self.history.last() != Some(&sql) {
            self.history.push(sql);
        }
    }
}