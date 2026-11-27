use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Paragraph, Widget},
    Frame,
};

use crate::editor::core::{Editor, Mode, char_to_byte};
use crate::ui::background::Background;

pub fn render_ui(f: &mut Frame, editor: &Editor, background: &mut Background) {
    let area = f.area();

    // First, draw background animations directly to buffer
    background.draw(f.buffer_mut(), area);

    // Create main layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),      // Editor area
            Constraint::Length(1),   // Status line
            Constraint::Length(1),   // Command line
        ])
        .split(area);

    // Render editor content with transparency (no background block)
    render_editor_area(f, editor, chunks[0]);

    // Render status line
    render_status_line(f, editor, chunks[1]);

    // Render command line
    render_command_line(f, editor, chunks[2]);
}

fn render_editor_area(f: &mut Frame, editor: &Editor, area: Rect) {
    let buffer = editor.buffer();
    let cursor = editor.cursor();
    let viewport = editor.viewport();
    let mode = editor.mode();

    // Calculate visible lines
    let visible_height = area.height as usize;
    let start_row = viewport.row;
    let end_row = (start_row + visible_height).min(buffer.len());

    // Build text lines
    let mut lines = Vec::new();
    for row in start_row..end_row {
        let line_content = buffer.line(row).unwrap_or("");
        let mut spans = Vec::new();

        // Line number
        let num_style = if row == cursor.row {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        spans.push(Span::styled(format!("{:4} │ ", row + 1), num_style));

        // Line content with cursor highlight
        if row == cursor.row && mode == Mode::Normal {
            let byte_col = char_to_byte(line_content, cursor.col);
            let before = &line_content[..byte_col.min(line_content.len())];
            let at = line_content.chars().nth(cursor.col).unwrap_or(' ');
            let after = &line_content[byte_col.min(line_content.len())..];

            spans.push(Span::styled(before, Style::default().fg(Color::White)));
            spans.push(Span::styled(
                at.to_string(),
                Style::default().bg(Color::White).fg(Color::Black).add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(after, Style::default().fg(Color::White)));
        } else if row == cursor.row && mode == Mode::Insert {
            let byte_col = char_to_byte(line_content, cursor.col);
            let before = &line_content[..byte_col.min(line_content.len())];
            let after = &line_content[byte_col.min(line_content.len())..];
            spans.push(Span::styled(before, Style::default().fg(Color::White)));
            spans.push(Span::styled(
                " ",
                Style::default().bg(Color::Green).fg(Color::Black),
            ));
            spans.push(Span::styled(after, Style::default().fg(Color::White)));
        } else {
            spans.push(Span::styled(line_content, Style::default().fg(Color::White)));
        }

        lines.push(Line::from(spans));
    }

    // Fill remaining space with empty lines (showing background)
    for _ in lines.len()..visible_height {
        lines.push(Line::raw(""));
    }

    let text = Text::from(lines);
    let paragraph = Paragraph::new(text)
        .style(Style::default())
        .alignment(Alignment::Left);

    f.render_widget(paragraph, area);
}

fn render_status_line(f: &mut Frame, editor: &Editor, area: Rect) {
    let mode = editor.mode();
    let cursor = editor.cursor();
    let buffer = editor.buffer();

    let mode_str = match mode {
        Mode::Normal => "NORMAL",
        Mode::Insert => "INSERT",
        Mode::Visual => "VISUAL",
        Mode::VisualLine => "V-LINE",
        Mode::Command => "COMMAND",
    };

    let mode_color = match mode {
        Mode::Normal => Color::Green,
        Mode::Insert => Color::Blue,
        Mode::Visual | Mode::VisualLine => Color::Magenta,
        Mode::Command => Color::Yellow,
    };

    let file_name = buffer.file_path()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("[No Name]");

    let modified = if buffer.is_modified() { " ●" } else { "" };

    let left = Line::from(vec![
        Span::styled(format!(" {} ", mode_str), Style::default().fg(Color::Black).bg(mode_color).add_modifier(Modifier::BOLD)),
        Span::styled(" ", Style::default().bg(Color::DarkGray)),
        Span::styled(format!(" {} ", file_name), Style::default().fg(Color::White).bg(Color::DarkGray)),
        Span::styled(modified, Style::default().fg(Color::Red).bg(Color::DarkGray).add_modifier(Modifier::BOLD)),
    ]);

    let right = Line::from(vec![
        Span::styled(
            format!(" Ln {}, Col {} ", cursor.row + 1, cursor.col + 1),
            Style::default().fg(Color::White).bg(Color::DarkGray),
        ),
        Span::styled(
            format!(" {}% ", if buffer.len() > 0 { ((cursor.row + 1) * 100) / buffer.len() } else { 0 }),
            Style::default().fg(Color::Cyan).bg(Color::DarkGray).add_modifier(Modifier::BOLD),
        ),
    ]);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(area);

    let left_para = Paragraph::new(left).style(Style::default().bg(Color::DarkGray));
    let right_para = Paragraph::new(right).alignment(Alignment::Right).style(Style::default().bg(Color::DarkGray));

    f.render_widget(left_para, chunks[0]);
    f.render_widget(right_para, chunks[1]);
}

fn render_command_line(f: &mut Frame, editor: &Editor, area: Rect) {
    let mode = editor.mode();
    let status = editor.status_msg();
    let cmd_buffer = editor.command_buffer();

    // Quitting is driven by `Editor::should_quit`, never by a magic status
    // string, so any status message can be displayed verbatim.
    if mode == Mode::Command {
        // Show the command being typed
        let cmd = Line::from(vec![
            Span::styled(&*cmd_buffer, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled("_", Style::default().fg(Color::Yellow).add_modifier(Modifier::SLOW_BLINK)),
        ]);
        let para = Paragraph::new(cmd).style(Style::default().bg(Color::Black));
        f.render_widget(para, area);
    } else if !status.is_empty() {
        let msg = Line::from(vec![
            Span::styled(&*status, Style::default().fg(Color::Cyan).add_modifier(Modifier::ITALIC)),
        ]);
        let para = Paragraph::new(msg).style(Style::default().bg(Color::Black));
        f.render_widget(para, area);
    }
    // Otherwise leave empty (shows background grass)
}