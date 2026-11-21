// Syntax highlighting - placeholder for tree-sitter integration later
use crate::editor::core::Buffer;
use ratatui::style::{Color, Style};

pub struct Highlighter;

impl Highlighter {
    pub fn new() -> Self {
        Self
    }

    pub fn highlight_line(&self, _buffer: &Buffer, _row: usize, line: &str) -> Vec<(String, Style)> {
        // For now, return the whole line as one span
        vec![(line.to_string(), Style::default().fg(Color::White))]
    }
}