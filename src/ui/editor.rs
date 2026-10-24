use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::App;

pub fn draw_editor(frame: &mut Frame, app: &mut App) {
    let area = super::centered_rect(70, 50, frame.area());
    frame.render_widget(Clear, area);

    let inner_height = area.height.saturating_sub(2) as usize;

    if app.editor.cursor_row < app.editor.scroll {
        app.editor.scroll = app.editor.cursor_row;
    }
    if app.editor.cursor_row >= app.editor.scroll + inner_height {
        app.editor.scroll = app.editor.cursor_row + 1 - inner_height;
    }

     let title = if app.editor_filter {
        " where filter — Ctrl+R: apply · empty clears "
    } else {
        " query editor — Ctrl+R: run "
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(Color::Yellow));

    let text = app.editor.lines.join("\n");
    let paragraph = Paragraph::new(text)
        .block(block)
        .scroll((app.editor.scroll as u16, 0));

    frame.render_widget(paragraph, area);

    let cursor_x = area.x + 1 + app.editor.cursor_col as u16;
    let cursor_y = area.y + 1 + (app.editor.cursor_row - app.editor.scroll) as u16;
    frame.set_cursor_position((cursor_x, cursor_y));
}