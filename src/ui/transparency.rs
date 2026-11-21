use anyhow::Result;
use crossterm::{execute, queue};
use std::io::{self, Write};

/// Attempt to enable true transparency in the terminal.
/// This works by:
/// 1. Checking if the terminal supports the necessary escape sequences
/// 2. Setting the background to transparent (if supported)
/// 3. Relies on compositor (picom, hyprland, etc.) for actual transparency
pub fn setup_transparency<W: Write>(stdout: &mut W) -> Result<()> {
    // Check if we're in a terminal that likely supports transparency
    let term = std::env::var("TERM").unwrap_or_default();
    let term_program = std::env::var("TERM_PROGRAM").unwrap_or_default();
    let _colorterm = std::env::var("COLORTERM").unwrap_or_default();

    // Common terminals with transparency support
    let supports_transparency = term.contains("kitty")
        || term.contains("alacritty")
        || term.contains("wezterm")
        || term.contains("ghostty")
        || term_program.contains("vscode")
        || term_program.contains("iTerm");

    if supports_transparency {
        // Try the modern way first (kitty, wezterm, etc.) - OSC 11 with alpha
        // \x1b]11;#00000000\x1b\\
        let osc11 = "\x1b]11;#00000000\x1b\\";
        write!(stdout, "{}", osc11)?;

        // Also try the rgba format for broader compatibility
        let osc11_alt = "\x1b]11;rgba:0000/0000/0000/0000\x1b\\";
        write!(stdout, "{}", osc11_alt)?;

        // For some terminals, also set the default background color index to transparent
        // OSC 111 ; <color> - set default background color (some terminals)
        let osc111 = "\x1b]111;#00000000\x1b\\";
        write!(stdout, "{}", osc111)?;

        stdout.flush()?;
    }

    Ok(())
}

/// Check if the current terminal/compositor setup likely supports transparency
pub fn check_transparency_support() -> bool {
    let term = std::env::var("TERM").unwrap_or_default();
    let term_program = std::env::var("TERM_PROGRAM").unwrap_or_default();
    let xdg_session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_default();
    let wayland_display = std::env::var("WAYLAND_DISPLAY").is_ok();

    // Check for compositor
    let has_compositor = std::process::Command::new("pgrep")
        .args(["-x", "picom", "compton", "hyprland", "sway", "wayfire", "kwin_wayland", "mutter", "kompmgr"])
        .output()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false);

    term.contains("kitty")
        || term.contains("alacritty")
        || term.contains("wezterm")
        || term.contains("ghostty")
        || term_program.contains("iTerm")
        || (wayland_display && has_compositor)
        || (xdg_session_type == "wayland" && has_compositor)
        || (xdg_session_type == "x11" && has_compositor)
}