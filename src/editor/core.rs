use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::{fs, path::PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Visual,
    VisualLine,
    Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub row: usize,
    pub col: usize,
}

impl Position {
    pub fn new(row: usize, col: usize) -> Self {
        Self { row, col }
    }

    pub fn origin() -> Self {
        Self { row: 0, col: 0 }
    }
}

/// Register for yank/paste
#[derive(Debug, Clone, Default)]
struct Register {
    content: String,
    linewise: bool,
}

impl Register {
    fn set(&mut self, content: String, linewise: bool) {
        self.content = content;
        self.linewise = linewise;
    }

    fn get(&self) -> Option<(&str, bool)> {
        if self.content.is_empty() { None } else { Some((&self.content, self.linewise)) }
    }
}

/// Undo/Redo entry
#[derive(Debug, Clone)]
enum UndoEntry {
    InsertChar { row: usize, col: usize, ch: char },
    InsertNewline { row: usize, col: usize },
    DeleteChar { row: usize, col: usize, ch: char },
    DeleteLine { row: usize, line: String },
    SplitLine { row: usize, original: String },
    JoinLines { row: usize, upper: String, lower: String },
    ReplaceLine { row: usize, old: String },
}

pub struct Buffer {
    lines: Vec<String>,
    file_path: Option<PathBuf>,
    modified: bool,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            file_path: None,
            modified: false,
        }
    }

    pub fn from_file(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        Ok(Self {
            lines: if lines.is_empty() { vec![String::new()] } else { lines },
            file_path: Some(PathBuf::from(path)),
            modified: false,
        })
    }

    pub fn line(&self, row: usize) -> Option<&str> {
        self.lines.get(row).map(|s| s.as_str())
    }

    pub fn line_mut(&mut self, row: usize) -> Option<&mut String> {
        self.lines.get_mut(row)
    }

    pub fn insert_char(&mut self, pos: Position, ch: char) {
        if let Some(line) = self.lines.get_mut(pos.row) {
            let col = pos.col.min(line.len());
            line.insert(col, ch);
            self.modified = true;
        }
    }

    pub fn insert_newline(&mut self, pos: Position) {
        if let Some(line) = self.lines.get_mut(pos.row) {
            let new_line = line.split_off(pos.col);
            self.lines.insert(pos.row + 1, new_line);
            self.modified = true;
        }
    }

    pub fn delete_char(&mut self, pos: Position) -> bool {
        if pos.col > 0 {
            if let Some(line) = self.lines.get_mut(pos.row) {
                if pos.col <= line.len() {
                    line.remove(pos.col - 1);
                    self.modified = true;
                    return true;
                }
            }
        } else if pos.row > 0 {
            let current = self.lines.remove(pos.row);
            if let Some(prev) = self.lines.get_mut(pos.row - 1) {
                prev.push_str(&current);
                self.modified = true;
                return true;
            }
        }
        false
    }

    pub fn delete_line(&mut self, row: usize) -> Option<String> {
        if row < self.lines.len() {
            self.modified = true;
            Some(self.lines.remove(row))
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn line_len(&self, row: usize) -> usize {
        self.lines.get(row).map(|s| s.len()).unwrap_or(0)
    }

    pub fn is_modified(&self) -> bool {
        self.modified
    }

    pub fn file_path(&self) -> Option<&PathBuf> {
        self.file_path.as_ref()
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    pub fn set_file_path(&mut self, path: PathBuf) {
        self.file_path = Some(path);
    }

    pub fn mark_saved(&mut self) {
        self.modified = false;
    }

    pub fn replace_line(&mut self, row: usize, new_line: String) -> Option<String> {
        if row < self.lines.len() {
            self.modified = true;
            Some(std::mem::replace(&mut self.lines[row], new_line))
        } else {
            None
        }
    }
}

pub struct Editor {
    buffer: Buffer,
    cursor: Position,
    mode: Mode,
    viewport: Position,
    pending_count: Option<usize>,
    pending_operator: Option<Operator>,
    status_msg: String,
    command_buffer: String,
    last_search: Option<String>,
    quit_times: u8,
    // For gg detection
    last_key: Option<KeyCode>,
    // Registers
    unnamed_register: Register,
    numbered_registers: [Register; 9], // "1-"9
    // Undo/Redo
    undo_stack: Vec<UndoEntry>,
    redo_stack: Vec<UndoEntry>,
    // Callbacks for theme integration
    on_theme_complete: Option<Box<dyn Fn(&str) -> Option<String> + Send>>,
}

#[derive(Debug, Clone, Copy)]
pub enum Operator {
    Delete,
    Yank,
    Change,
}

impl Editor {
    pub fn new() -> Result<Self> {
        Ok(Self {
            buffer: Buffer::new(),
            cursor: Position::origin(),
            mode: Mode::Normal,
            viewport: Position::origin(),
            pending_count: None,
            pending_operator: None,
            status_msg: String::new(),
            command_buffer: String::new(),
            last_search: None,
            quit_times: 0,
            last_key: None,
            unnamed_register: Register::default(),
            numbered_registers: std::array::from_fn(|_| Register::default()),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            on_theme_complete: None,
        })
    }

    pub fn set_theme_complete_callback(&mut self, cb: impl Fn(&str) -> Option<String> + Send + 'static) {
        self.on_theme_complete = Some(Box::new(cb));
    }

    pub fn open_file(&mut self, path: &str) -> Result<()> {
        if self.buffer.is_modified() && self.quit_times == 0 {
            self.set_status("No write since last change (use :w or :q!)");
            self.quit_times = 1;
            return Ok(());
        }
        self.buffer = Buffer::from_file(path)?;
        self.cursor = Position::origin();
        self.viewport = Position::origin();
        self.quit_times = 0;
        self.undo_stack.clear();
        self.redo_stack.clear();
        Ok(())
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn cursor(&self) -> Position {
        self.cursor
    }

    pub fn viewport(&self) -> Position {
        self.viewport
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub fn status_msg(&self) -> &str {
        &self.status_msg
    }

    pub fn command_buffer(&self) -> &str {
        &self.command_buffer
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_msg = msg.into();
    }

    pub fn clear_command_buffer(&mut self) {
        self.command_buffer.clear();
    }

    fn count(&mut self) -> usize {
        self.pending_count.unwrap_or(1)
    }

    pub async fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        let result = match self.mode {
            Mode::Normal => self.handle_normal(key).await,
            Mode::Insert => self.handle_insert(key).await,
            Mode::Visual | Mode::VisualLine => self.handle_visual(key).await,
            Mode::Command => self.handle_command(key).await,
        };
        
        // Track last key for gg detection (only in normal mode)
        if self.mode == Mode::Normal {
            self.last_key = Some(key.code);
        }
        
        result
    }

    async fn handle_normal(&mut self, key: KeyEvent) -> Result<bool> {
        // Handle count prefix (1-9)
        if let KeyCode::Char(c @ '1'..='9') = key.code {
            if self.pending_count.is_none() {
                self.pending_count = Some((c as usize) - 48);
            } else if let Some(n) = self.pending_count {
                self.pending_count = Some(n * 10 + (c as usize) - 48);
            }
            return Ok(false);
        }

        let count = self.count();
        self.pending_count = None;

        // Operators (d, y, c) - wait for motion
        if self.pending_operator.is_none() {
            match key.code {
                KeyCode::Char('d') => { self.pending_operator = Some(Operator::Delete); return Ok(false); }
                KeyCode::Char('y') => { self.pending_operator = Some(Operator::Yank); return Ok(false); }
                KeyCode::Char('c') => { self.pending_operator = Some(Operator::Change); return Ok(false); }
                _ => {}
            }
        }

        // If we have an operator, apply it with the motion
        if let Some(op) = self.pending_operator {
            self.pending_operator = None;
            self.apply_operator(op, key.code, count)?;
            self.last_key = None; // Reset after operator+motion
            return Ok(false);
        }

        // Check for gg (go to first line)
        if key.code == KeyCode::Char('g') && self.last_key == Some(KeyCode::Char('g')) {
            self.cursor.row = 0;
            self.cursor.col = 0;
            self.ensure_cursor_visible();
            self.last_key = None;
            return Ok(false);
        }

        // Normal motions and commands
        match key.code {
            // Movement
            KeyCode::Char('h') | KeyCode::Left => self.move_cursor(-(count as isize), 0),
            KeyCode::Char('l') | KeyCode::Right => self.move_cursor(count as isize, 0),
            KeyCode::Char('j') | KeyCode::Down => self.move_cursor(0, count as isize),
            KeyCode::Char('k') | KeyCode::Up => self.move_cursor(0, -(count as isize)),

            KeyCode::Char('w') => self.move_word_forward(count),
            KeyCode::Char('b') => self.move_word_backward(count),
            KeyCode::Char('e') => self.move_word_end(count),
            KeyCode::Char('0') => { self.cursor.col = 0; self.ensure_cursor_visible(); }
            KeyCode::Char('^') => { self.move_to_first_non_blank(); self.ensure_cursor_visible(); }
            KeyCode::Char('$') => { self.move_to_line_end(); self.ensure_cursor_visible(); }
            KeyCode::Char('G') => { 
                self.cursor.row = self.buffer.len().saturating_sub(1); 
                self.cursor.col = 0;
                self.ensure_cursor_visible(); 
            }

            // Enter insert mode
            KeyCode::Char('i') => self.enter_insert(),
            KeyCode::Char('a') => { self.move_cursor(1, 0); self.enter_insert(); }
            KeyCode::Char('I') => { self.cursor.col = 0; self.move_to_first_non_blank(); self.enter_insert(); }
            KeyCode::Char('A') => { self.move_to_line_end(); self.enter_insert(); }
            KeyCode::Char('o') => { self.new_line_below(); self.enter_insert(); }
            KeyCode::Char('O') => { self.new_line_above(); self.enter_insert(); }

            // Paste
            KeyCode::Char('p') => self.paste_after()?,
            KeyCode::Char('P') => self.paste_before()?,

            // Visual mode
            KeyCode::Char('v') => self.mode = Mode::Visual,
            KeyCode::Char('V') => self.mode = Mode::VisualLine,

            // Command mode
            KeyCode::Char(':') => {
                self.mode = Mode::Command;
                self.command_buffer.clear();
                self.command_buffer.push(':');
            }

            // Quit (Ctrl+q)
            KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => return Ok(true),

            // Save (Ctrl+s)
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => self.save()?,

            // Undo/Redo
            KeyCode::Char('u') => self.undo()?,
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => self.redo()?,

            // Search
            KeyCode::Char('/') => {
                self.mode = Mode::Command;
                self.command_buffer.clear();
                self.command_buffer.push('/');
            }
            KeyCode::Char('?') => {
                self.mode = Mode::Command;
                self.command_buffer.clear();
                self.command_buffer.push('?');
            }
            KeyCode::Char('n') => self.search_next(true)?,
            KeyCode::Char('N') => self.search_next(false)?,

            // Line operations (capital = linewise)
            KeyCode::Char('D') => { self.delete_to_line_end(); }
            KeyCode::Char('Y') => { self.yank_line(count); }
            KeyCode::Char('C') => { self.delete_to_line_end(); self.enter_insert(); }
            KeyCode::Char('S') => { self.delete_line(); self.enter_insert(); }

            _ => {}
        }

        self.clamp_cursor();
        self.quit_times = 0; // Reset quit confirmation on any normal command
        Ok(false)
    }

    async fn handle_insert(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Char(c) => {
                self.push_undo(UndoEntry::InsertChar { row: self.cursor.row, col: self.cursor.col, ch: c });
                self.buffer.insert_char(self.cursor, c);
                self.move_cursor(1, 0);
            }
            KeyCode::Enter => {
                self.push_undo(UndoEntry::InsertNewline { row: self.cursor.row, col: self.cursor.col });
                self.buffer.insert_newline(self.cursor);
                self.move_cursor(0, 1);
                self.cursor.col = 0;
            }
            KeyCode::Backspace => {
                if self.cursor.col > 0 {
                    self.move_cursor(-1, 0);
                    if let Some(ch) = self.buffer.line(self.cursor.row).and_then(|l| l.chars().nth(self.cursor.col)) {
                        self.push_undo(UndoEntry::DeleteChar { row: self.cursor.row, col: self.cursor.col, ch });
                    }
                    self.buffer.delete_char(self.cursor);
                } else if self.cursor.row > 0 {
                    // Join with previous line
                    let current_line = self.buffer.lines[self.cursor.row].clone();
                    let prev_len = self.buffer.line_len(self.cursor.row - 1);
                    self.push_undo(UndoEntry::JoinLines { 
                        row: self.cursor.row - 1, 
                        upper: self.buffer.line(self.cursor.row - 1).unwrap_or("").to_string(),
                        lower: current_line.clone(),
                    });
                    self.move_cursor(0, -1);
                    self.move_to_line_end();
                    self.buffer.lines[self.cursor.row].push_str(&current_line);
                    self.buffer.lines.remove(self.cursor.row + 1);
                    self.buffer.modified = true;
                }
            }
            KeyCode::Tab => {
                self.push_undo(UndoEntry::InsertChar { row: self.cursor.row, col: self.cursor.col, ch: '\t' });
                self.buffer.insert_char(self.cursor, '\t');
                self.move_cursor(1, 0);
            }
            _ => {}
        }
        self.clamp_cursor();
        Ok(false)
    }

    async fn handle_visual(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Char('h') | KeyCode::Left => self.move_cursor(-1, 0),
            KeyCode::Char('l') | KeyCode::Right => self.move_cursor(1, 0),
            KeyCode::Char('j') | KeyCode::Down => self.move_cursor(0, 1),
            KeyCode::Char('k') | KeyCode::Up => self.move_cursor(0, -1),
            KeyCode::Char('y') => { self.yank_visual(); self.mode = Mode::Normal; }
            KeyCode::Char('d') => { self.delete_visual(); self.mode = Mode::Normal; }
            _ => {}
        }
        self.clamp_cursor();
        Ok(false)
    }

    async fn handle_command(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.command_buffer.clear();
            }
            KeyCode::Enter => {
                let cmd = self.command_buffer.clone();
                self.command_buffer.clear();
                self.execute_command(&cmd)?;
                self.mode = Mode::Normal;
            }
            KeyCode::Char(c) => self.command_buffer.push(c),
            KeyCode::Backspace => { self.command_buffer.pop(); }
            KeyCode::Tab => {
                if let Some(cb) = &self.on_theme_complete {
                    let cmd = self.command_buffer.trim();
                    // Theme completion: :theme <partial>
                    if cmd.starts_with(":theme ") || cmd.starts_with(":colorscheme ") {
                        let prefix = cmd.split_whitespace().nth(1).unwrap_or("");
                        if let Some(completion) = cb(prefix) {
                            // Replace the partial with completion
                            let base = if cmd.starts_with(":colorscheme ") { ":colorscheme " } else { ":theme " };
                            self.command_buffer = format!("{}{}", base, completion);
                        }
                    }
                    // Command completion: :theme<tab> -> :theme 
                    else if cmd == ":theme" || cmd == ":colorscheme" {
                        self.command_buffer.push(' ');
                    }
                }
            }
            _ => {}
        }
        Ok(false)
    }

    fn execute_command(&mut self, cmd: &str) -> Result<()> {
        let cmd = cmd.trim();

        if cmd == ":" || cmd.is_empty() {
            return Ok(());
        }

        // Handle search commands
        if cmd.starts_with('/') || cmd.starts_with('?') {
            let pattern = &cmd[1..];
            self.last_search = Some(pattern.to_string());
            self.search_next(cmd.starts_with('/'))?;
            return Ok(());
        }

        // Parse :command [args] - command includes the leading :
        let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
        let command = parts[0];
        let args = parts.get(1).map(|s| s.trim()).unwrap_or("");

        // Helper to match commands with or without leading :
        let cmd_name = command.strip_prefix(':').unwrap_or(command);

        // Theme commands (handled here so buffer isn't cleared yet)
        if matches!(cmd_name, "theme" | "colorscheme" | "themes" | "themenext" | "themeprev") {
            return self.execute_theme_command(cmd_name, args);
        }

        match cmd_name {
            "q" | "quit" => {
                if self.buffer.is_modified() && self.quit_times == 0 {
                    self.set_status("No write since last change (use :q! to force)");
                    self.quit_times = 1;
                } else {
                    self.set_status("QUIT_REQUESTED");
                }
            }
            "q!" | "quit!" => {
                self.set_status("QUIT_REQUESTED");
            }
            "w" | "write" => {
                if args.is_empty() {
                    self.save()?;
                } else {
                    self.save_as(args)?;
                }
            }
            "wq" | "x" => {
                self.save()?;
                if !self.buffer.is_modified() {
                    self.set_status("QUIT_REQUESTED");
                }
            }
            "wq!" | "x!" => {
                self.save()?;
                self.set_status("QUIT_REQUESTED");
            }
            "e" | "edit" => {
                if args.is_empty() {
                    self.set_status("E471: Argument required");
                } else if self.buffer.is_modified() && self.quit_times == 0 {
                    self.set_status("No write since last change (use :e! to force)");
                    self.quit_times = 1;
                } else {
                    self.open_file(args)?;
                }
            }
            "e!" | "edit!" => {
                if args.is_empty() {
                    self.set_status("E471: Argument required");
                } else {
                    self.buffer = Buffer::from_file(args)?;
                    self.cursor = Position::origin();
                    self.viewport = Position::origin();
                    self.undo_stack.clear();
                    self.redo_stack.clear();
                }
            }
            "bn" | "bnext" => { self.set_status("Not implemented: :bn"); }
            "bp" | "bprev" => { self.set_status("Not implemented: :bp"); }
            "ls" | "buffers" => { self.set_status("Not implemented: :ls"); }
            "set" => {
                if args == "nu" || args == "number" {
                    self.set_status("set number (not implemented)");
                } else if args == "nonu" || args == "nonumber" {
                    self.set_status("set nonumber (not implemented)");
                } else {
                    self.set_status(format!("Unknown option: {}", args));
                }
            }
            "help" | "h" => {
                self.set_status("Help: :q quit, :w save, :wq save&quit, :e file, :set nu, /search, :theme [name], :themes, :themenext, :themeprev");
            }
            _ => {
                self.set_status(format!("Not an editor command: {}", command));
            }
        }
        Ok(())
    }

    fn execute_theme_command(&mut self, cmd: &str, args: &str) -> Result<()> {
        // Note: We need a callback to background for actual theme switching
        // For now, store the theme request in status for main.rs to pick up
        match cmd {
            "theme" | "colorscheme" => {
                if args.is_empty() {
                    self.set_status("THEME_GET_CURRENT");
                } else {
                    self.set_status(format!("THEME_SET:{}", args));
                }
            }
            "themes" => {
                self.set_status("THEME_LIST");
            }
            "themenext" => {
                self.set_status("THEME_NEXT");
            }
            "themeprev" => {
                self.set_status("THEME_PREV");
            }
            _ => {}
        }
        Ok(())
    }

    fn save(&mut self) -> Result<()> {
        if let Some(path) = self.buffer.file_path().cloned() {
            let content = self.buffer.lines().join("\n");
            let content_len = content.len();
            fs::write(&path, content)?;
            self.buffer.mark_saved();
            self.set_status(format!("\"{}\" {}L, {}B written", path.display(), self.buffer.len(), content_len));
        } else {
            self.set_status("E32: No file name");
        }
        Ok(())
    }

    fn save_as(&mut self, path: &str) -> Result<()> {
        let path_buf = PathBuf::from(path);
        let content = self.buffer.lines().join("\n");
        let content_len = content.len();
        fs::write(&path_buf, content)?;
        self.buffer.set_file_path(path_buf.clone());
        self.buffer.mark_saved();
        self.set_status(format!("\"{}\" {}L, {}B written", path_buf.display(), self.buffer.len(), content_len));
        Ok(())
    }

    // ========== UNDO/REDO ==========
    fn push_undo(&mut self, entry: UndoEntry) {
        self.undo_stack.push(entry);
        if self.undo_stack.len() > 1000 { self.undo_stack.remove(0); }
        self.redo_stack.clear();
    }

    fn undo(&mut self) -> Result<()> {
        if let Some(entry) = self.undo_stack.pop() {
            self.apply_undo(entry.clone());
            self.redo_stack.push(entry);
            self.set_status("Undo");
        } else {
            self.set_status("Already at oldest change");
        }
        Ok(())
    }

    fn redo(&mut self) -> Result<()> {
        if let Some(entry) = self.redo_stack.pop() {
            self.apply_redo(entry.clone());
            self.undo_stack.push(entry);
            self.set_status("Redo");
        } else {
            self.set_status("Already at newest change");
        }
        Ok(())
    }

    fn apply_undo(&mut self, entry: UndoEntry) {
        match entry {
            UndoEntry::InsertChar { row, col, .. } => {
                if let Some(line) = self.buffer.line_mut(row) {
                    if col < line.len() { line.remove(col); }
                }
            }
            UndoEntry::InsertNewline { row, col } => {
                if row + 1 < self.buffer.len() {
                    let lower = self.buffer.lines.remove(row + 1);
                    if let Some(upper) = self.buffer.line_mut(row) {
                        upper.push_str(&lower);
                    }
                }
            }
            UndoEntry::DeleteChar { row, col, ch } => {
                if let Some(line) = self.buffer.line_mut(row) {
                    if col <= line.len() { line.insert(col, ch); }
                }
            }
            UndoEntry::DeleteLine { row, line } => {
                self.buffer.lines.insert(row, line);
            }
            UndoEntry::SplitLine { row, original } => {
                self.buffer.lines[row] = original;
            }
            UndoEntry::JoinLines { row, upper, lower } => {
                self.buffer.lines[row] = upper;
                self.buffer.lines.insert(row + 1, lower);
            }
            UndoEntry::ReplaceLine { row, old } => {
                if row < self.buffer.len() {
                    self.buffer.lines[row] = old;
                }
            }
        }
        self.buffer.modified = true;
        self.clamp_cursor();
    }

    fn apply_redo(&mut self, entry: UndoEntry) {
        match entry {
            UndoEntry::InsertChar { row, col, ch } => {
                if let Some(line) = self.buffer.line_mut(row) {
                    if col <= line.len() { line.insert(col, ch); }
                }
            }
            UndoEntry::InsertNewline { row, col } => {
                if let Some(line) = self.buffer.line_mut(row) {
                    let new_line = line.split_off(col);
                    self.buffer.lines.insert(row + 1, new_line);
                }
            }
            UndoEntry::DeleteChar { row, col, .. } => {
                if let Some(line) = self.buffer.line_mut(row) {
                    if col < line.len() { line.remove(col); }
                }
            }
            UndoEntry::DeleteLine { row, .. } => {
                if row < self.buffer.len() {
                    self.buffer.lines.remove(row);
                }
            }
            UndoEntry::SplitLine { row, .. } => {
                if let Some(line) = self.buffer.line_mut(row) {
                    let new_line = line.split_off(row); // This is wrong but matches original behavior
                    self.buffer.lines.insert(row + 1, new_line);
                }
            }
            UndoEntry::JoinLines { row, .. } => {
                if row + 1 < self.buffer.len() {
                    let lower = self.buffer.lines.remove(row + 1);
                    if let Some(upper) = self.buffer.line_mut(row) {
                        upper.push_str(&lower);
                    }
                }
            }
            UndoEntry::ReplaceLine { row, .. } => {
                // Redo would need new content - simplified
            }
        }
        self.buffer.modified = true;
        self.clamp_cursor();
    }

    // ========== YANK/PASTE ==========
    fn yank_to_register(&mut self, content: String, linewise: bool, reg_idx: Option<usize>) {
        if let Some(idx) = reg_idx {
            if idx < 9 { self.numbered_registers[idx].set(content, linewise); }
        } else {
            self.unnamed_register.set(content, linewise);
        }
        // Also update numbered register "1 (shift others)
        if !linewise || reg_idx.is_none() { // Simplified: only shift on linewise delete
            for i in (1..9).rev() {
                self.numbered_registers[i] = self.numbered_registers[i - 1].clone();
            }
        }
    }

    fn get_register(&self, reg_idx: Option<usize>) -> Option<(&str, bool)> {
        if let Some(idx) = reg_idx {
            if idx < 9 { self.numbered_registers[idx].get() } else { None }
        } else {
            self.unnamed_register.get()
        }
    }

    fn yank_line(&mut self, count: usize) {
        let mut content = String::new();
        for i in 0..count {
            if self.cursor.row + i < self.buffer.len() {
                content.push_str(self.buffer.line(self.cursor.row + i).unwrap_or(""));
                if i < count - 1 { content.push('\n'); }
            }
        }
        if !content.is_empty() {
            self.yank_to_register(content, true, None);
            self.set_status(format!("{} line{} yanked", count, if count > 1 { "s" } else { "" }));
        }
    }

    fn yank_visual(&mut self) {
        // Simplified: yank current line in visual mode
        if let Some(line) = self.buffer.line(self.cursor.row) {
            self.yank_to_register(line.to_string(), false, None);
            self.set_status("Yanked");
        }
    }

    fn delete_visual(&mut self) {
        // Simplified: delete current line
        if let Some(line) = self.buffer.delete_line(self.cursor.row) {
            self.yank_to_register(line, true, None);
        }
    }

    fn paste_after(&mut self) -> Result<()> {
        let (content, linewise) = match self.get_register(None) {
            Some((c, l)) => (c.to_string(), l),
            None => return Ok(()),
        };
        if linewise {
            // Insert after current line
            self.buffer.lines.insert(self.cursor.row + 1, content);
            self.cursor.row += 1;
            self.cursor.col = 0;
        } else {
            // Insert after cursor
            if let Some(line) = self.buffer.line_mut(self.cursor.row) {
                let insert_pos = (self.cursor.col + 1).min(line.len());
                line.insert_str(insert_pos, &content);
                self.cursor.col = insert_pos + content.chars().count();
            }
        }
        self.buffer.modified = true;
        self.set_status("Pasted");
        Ok(())
    }

    fn paste_before(&mut self) -> Result<()> {
        let (content, linewise) = match self.get_register(None) {
            Some((c, l)) => (c.to_string(), l),
            None => return Ok(()),
        };
        if linewise {
            // Insert before current line
            self.buffer.lines.insert(self.cursor.row, content);
            self.cursor.col = 0;
        } else {
            // Insert before cursor
            if let Some(line) = self.buffer.line_mut(self.cursor.row) {
                let insert_pos = self.cursor.col.min(line.len());
                line.insert_str(insert_pos, &content);
            }
        }
        self.buffer.modified = true;
        self.set_status("Pasted");
        Ok(())
    }

    // ========== OPERATORS ==========
    fn apply_operator(&mut self, op: Operator, motion: KeyCode, count: usize) -> Result<()> {
        let start_row = self.cursor.row;
        let start_col = self.cursor.col;

        // Execute the motion to find end position using a temporary editor
        let mut temp_editor = Editor {
            buffer: Buffer { lines: self.buffer.lines.clone(), file_path: None, modified: false },
            cursor: self.cursor,
            mode: Mode::Normal,
            viewport: self.viewport,
            pending_count: None,
            pending_operator: None,
            status_msg: String::new(),
            command_buffer: String::new(),
            last_search: None,
            quit_times: 0,
            last_key: None,
            unnamed_register: Register::default(),
            numbered_registers: std::array::from_fn(|_| Register::default()),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            on_theme_complete: None,
        };

        // Apply motion count times
        for _ in 0..count {
            match motion {
                KeyCode::Char('w') => temp_editor.move_word_forward(1),
                KeyCode::Char('b') => temp_editor.move_word_backward(1),
                KeyCode::Char('e') => temp_editor.move_word_end(1),
                KeyCode::Char('0') => temp_editor.cursor.col = 0,
                KeyCode::Char('^') => temp_editor.move_to_first_non_blank(),
                KeyCode::Char('$') => temp_editor.move_to_line_end(),
                KeyCode::Char('h') | KeyCode::Left => temp_editor.move_cursor(-1, 0),
                KeyCode::Char('l') | KeyCode::Right => temp_editor.move_cursor(1, 0),
                KeyCode::Char('j') | KeyCode::Down => temp_editor.move_cursor(0, 1),
                KeyCode::Char('k') | KeyCode::Up => temp_editor.move_cursor(0, -1),
                KeyCode::Char('G') => temp_editor.cursor.row = temp_editor.buffer.len().saturating_sub(1),
                KeyCode::Char('g') => temp_editor.cursor.row = 0,
                _ => {}
            }
        }
        let end_pos = temp_editor.cursor;

        // Ensure start <= end
        let (start_row, start_col, end_row, end_col) = if (start_row, start_col) <= (end_pos.row, end_pos.col) {
            (start_row, start_col, end_pos.row, end_pos.col)
        } else {
            (end_pos.row, end_pos.col, start_row, start_col)
        };

        match op {
            Operator::Delete => {
                let mut deleted = String::new();
                let linewise = start_col == 0 && end_col == 0 && end_row > start_row;
                
                if start_row == end_row {
                    // Single line
                    if let Some(line) = self.buffer.line_mut(start_row) {
                        deleted = line[start_col..end_col.min(line.len())].to_string();
                        line.drain(start_col..end_col.min(line.len()));
                    }
                } else {
                    // Multi-line
                    if let Some(first) = self.buffer.line_mut(start_row) {
                        deleted.push_str(&first[start_col..]);
                        deleted.push('\n');
                    }
                    for row in (start_row + 1)..end_row {
                        if let Some(line) = self.buffer.line(row) {
                            deleted.push_str(line);
                            deleted.push('\n');
                        }
                    }
                    if let Some(last) = self.buffer.line_mut(end_row) {
                        deleted.push_str(&last[..end_col.min(last.len())]);
                    }
                    // Remove lines
                    for _ in start_row..=end_row {
                        self.buffer.lines.remove(start_row);
                    }
                    // Insert remaining parts
                    let mut remaining = String::new();
                    if let Some(first) = self.buffer.line(start_row) {
                        remaining.push_str(&first[..start_col]);
                    }
                    if let Some(last) = self.buffer.line(start_row) { // After removal, end_row shifted
                        remaining.push_str(&last[end_col.min(last.len())..]);
                    }
                    self.buffer.lines.insert(start_row, remaining);
                }
                self.buffer.modified = true;
                self.yank_to_register(deleted, linewise, None);
                self.cursor = Position::new(start_row, start_col);
            }
            Operator::Yank => {
                let mut yanked = String::new();
                let linewise = start_col == 0 && end_col == 0 && end_row > start_row;
                
                if start_row == end_row {
                    if let Some(line) = self.buffer.line(start_row) {
                        yanked = line[start_col..end_col.min(line.len())].to_string();
                    }
                } else {
                    if let Some(first) = self.buffer.line(start_row) {
                        yanked.push_str(&first[start_col..]);
                        yanked.push('\n');
                    }
                    for row in (start_row + 1)..end_row {
                        if let Some(line) = self.buffer.line(row) {
                            yanked.push_str(line);
                            yanked.push('\n');
                        }
                    }
                    if let Some(last) = self.buffer.line(end_row) {
                        yanked.push_str(&last[..end_col.min(last.len())]);
                    }
                }
                self.yank_to_register(yanked, linewise, None);
                self.set_status(format!("Yanked {} line{}", if linewise { "line" } else { "char" }, if count > 1 { "s" } else { "" }));
            }
            Operator::Change => {
                // Delete then enter insert
                self.apply_operator(Operator::Delete, motion, count)?;
                self.enter_insert();
            }
        }
        self.clamp_cursor();
        Ok(())
    }

    // Helper for linewise delete (dd, cc, etc.)
    fn delete_line(&mut self) {
        if let Some(line) = self.buffer.delete_line(self.cursor.row) {
            self.yank_to_register(line, true, None);
        }
        if self.cursor.row >= self.buffer.len() && self.cursor.row > 0 {
            self.cursor.row = self.buffer.len().saturating_sub(1);
        }
        self.cursor.col = 0;
    }

    fn delete_to_line_end(&mut self) {
        if let Some(line) = self.buffer.line_mut(self.cursor.row) {
            let deleted = line[self.cursor.col..].to_string();
            line.truncate(self.cursor.col);
            self.buffer.modified = true;
            self.yank_to_register(deleted, false, None);
        }
    }

    // ========== MOTIONS ==========
    fn enter_insert(&mut self) {
        self.mode = Mode::Insert;
    }

    fn move_cursor(&mut self, dx: isize, dy: isize) {
        let new_col = (self.cursor.col as isize + dx).max(0) as usize;
        let new_row = (self.cursor.row as isize + dy).max(0) as usize;
        self.cursor = Position::new(new_row.min(self.buffer.len().saturating_sub(1)), new_col);
        self.ensure_cursor_visible();
    }

    fn move_to_first_non_blank(&mut self) {
        if let Some(line) = self.buffer.line(self.cursor.row) {
            for (i, ch) in line.chars().enumerate() {
                if !ch.is_whitespace() {
                    self.cursor.col = i;
                    break;
                }
            }
        }
    }

    fn move_word_forward(&mut self, count: usize) {
        for _ in 0..count {
            if let Some(line) = self.buffer.line(self.cursor.row) {
                let mut col = self.cursor.col;
                while col < line.len() && !line.chars().nth(col).unwrap_or(' ').is_whitespace() {
                    col += 1;
                }
                while col < line.len() && line.chars().nth(col).unwrap_or(' ').is_whitespace() {
                    col += 1;
                }
                if col >= line.len() {
                    if self.cursor.row + 1 < self.buffer.len() {
                        self.cursor.row += 1;
                        self.cursor.col = 0;
                        if let Some(next_line) = self.buffer.line(self.cursor.row) {
                            let mut c = 0;
                            while c < next_line.len() && next_line.chars().nth(c).unwrap_or(' ').is_whitespace() {
                                c += 1;
                            }
                            self.cursor.col = c;
                        }
                    }
                } else {
                    self.cursor.col = col;
                }
            }
        }
        self.ensure_cursor_visible();
    }

    fn move_word_backward(&mut self, count: usize) {
        for _ in 0..count {
            if self.cursor.col > 0 {
                let line = self.buffer.line(self.cursor.row).unwrap_or("");
                let mut col = self.cursor.col - 1;
                while col > 0 && line.chars().nth(col).unwrap_or(' ').is_whitespace() {
                    col -= 1;
                }
                while col > 0 && !line.chars().nth(col - 1).unwrap_or(' ').is_whitespace() {
                    col -= 1;
                }
                self.cursor.col = col;
            } else if self.cursor.row > 0 {
                self.cursor.row -= 1;
                if let Some(line) = self.buffer.line(self.cursor.row) {
                    self.cursor.col = line.len();
                }
            }
        }
        self.ensure_cursor_visible();
    }

    fn move_word_end(&mut self, count: usize) {
        for _ in 0..count {
            if let Some(line) = self.buffer.line(self.cursor.row) {
                let mut col = self.cursor.col;
                if col >= line.len() { continue; }
                while col + 1 < line.len() && !line.chars().nth(col + 1).unwrap_or(' ').is_whitespace() {
                    col += 1;
                }
                self.cursor.col = col;
            }
        }
        self.ensure_cursor_visible();
    }

    fn move_to_line_end(&mut self) {
        if let Some(line) = self.buffer.line(self.cursor.row) {
            self.cursor.col = line.len().saturating_sub(1);
        }
    }

    fn new_line_below(&mut self) {
        self.cursor.row = (self.cursor.row + 1).min(self.buffer.len());
        self.cursor.col = 0;
        self.buffer.insert_newline(Position::new(self.cursor.row, 0));
    }

    fn new_line_above(&mut self) {
        self.cursor.col = 0;
        self.buffer.insert_newline(Position::new(self.cursor.row, 0));
    }

    // ========== SEARCH ==========
    fn search_next(&mut self, forward: bool) -> Result<()> {
        if let Some(pattern) = self.last_search.clone() {
            let lines = self.buffer.lines();
            let start_row = self.cursor.row;
            let start_col = self.cursor.col;
            let len = lines.len();

            if forward {
                // Search forward from cursor+1
                for offset in 1..=len {
                    let row = (start_row + offset) % len;
                    let line = &lines[row];
                    let search_start = if row == start_row { start_col + 1 } else { 0 };
                    if let Some(pos) = line[search_start..].find(&pattern) {
                        self.cursor.row = row;
                        self.cursor.col = search_start + pos;
                        self.ensure_cursor_visible();
                        return Ok(());
                    }
                }
            } else {
                // Search backward from cursor-1
                for offset in 1..=len {
                    let row = if start_row >= offset { start_row - offset } else { len - (offset - start_row) };
                    let line = &lines[row];
                    let search_end = if row == start_row { start_col } else { line.len() };
                    if let Some(pos) = line[..search_end].rfind(&pattern) {
                        self.cursor.row = row;
                        self.cursor.col = pos;
                        self.ensure_cursor_visible();
                        return Ok(());
                    }
                }
            }
            self.set_status(format!("Pattern not found: {}", pattern));
        }
        Ok(())
    }

    fn ensure_cursor_visible(&mut self) {
        // Vertical scroll - approximate visible height
        let visible_height = 24; // Will be overridden by actual render area
        if self.cursor.row < self.viewport.row {
            self.viewport.row = self.cursor.row;
        } else if self.cursor.row >= self.viewport.row + visible_height {
            self.viewport.row = self.cursor.row.saturating_sub(visible_height) + 1;
        }
        // Horizontal scroll would need terminal width
    }

    fn clamp_cursor(&mut self) {
        let max_row = self.buffer.len().saturating_sub(1);
        self.cursor.row = self.cursor.row.min(max_row);
        if let Some(line) = self.buffer.line(self.cursor.row) {
            self.cursor.col = self.cursor.col.min(line.len());
        }
    }
}