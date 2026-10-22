use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::app::{App, Screen};

pub fn draw(frame: &mut Frame, app: &mut App) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(frame.area());

    match app.screen {
        Screen::Profiles => draw_profiles(frame, app, root[0]),
        Screen::Main => draw_main(frame, app, root[0]),
    }
    draw_status_bar(frame, app, root[1]);
}

fn draw_profiles(frame: &mut Frame, app: &mut App, area: Rect) {
    let area = centered_rect(60, 50, area);

    if app.profiles.is_empty() {
        let hint = Paragraph::new(
            "no saved connections\n\nadd one :\n  pluto add <name> <host> <user> <password> <database>",
        )
        .block(Block::default().borders(Borders::ALL).title(" pluto "));
        frame.render_widget(hint, area);
        return;
    }

    let items: Vec<ListItem> = app
        .profiles
        .iter()
        .map(|p| ListItem::new(format!("{}   ({}/{})", p.name, p.host, p.database)))
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" select connection "),
        )
        .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, area, &mut app.profile_list);
}

fn draw_main(frame: &mut Frame, app: &App, area: Rect) {
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(area);

    let sidebar_inner = Block::default()
        .borders(Borders::ALL)
        .title(" tables ")
        .border_style(Style::default().fg(Color::DarkGray));

    let content = if app.loading_tables {
        Paragraph::new(" loading… ")
    } else if app.schemas.is_empty() {
        Paragraph::new(" no tables ")
    } else {
        let mut lines: Vec<Line> = Vec::new();
        for schema in &app.schemas {
            lines.push(Line::from(Span::styled(
                format!("▸ {}", schema.name),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            )));
            for table in &schema.tables {
                lines.push(Line::from(format!("    {}", table)));
            }
        }
        Paragraph::new(lines)
    };

    frame.render_widget(content.block(sidebar_inner), panes[0]);

    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .title(" main ")
            .border_style(Style::default().fg(Color::DarkGray)),
        panes[1],
    );
}

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let hints = match app.screen {
        Screen::Profiles => "↑/↓: select  |  Enter: connect  |  d: delete  |  q: quit",
        Screen::Main => "q: quit",
    };
    let status_color = if app.status.starts_with("connection failed")
        || app.status.starts_with("failed")
    {
        Color::Red
    } else {
        Color::Yellow
    };
    let status = Paragraph::new(Line::from(vec![
        Span::styled(format!(" {} ", app.status), Style::default().fg(status_color)),
        Span::styled(
            hints,
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
        ),
    ]));
    frame.render_widget(status, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}