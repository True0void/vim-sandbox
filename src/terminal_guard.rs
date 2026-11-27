use crossterm::{
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::{self, Write};

use crate::ui::transparency;

/// RAII guard to ensure terminal is restored on drop (panic or normal exit).
///
/// Lifecycle rules that make the guard safe against partial setup:
/// * every state flag starts at `false`, so a guard that never finished
///   `new()` still has a well-defined `Drop`;
/// * a flag is only set to `true` *after* the corresponding operation
///   succeeded, so we never try to undo something that was never done;
/// * each restore step clears its flag *before* acting and is therefore
///   idempotent: a second call is a no-op, and a failing step never blocks
///   the remaining ones;
/// * [`TerminalGuard::restore`] additionally latches `cleaned_up`, giving a
///   single, unique cleanup pass per guard.
pub struct TerminalGuard {
    raw_mode_enabled: bool,
    alternate_screen: bool,
    transparency_enabled: bool,
    cleaned_up: bool,
}

impl TerminalGuard {
    /// Puts the terminal in raw mode + alternate screen.
    ///
    /// If entering the alternate screen fails, the guard (still a local here)
    /// is dropped while the error propagates, so raw mode is already restored
    /// before the caller sees the `Err`.
    pub fn new() -> io::Result<Self> {
        let mut guard = Self {
            raw_mode_enabled: false,
            alternate_screen: false,
            transparency_enabled: false,
            cleaned_up: false,
        };

        terminal::enable_raw_mode()?;
        guard.raw_mode_enabled = true;

        // On failure the `?` returns early: `guard` goes out of scope and its
        // `Drop` disables raw mode before the error reaches the caller.
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        guard.alternate_screen = true;

        Ok(guard)
    }

    /// Requests a transparent default background (OSC 11).
    ///
    /// Returns `Ok(())` when the terminal does not support transparency; the
    /// flag stays `false` so `Drop` does not emit a pointless OSC 111.
    pub fn enable_transparency(&mut self) -> io::Result<()> {
        if self.transparency_enabled || self.cleaned_up {
            return Ok(());
        }
        let mut stdout = io::stdout();
        if transparency::setup_transparency(&mut stdout)? {
            self.transparency_enabled = true;
        }
        Ok(())
    }

    pub fn disable_raw_mode(&mut self) -> io::Result<()> {
        if !self.raw_mode_enabled {
            return Ok(());
        }
        // Clear the flag first: cleanup happens at most once even if the
        // syscall below fails.
        self.raw_mode_enabled = false;
        terminal::disable_raw_mode()
    }

    pub fn leave_alternate_screen(&mut self) -> io::Result<()> {
        if !self.alternate_screen {
            return Ok(());
        }
        self.alternate_screen = false;
        let mut stdout = io::stdout();
        execute!(stdout, LeaveAlternateScreen)
    }

    /// Undo the OSC 11 transparency setup with the symmetric OSC 111 reset.
    pub fn restore_transparency(&mut self) -> io::Result<()> {
        if !self.transparency_enabled {
            return Ok(());
        }
        self.transparency_enabled = false;
        let mut stdout = io::stdout();
        transparency::teardown_transparency(&mut stdout)
    }

    /// Full, idempotent cleanup: alternate screen, raw mode, then transparency.
    ///
    /// Every step is attempted even if a previous one failed; the first error is
    /// returned. Calling it twice (explicitly and then from `Drop`) is safe.
    pub fn restore(&mut self) -> io::Result<()> {
        if self.cleaned_up {
            return Ok(());
        }
        self.cleaned_up = true;

        let alt = self.leave_alternate_screen();
        let raw = self.disable_raw_mode();
        let transparency = self.restore_transparency();
        let _ = io::stdout().flush();

        alt.and(raw).and(transparency)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}