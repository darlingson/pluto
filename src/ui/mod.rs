use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::App;

pub fn draw(frame: &mut Frame, _app: &App) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(frame.area());

    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(root[0]);

    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .title(" sidebar ")
            .border_style(Style::default().fg(Color::DarkGray)),
        panes[0],
    );

    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .title(" main ")
            .border_style(Style::default().fg(Color::DarkGray)),
        panes[1],
    );

    let status = Paragraph::new(Span::styled(
        " q: quit  |  Ctrl-C: quit ",
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::DIM),
    ));
    frame.render_widget(status, root[1]);
}
