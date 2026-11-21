use anyhow::Result;
use crossterm::{
    event::{Event, KeyEventKind},
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
    execute,
};
use futures_util::StreamExt;
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use std::{io, time::Duration};
use tokio::time::interval;

mod editor;
mod terminal_guard;
mod ui;
mod sandbox;

use editor::core::Editor;
use terminal_guard::TerminalGuard;
use ui::{render::render_ui, background::Background, transparency::setup_transparency};

#[tokio::main]
async fn main() -> Result<()> {
    // Setup terminal with RAII guard
    let mut guard = TerminalGuard::new()?;

    // Enable true transparency if compositor supports it
    let mut stdout = io::stdout();
    setup_transparency(&mut stdout)?;

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

    let res = run_app(&mut terminal, &mut editor, &mut background, &mut tick_rate, &mut event_stream).await;

    // Cleanup - show cursor before guard drops
    terminal.show_cursor()?;
    // Guard will automatically restore terminal on drop
    drop(guard);

    res
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    editor: &mut Editor,
    background: &mut Background,
    tick_rate: &mut tokio::time::Interval,
    event_stream: &mut crossterm::event::EventStream,
) -> Result<()> {
    loop {
        tokio::select! {
            // Animation tick
            _ = tick_rate.tick() => {
                background.update();
                terminal.draw(|f| render_ui(f, editor, background))?;

                // Check for quit request after render
                if editor.status_msg() == "QUIT_REQUESTED" {
                    break;
                }
            }
            // Input events
            Some(Ok(event)) = event_stream.next() => {
                if let Event::Key(key) = event {
                    if key.kind == KeyEventKind::Press {
                        let quit = editor.handle_key(key).await?;
                        if quit {
                            break;
                        }
                    }
                } else if let Event::Resize(w, h) = event {
                    terminal.resize(ratatui::layout::Rect::new(0, 0, w, h))?;
                    background.resize(w, h);
                }
            }
        }
    }
    Ok(())
}