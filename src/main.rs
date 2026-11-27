use anyhow::Result;
use crossterm::event::{Event, KeyEventKind};
use futures_util::StreamExt;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, time::Duration};
use tokio::time::interval;

mod editor;
mod terminal_guard;
mod ui;

use editor::core::Editor;
use terminal_guard::TerminalGuard;
use ui::{background::Background, render::render_ui};

#[tokio::main]
async fn main() -> Result<()> {
    // Setup terminal with RAII guard: raw mode + alternate screen.
    // From here on, every exit path (error, panic, signal, normal return)
    // restores the terminal through `Drop`.
    let mut guard = TerminalGuard::new()?;

    // Enable true transparency if the terminal/compositor supports it.
    // The guard remembers it and undoes it (OSC 111) during cleanup.
    guard.enable_transparency()?;

    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    // Initialize editor and background animations
    let mut editor = Editor::new()?;
    let size = terminal.size()?;
    let mut background = Background::new(ratatui::layout::Rect::new(0, 0, size.width, size.height));

    // If a file was passed as argument, open it
    if let Some(file) = std::env::args().nth(1) {
        let _ = editor.open_file(&file);
    }

    // Event loop with fixed timestep for animations
    let mut tick_rate = interval(Duration::from_millis(16)); // ~60fps
    let mut event_stream = crossterm::event::EventStream::new();
    let mut signals = ShutdownSignals::new()?;

    let res = run_app(
        &mut terminal,
        &mut editor,
        &mut background,
        &mut tick_rate,
        &mut event_stream,
        &mut signals,
    )
    .await;

    // Cleanup - show cursor and restore the terminal explicitly.
    // `restore()` is idempotent, so the `Drop` below is a no-op.
    let show_cursor = terminal.show_cursor();
    let restored = guard.restore();
    drop(guard);

    // Report the application error first, then any cleanup error.
    res?;
    show_cursor?;
    restored?;

    Ok(())
}

/// Shutdown signals: Ctrl+C (SIGINT), SIGTERM and SIGHUP.
///
/// Installing handlers keeps the process from being killed outright, so the
/// event loop can quit cleanly and let `TerminalGuard::drop` restore raw mode,
/// the alternate screen and the background colour instead of leaving the user's
/// shell broken.
struct ShutdownSignals {
    #[cfg(unix)]
    sigterm: tokio::signal::unix::Signal,
    #[cfg(unix)]
    sighup: tokio::signal::unix::Signal,
}

impl ShutdownSignals {
    fn new() -> io::Result<Self> {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            Ok(Self {
                sigterm: signal(SignalKind::terminate())?,
                sighup: signal(SignalKind::hangup())?,
            })
        }
        #[cfg(not(unix))]
        {
            Ok(Self {})
        }
    }

    /// Resolves as soon as any shutdown signal is received.
    async fn recv(&mut self) {
        #[cfg(unix)]
        {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = self.sigterm.recv() => {}
                _ = self.sighup.recv() => {}
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }
    }
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    editor: &mut Editor,
    background: &mut Background,
    tick_rate: &mut tokio::time::Interval,
    event_stream: &mut crossterm::event::EventStream,
    signals: &mut ShutdownSignals,
) -> Result<()> {
    loop {
        tokio::select! {
            // Animation tick
            _ = tick_rate.tick() => {
                background.update();
                terminal.draw(|f| render_ui(f, editor, background))?;

                // Explicit quit flag, immune to any status-line message.
                if editor.should_quit() {
                    break;
                }
            }
            // Input events
            Some(Ok(event)) = event_stream.next() => {
                match event {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        let quit = editor.handle_key(key).await?;
                        // Checked after *every* key, whatever the mode.
                        if quit || editor.should_quit() {
                            break;
                        }
                        // Consomme les événements de thème émis par l'éditeur
                        // (:theme, :themes, :themenext, :themeprev, :colorscheme).
                        let status = editor.status_msg().to_string();
                        if status.starts_with("THEME_SET:") {
                            let name = status.strip_prefix("THEME_SET:").unwrap_or("");
                            if background.set_theme(name) {
                                editor.set_status(format!("Theme: {}", name));
                            } else if !name.is_empty() {
                                editor.set_status(format!("Unknown theme: {}", name));
                            } else {
                                editor.set_status(format!("Theme: {}", background.current_theme_name()));
                            }
                        } else if status == "THEME_LIST" {
                            editor.set_status("Themes: spider-lily (1/1)");
                        } else if status == "THEME_NEXT" {
                            background.next_theme();
                            editor.set_status(format!("Theme: {}", background.current_theme_name()));
                        } else if status == "THEME_PREV" {
                            background.prev_theme();
                            editor.set_status(format!("Theme: {}", background.current_theme_name()));
                        } else if status == "THEME_GET_CURRENT" {
                            editor.set_status(format!("Theme: {}", background.current_theme_name()));
                        } else {
                            // Pas un événement thème : efface le statut thématique
                            // pour ne pas le retraiter à la prochaine itération.
                            // Les autres statuts (undo, yank, etc.) sont gérés par l'éditeur.
                            if status.starts_with("THEME_") {
                                editor.set_status("");
                            }
                        }
                    }
                    Event::Resize(w, h) => {
                        terminal.resize(ratatui::layout::Rect::new(0, 0, w, h))?;
                        background.resize(w, h);
                    }
                    _ => {}
                }
            }
            // SIGINT / SIGTERM / SIGHUP: leave the loop cleanly so `Drop`
            // restores raw mode, the alternate screen and the background.
            _ = signals.recv() => {
                editor.request_quit();
                break;
            }
        }
    }
    Ok(())
}