mod app;
mod config;
mod db;
mod grid;
mod input;
mod ui;

use std::io::stdout;
use std::time::Duration;

use app::{App, Entry, Focus, Screen};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::DefaultTerminal;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).is_some_and(|a| a == "add") {
        return add_profile(&args);
    }

    install_panic_hook();

    let rt = tokio::runtime::Runtime::new()?;
    let _rt_guard = rt.enter();

    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, event::EnableMouseCapture)?;
    execute!(
        stdout,
        event::PushKeyboardEnhancementFlags(
            event::KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                | event::KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
        )
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = DefaultTerminal::new(backend)?;

    let mut app = App::new();
    let result = run(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), event::PopKeyboardEnhancementFlags)?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        event::DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

fn add_profile(args: &[String]) -> anyhow::Result<()> {
    if args.len() < 7 {
        eprintln!("usage: pluto add <name> <host> <user> <password> <database> [sslmode]");
        return Ok(());
    }
    let ssl_mode = args.get(7).cloned().unwrap_or_else(|| "require".into());
    let mut cfg = config::load().unwrap_or_default();
    cfg.connections.push(config::ConnectionProfile {
        name: args[2].clone(),
        host: args[3].clone(),
        port: 5432,
        user: args[4].clone(),
        password: args[5].clone(),
        database: args[6].clone(),
        ssl_mode,
    });
    config::save(&cfg)?;
    println!("saved.");
    Ok(())
}

fn install_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), event::PopKeyboardEnhancementFlags);
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen, event::DisableMouseCapture);
        original(info);
    }));
}

fn run(terminal: &mut DefaultTerminal, app: &mut App) -> anyhow::Result<()> {
    while app.running {
        terminal.draw(|frame| ui::draw(frame, app))?;

        while let Ok(event) = app.rx.try_recv() {
            app.handle_db_event(event);
        }

        handle_input(app)?;
    }
    Ok(())
}

fn handle_input(app: &mut App) -> anyhow::Result<()> {
    if !event::poll(Duration::from_millis(33))? {
        return Ok(());
    }
    match event::read()? {
        Event::Key(key) => handle_key(app, key),
        Event::Mouse(mouse) => handle_mouse(app, mouse),
        _ => {}
    }
    Ok(())
}

fn handle_key(app: &mut App, key: event::KeyEvent) {
    if key.kind != KeyEventKind::Press {
        return;
    }
    if app.screen == Screen::Main && app.editor.visible {
        handle_editor_key(app, key);
        return;
    }
    if app.screen == Screen::Main && app.viewer.visible {
        handle_viewer_key(app, key);
        return;
    }

    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    match app.screen {
        Screen::Profiles => match key.code {
            KeyCode::Char('j') | KeyCode::Down => { app.profile_info_open = false; app.next_profile() }
            KeyCode::Char('k') | KeyCode::Up => { app.profile_info_open = false; app.prev_profile() }
            KeyCode::Enter => app.connect_selected(),
            KeyCode::Char('i') => app.profile_info_open = !app.profile_info_open,
            KeyCode::Esc => app.profile_info_open = false,
            KeyCode::Char('d') => app.delete_selected_profile(),
            KeyCode::Char('q') => app.quit(),
            KeyCode::Char('c') if ctrl => app.quit(),
            _ => {}
        },
        Screen::Main => match key.code {
            KeyCode::Char('q') => app.quit(),
            KeyCode::Char('c') if ctrl => app.quit(),
            KeyCode::Tab => app.toggle_focus(),
            KeyCode::Char('i') => app.open_editor(),
            KeyCode::Char('w') => app.open_filter_editor(),
            KeyCode::Char('m') => app.toggle_metadata(),
            KeyCode::Char('f') => app.toggle_focus_mode(),
            KeyCode::Esc => app.focus_mode = false,
            _ if app.focus == Focus::Sidebar => match key.code {
                KeyCode::Char('j') | KeyCode::Down => app.sidebar.next(&app.schemas),
                KeyCode::Char('k') | KeyCode::Up => app.sidebar.prev(&app.schemas),
                KeyCode::Char('g') => app.sidebar.first(&app.schemas),
                KeyCode::Char('G') => app.sidebar.last(&app.schemas),
                KeyCode::Enter => match app.sidebar.selected_entry(&app.schemas) {
                    Some(Entry::Schema(_)) => app.sidebar.toggle(&app.schemas),
                    Some(Entry::Table(si, ti)) => app.select_table(si, ti),
                    None => {}
                },
                KeyCode::Char('h') | KeyCode::Left => app.sidebar.collapse(&app.schemas),
                KeyCode::Char('l') | KeyCode::Right => app.sidebar.expand(&app.schemas),
                _ => {}
            },
            _ if app.focus == Focus::Main => match key.code {
                KeyCode::Char('j') | KeyCode::Down => app.grid.next_row(),
                KeyCode::Char('k') | KeyCode::Up => app.grid.prev_row(),
                KeyCode::Char('l') | KeyCode::Right => app.grid.next_col(),
                KeyCode::Char('h') | KeyCode::Left => app.grid.prev_col(),
                KeyCode::Char('g') => app.grid.top(),
                KeyCode::Char('G') => app.grid.bottom(),
                KeyCode::Char('d') if ctrl => app.grid.half_page_down(),
                KeyCode::Char('u') if ctrl => app.grid.half_page_up(),
                KeyCode::Char(']') => app.next_page(),
                KeyCode::Char('[') => app.prev_page(),
                KeyCode::Char('r') => app.refresh_table(),
                KeyCode::Char('s') => app.cycle_sort(),
                KeyCode::Char('-') => app.grid.widen_col(),
                KeyCode::Char('=') => app.grid.narrow_col(),
                KeyCode::Char('x') => app.grid.toggle_hide_col(),
                KeyCode::Char('0') => app.grid.reset_layout(),
                KeyCode::Char('e') => app.export_csv(),
                KeyCode::Enter => app.open_cell_viewer(),
                _ => {}
            },
            _ => {}
        },
    }
}

fn handle_editor_key(app: &mut App, key: event::KeyEvent) {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Esc => app.editor.visible = false,
        KeyCode::Enter if ctrl => app.run_editor_query(),
        KeyCode::Char('r') if ctrl => app.run_editor_query(),
        KeyCode::Char('e') if ctrl => app.run_editor_query(),
        KeyCode::F(5) => app.run_editor_query(),
        KeyCode::Enter => app.editor.newline(),
        KeyCode::Backspace => app.editor.backspace(),
        KeyCode::Delete => app.editor.delete_char(),
        KeyCode::Left => app.editor.move_left(),
        KeyCode::Right => app.editor.move_right(),
        KeyCode::Up => app.editor.move_up(),
        KeyCode::Down => app.editor.move_down(),
        KeyCode::Home => app.editor.move_home(),
        KeyCode::End => app.editor.move_end(),
        KeyCode::Char(c) if !ctrl => app.editor.insert_char(c),
        _ => {}
    }
}

fn handle_viewer_key(app: &mut App, key: event::KeyEvent) {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let half = (app.viewer_page / 2).max(1);
    match key.code {
        KeyCode::Esc | KeyCode::Enter => app.viewer.close(),
        KeyCode::Char('q') => app.viewer.close(),
        KeyCode::Char('j') | KeyCode::Down => app.viewer.scroll = app.viewer.scroll.saturating_add(1),
        KeyCode::Char('k') | KeyCode::Up => app.viewer.scroll = app.viewer.scroll.saturating_sub(1),
        KeyCode::Char('d') if ctrl => app.viewer.scroll = app.viewer.scroll.saturating_add(half),
        KeyCode::Char('u') if ctrl => app.viewer.scroll = app.viewer.scroll.saturating_sub(half),
        KeyCode::Char('g') => app.viewer.scroll = 0,
        KeyCode::Char('G') => app.viewer.scroll = usize::MAX,
        _ => {}
    }
}

fn handle_mouse(app: &mut App, mouse: event::MouseEvent) {
    use crossterm::event::{MouseButton, MouseEventKind};

    if app.viewer.visible {
        match mouse.kind {
            MouseEventKind::ScrollDown => {
                app.viewer.scroll = app.viewer.scroll.saturating_add(3);
            }
            MouseEventKind::ScrollUp => {
                app.viewer.scroll = app.viewer.scroll.saturating_sub(3);
            }
            _ => {}
        }
        return;
    }

    match mouse.kind {
        MouseEventKind::ScrollDown if in_rect(mouse.column, mouse.row, app.sidebar_rect) => {
            app.sidebar.next(&app.schemas)
        }
        MouseEventKind::ScrollUp if in_rect(mouse.column, mouse.row, app.sidebar_rect) => {
            app.sidebar.prev(&app.schemas)
        }
        MouseEventKind::ScrollDown if in_rect(mouse.column, mouse.row, app.grid_area) => {
            for _ in 0..3 {
                app.grid.next_row();
            }
        }
        MouseEventKind::ScrollUp if in_rect(mouse.column, mouse.row, app.grid_area) => {
            for _ in 0..3 {
                app.grid.prev_row();
            }
        }
        MouseEventKind::Down(MouseButton::Left) => {
            if in_rect(mouse.column, mouse.row, app.sidebar_rect) {
                app.focus = Focus::Sidebar;
                let row = mouse.row.saturating_sub(app.sidebar_rect.y + 1) as usize;
                let idx = app.sidebar.list.offset() + row;
                if idx < app.sidebar.visible_len(&app.schemas) {
                    app.sidebar.list.select(Some(idx));
                }
            } else if in_rect(mouse.column, mouse.row, app.grid_area) {
                app.focus = Focus::Main;
                let header_y = app.grid_area.y + 1;
                let body_y = app.grid_area.y + 2;
                if mouse.row == header_y {
                    if let Some(ci) = col_at(&app.grid_col_starts, mouse.column) {
                        if ci < app.grid.columns.len() {
                            app.grid.selected_col = ci;
                        }
                    }
                } else if mouse.row >= body_y {
                    let row = (mouse.row - body_y) as usize + app.grid.table_state.offset();
                    if row < app.grid.rows.len() {
                        app.grid.table_state.select(Some(row));
                        if let Some(ci) = col_at(&app.grid_col_starts, mouse.column) {
                            if ci < app.grid.columns.len() {
                                app.grid.selected_col = ci;
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

fn col_at(starts: &[u16], x: u16) -> Option<usize> {
    if starts.len() < 2 {
        return None;
    }
    (0..starts.len() - 1).rev().find(|&i| x >= starts[i] && x < starts[i + 1])
}

fn in_rect(x: u16, y: u16, r: Rect) -> bool {
    x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height
}