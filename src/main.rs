mod app;
mod config;
mod db;
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
    // CLI stopgap: pluto add <name> <host> <user> <password> <database> [sslmode]
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
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = DefaultTerminal::new(backend)?;

    let mut app = App::new();
    let result = run(&mut terminal, &mut app);

    disable_raw_mode()?;
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
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    match app.screen {
        Screen::Profiles => match key.code {
            KeyCode::Char('j') | KeyCode::Down => app.next_profile(),
            KeyCode::Char('k') | KeyCode::Up => app.prev_profile(),
            KeyCode::Enter => app.connect_selected(),
            KeyCode::Char('d') => app.delete_selected_profile(),
            KeyCode::Char('q') => app.quit(),
            KeyCode::Char('c') if ctrl => app.quit(),
            _ => {}
        },
        Screen::Main => match key.code {
            KeyCode::Char('q') => app.quit(),
            KeyCode::Char('c') if ctrl => app.quit(),
            KeyCode::Tab => app.toggle_focus(),
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
            _ => {}
        },
    }
}

fn handle_mouse(app: &mut App, mouse: event::MouseEvent) {
    use crossterm::event::{MouseButton, MouseEventKind};

    match mouse.kind {
        MouseEventKind::ScrollDown if in_rect(mouse.column, mouse.row, app.sidebar_rect) => {
            app.sidebar.next(&app.schemas)
        }
        MouseEventKind::ScrollUp if in_rect(mouse.column, mouse.row, app.sidebar_rect) => {
            app.sidebar.prev(&app.schemas)
        }
        MouseEventKind::Down(MouseButton::Left) => {
            if in_rect(mouse.column, mouse.row, app.sidebar_rect) {
                app.focus = Focus::Sidebar;
                // +1 skips the top border row; offset accounts for scrolled-out rows
                let row = mouse.row.saturating_sub(app.sidebar_rect.y + 1) as usize;
                let idx = app.sidebar.list.offset() + row;
                if idx < app.sidebar.visible_len(&app.schemas) {
                    app.sidebar.list.select(Some(idx));
                }
            } else if in_rect(mouse.column, mouse.row, app.main_rect) {
                app.focus = Focus::Main;
            }
        }
        _ => {}
    }
}

fn in_rect(x: u16, y: u16, r: Rect) -> bool {
    x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height
}