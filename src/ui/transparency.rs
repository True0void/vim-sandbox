use std::io::{self, Write};

/// OSC 11 with an alpha channel: makes the default background translucent in
/// terminals that support ARGB/RGBA colors (kitty, wezterm, ghostty, ...).
const OSC11_TRANSPARENT_HEX: &str = "\x1b]11;#00000000\x1b\\";

/// Same as above, RGBA spelling for broader compatibility.
const OSC11_TRANSPARENT_RGBA: &str = "\x1b]11;rgba:0000/0000/0000/0000\x1b\\";

/// OSC 111 *resets* the default background color to the terminal's configured
/// default. It takes no payload: `ESC ] 111 ST` (payloads such as
/// `#00000000` are meaningless and some terminals choke on them).
const OSC111_RESET_BACKGROUND: &str = "\x1b]111\x1b\\";

/// Whether the current terminal is likely to honour background transparency.
fn terminal_supports_transparency() -> bool {
    let term = std::env::var("TERM").unwrap_or_default();
    let term_program = std::env::var("TERM_PROGRAM").unwrap_or_default();

    // Common terminals with transparency support
    term.contains("kitty")
        || term.contains("alacritty")
        || term.contains("wezterm")
        || term.contains("ghostty")
        || term_program.contains("vscode")
        || term_program.contains("iTerm")
}

/// Attempt to enable true transparency in the terminal.
/// This works by:
/// 1. Checking if the terminal supports the necessary escape sequences
/// 2. Setting the background to transparent (if supported)
/// 3. Relies on compositor (picom, hyprland, etc.) for actual transparency
///
/// Returns `true` when the escape sequences were actually emitted, so the
/// caller knows whether [`teardown_transparency`] has to undo anything.
pub fn setup_transparency<W: Write>(stdout: &mut W) -> io::Result<bool> {
    if !terminal_supports_transparency() {
        return Ok(false);
    }

    // Modern way first (kitty, wezterm, etc.) - OSC 11 with alpha
    write!(stdout, "{}", OSC11_TRANSPARENT_HEX)?;

    // Also try the rgba format for broader compatibility
    write!(stdout, "{}", OSC11_TRANSPARENT_RGBA)?;

    stdout.flush()?;
    Ok(true)
}

/// Symmetric teardown of [`setup_transparency`]: restore the terminal's default
/// background so the shell/window keeps its original look after we exit.
///
/// Must be called while the process still owns the terminal (i.e. before
/// leaving the alternate screen for good), and is safe to call unconditionally
/// as long as `setup_transparency` returned `true`.
pub fn teardown_transparency<W: Write>(stdout: &mut W) -> io::Result<()> {
    // OSC 111 without payload = "reset default background color".
    write!(stdout, "{}", OSC111_RESET_BACKGROUND)?;
    // OSC 11 with an *empty* payload is a query in most terminals, so it would
    // not reset anything; OSC 111 above is the portable reset.
    stdout.flush()?;
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