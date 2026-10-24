use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::layout::{Constraint, Rect};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};
use ratatui::Frame;

use crate::app::{App, Focus};
use crate::grid::PAGE_SIZE;

pub fn draw_grid(frame: &mut Frame, app: &mut App, area: Rect) {
    app.grid_area = area;
    app.grid.viewport_rows = area.height.saturating_sub(3) as usize;

    let border_color = if app.focus == Focus::Main {
        Color::Yellow
    } else {
        Color::DarkGray
    };

    let block = |title: String| {
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" {title} "))
            .border_style(Style::default().fg(border_color))
    };

    if app.current_table.is_none() {
        let p = Paragraph::new("select a table from the sidebar\n\nits data lands here")
            .block(block("main".to_string()));
        frame.render_widget(p, area);
        return;
    }

    if app.grid.loading && app.grid.rows.is_empty() {
        let p = Paragraph::new(format!(" loading {}… ", app.grid.label))
            .block(block(app.grid.label.clone()));
        frame.render_widget(p, area);
        return;
    }

    let title = if !app.grid.paged {
        format!("{} — {} rows", app.grid.label, app.grid.rows.len())
    } else if app.grid.rows.is_empty() {
        format!("{} — 0 rows", app.grid.label)
    } else {
        let start = app.grid.page * PAGE_SIZE;
        let total_pages = app.grid.total_rows.div_ceil(PAGE_SIZE).max(1);
        format!(
            "{} — rows {}-{} of {} (page {}/{})",
            app.grid.label,
            start + 1,
            start + app.grid.rows.len(),
            app.grid.total_rows,
            app.grid.page + 1,
            total_pages
        )
    };

    let selected_row = app.grid.selected_row();
    let selected_col = app.grid.selected_col;

    let header = Row::new(
        app.grid
            .columns
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let style = if i == selected_col {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                } else {
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                };
                Cell::from(Span::styled(c.clone(), style))
            })
            .collect::<Vec<_>>(),
    );

    let mut table_rows: Vec<Row> = Vec::new();
    for (ri, row) in app.grid.rows.iter().enumerate() {
        let cells: Vec<Cell> = row
            .iter()
            .enumerate()
            .map(|(ci, cell)| {
                let (text, base_style) = match cell {
                    Some(v) => (v.clone(), Style::default()),
                    None => (
                        "NULL".to_string(),
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::ITALIC),
                    ),
                };
                let style = if ri == selected_row && ci == selected_col {
                    Style::default().fg(Color::Black).bg(Color::Yellow)
                } else if ri == selected_row || ci == selected_col {
                    Style::default().fg(Color::Yellow)
                } else {
                    base_style
                };
                Cell::from(Span::styled(text, style))
            })
            .collect();
        table_rows.push(Row::new(cells));
    }

    let widths: Vec<Constraint> = app
        .grid
        .col_widths
        .iter()
        .map(|&w| Constraint::Length(w as u16))
        .collect();

    let mut x = area.x + 1;
    let mut starts: Vec<u16> = Vec::new();
    for &w in &app.grid.col_widths {
        starts.push(x);
        x += w as u16 + 1;
    }
    starts.push(x);
    app.grid_col_starts = starts;

    let table = Table::new(table_rows, widths)
        .header(header)
        .block(block(title))
        .column_spacing(1);

    frame.render_stateful_widget(table, area, &mut app.grid.table_state);
}