// Vim keymap definitions - placeholder for future expansion
use crate::editor::core::{Mode, Position};

pub struct Keymap;

impl Keymap {
    pub fn new() -> Self {
        Self
    }

    pub fn handle(&self, _mode: Mode, _key: crossterm::event::KeyEvent) -> Option<Action> {
        None
    }
}

#[derive(Debug, Clone)]
pub enum Action {
    Move(Position),
    EnterMode(Mode),
    Operator(Operator),
    Command(String),
}

#[derive(Debug, Clone)]
pub enum Operator {
    Delete,
    Yank,
    Change,
}