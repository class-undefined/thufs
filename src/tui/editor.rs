use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// A grapheme-aware single-line editor. The byte cursor is always a grapheme boundary.
#[derive(Debug, Clone, Default)]
pub struct Editor {
    pub text: String,
    cursor: usize,
}

impl Editor {
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let cursor = text.len();
        Self { text, cursor }
    }

    pub fn insert(&mut self, value: &str) {
        let value: String = value.chars().filter(|ch| !ch.is_control()).collect();
        self.text.insert_str(self.cursor, &value);
        self.cursor += value.len();
        // A combining mark may have merged with the adjacent grapheme.
        self.cursor = self
            .text
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .find(|&i| i >= self.cursor)
            .unwrap_or(self.text.len());
    }

    fn previous(&self) -> usize {
        self.text[..self.cursor]
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(i, _)| i)
    }

    fn next(&self) -> usize {
        self.text[self.cursor..]
            .graphemes(true)
            .next()
            .map_or(self.cursor, |s| self.cursor + s.len())
    }

    pub fn handle(&mut self, key: KeyEvent) -> bool {
        let before = self.text.clone();
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('a') => self.cursor = 0,
                KeyCode::Char('e') => self.cursor = self.text.len(),
                KeyCode::Char('u') => {
                    self.text.drain(..self.cursor);
                    self.cursor = 0;
                }
                KeyCode::Char('k') => {
                    self.text.truncate(self.cursor);
                }
                KeyCode::Char('w') => {
                    let end = self.cursor;
                    while self.cursor > 0 && self.text[..self.cursor].ends_with(char::is_whitespace)
                    {
                        self.cursor = self.previous();
                    }
                    while self.cursor > 0
                        && !self.text[..self.cursor].ends_with(char::is_whitespace)
                    {
                        self.cursor = self.previous();
                    }
                    self.text.drain(self.cursor..end);
                }
                _ => {}
            }
        } else {
            match key.code {
                KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::ALT) => {
                    self.insert(&ch.to_string())
                }
                KeyCode::Backspace if self.cursor > 0 => {
                    let previous = self.previous();
                    self.text.drain(previous..self.cursor);
                    self.cursor = previous;
                }
                KeyCode::Delete => {
                    let next = self.next();
                    self.text.drain(self.cursor..next);
                }
                KeyCode::Left => self.cursor = self.previous(),
                KeyCode::Right => self.cursor = self.next(),
                KeyCode::Home => self.cursor = 0,
                KeyCode::End => self.cursor = self.text.len(),
                _ => {}
            }
        }
        before != self.text
    }

    /// Visible suffix around the cursor, in terminal cells rather than bytes.
    pub fn viewport(&self, width: usize, secret: bool) -> (String, u16) {
        let width = width.max(1);
        let graphemes = self.text.grapheme_indices(true).collect::<Vec<_>>();
        let cursor = graphemes
            .iter()
            .take_while(|(i, _)| *i < self.cursor)
            .count();
        let display: Vec<&str> = graphemes
            .iter()
            .map(|(_, s)| {
                if secret {
                    "•"
                } else if s.chars().any(char::is_control) {
                    "�"
                } else {
                    *s
                }
            })
            .collect();
        let mut start = 0;
        let mut cells: usize = display[..cursor]
            .iter()
            .map(|s| UnicodeWidthStr::width(*s))
            .sum();
        while cells >= width && start < cursor {
            cells = cells.saturating_sub(UnicodeWidthStr::width(display[start]));
            start += 1;
        }
        let mut visible = String::new();
        let mut used = 0;
        for s in &display[start..] {
            let size = UnicodeWidthStr::width(*s);
            if used + size > width {
                break;
            }
            visible.push_str(s);
            used += size;
        }
        (visible, cells.min(width - 1) as u16)
    }
}
