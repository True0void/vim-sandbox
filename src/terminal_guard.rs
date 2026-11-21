use crossterm::{
    terminal::{self, LeaveAlternateScreen},
    execute,
};
use std::io::{self, Stdout};

/// RAII guard to ensure terminal is restored on drop (panic or normal exit)
pub struct TerminalGuard {
    raw_mode_enabled: bool,
    alternate_screen: bool,
}

impl TerminalGuard {
    pub fn new() -> io::Result<Self> {
        let mut stdout = io::stdout();
        terminal::enable_raw_mode()?;
        execute!(stdout, terminal::EnterAlternateScreen)?;
        Ok(Self {
            raw_mode_enabled: true,
            alternate_screen: true,
        })
    }

    pub fn disable_raw_mode(&mut self) -> io::Result<()> {
        if self.raw_mode_enabled {
            terminal::disable_raw_mode()?;
            self.raw_mode_enabled = false;
        }
        Ok(())
    }

    pub fn leave_alternate_screen(&mut self) -> io::Result<()> {
        if self.alternate_screen {
            let mut stdout = io::stdout();
            execute!(stdout, LeaveAlternateScreen)?;
            self.alternate_screen = false;
        }
        Ok(())
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = self.leave_alternate_screen();
        let _ = self.disable_raw_mode();
    }
}