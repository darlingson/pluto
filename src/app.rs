use ratatui::layout::Rect;
use ratatui::widgets::ListState;
use sqlx::PgPool;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::config::{self, ConnectionProfile};
use crate::db::{self, Metadata};
use crate::grid::{Grid, PAGE_SIZE};
use crate::input::Editor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Profiles,
    Main,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Sidebar,
    Main,
}

#[derive(Debug)]
pub enum DbEvent {
    Connected(PgPool),
    ConnectFailed(String),
    TablesLoaded(Vec<(String, String)>),
    TablesFailed(String),
    TablePageLoaded {
        columns: Vec<String>,
        rows: Vec<Vec<Option<String>>>,
        total: usize,
        page: usize,
    },
    TableQueryFailed(String),
    MetadataLoaded { label: String, meta: Metadata },
    MetadataFailed(String),
}

#[derive(Debug)]
pub struct Schema {
    pub name: String,
    pub tables: Vec<String>,
}

pub enum Entry {
    Schema(usize),
    Table(usize, usize),
}

#[derive(Debug)]
pub struct Viewer {
    pub visible: bool,
    pub title: String,
    pub content: String,
    pub scroll: usize,
}

impl Viewer {
    pub fn new() -> Self {
        Self {
            visible: false,
            title: String::new(),
            content: String::new(),
            scroll: 0,
        }
    }

    pub fn open(&mut self, title: String, content: String) {
        self.title = title;
        self.content = content;
        self.scroll = 0;
        self.visible = true;
    }

    pub fn close(&mut self) {
        self.visible = false;
    }
}

#[derive(Debug)]
pub struct Sidebar {
    pub list: ListState,
    pub expanded: Vec<bool>,
}

impl Sidebar {
    pub fn new() -> Self {
        let mut list = ListState::default();
        list.select(Some(0));
        Self {
            list,
            expanded: Vec::new(),
        }
    }

    pub fn is_expanded(&self, schema_idx: usize) -> bool {
        self.expanded.get(schema_idx).copied().unwrap_or(true)
    }

    pub fn visible_len(&self, schemas: &[Schema]) -> usize {
        schemas
            .iter()
            .enumerate()
            .map(|(i, s)| 1 + if self.is_expanded(i) { s.tables.len() } else { 0 })
            .sum()
    }

    pub fn entry_at(&self, schemas: &[Schema], index: usize) -> Option<Entry> {
        let mut i = 0;
        for (si, s) in schemas.iter().enumerate() {
            if i == index {
                return Some(Entry::Schema(si));
            }
            i += 1;
            if self.is_expanded(si) {
                for ti in 0..s.tables.len() {
                    if i == index {
                        return Some(Entry::Table(si, ti));
                    }
                    i += 1;
                }
            }
        }
        None
    }

    pub fn selected_entry(&self, schemas: &[Schema]) -> Option<Entry> {
        self.list.selected().and_then(|i| self.entry_at(schemas, i))
    }

    fn index_of_schema(&self, schemas: &[Schema], si: usize) -> Option<usize> {
        let mut i = 0;
        for (j, s) in schemas.iter().enumerate() {
            if j == si {
                return Some(i);
            }
            i += 1 + if self.is_expanded(j) { s.tables.len() } else { 0 };
        }
        None
    }

    pub fn next(&mut self, schemas: &[Schema]) {
        let len = self.visible_len(schemas);
        if len == 0 {
            return;
        }
        let i = self.list.selected().unwrap_or(0);
        self.list.select(Some((i + 1).min(len - 1)));
    }

    pub fn prev(&mut self, _schemas: &[Schema]) {
        let i = self.list.selected().unwrap_or(0);
        self.list.select(Some(i.saturating_sub(1)));
    }

    pub fn first(&mut self, schemas: &[Schema]) {
        if self.visible_len(schemas) > 0 {
            self.list.select(Some(0));
        }
    }

    pub fn last(&mut self, schemas: &[Schema]) {
        let len = self.visible_len(schemas);
        if len > 0 {
            self.list.select(Some(len - 1));
        }
    }

    pub fn toggle(&mut self, schemas: &[Schema]) {
        if let Some(Entry::Schema(si)) = self.selected_entry(schemas) {
            self.set_expanded(si, !self.is_expanded(si));
        }
    }

    pub fn collapse(&mut self, schemas: &[Schema]) {
        match self.selected_entry(schemas) {
            Some(Entry::Schema(si)) => self.set_expanded(si, false),
            Some(Entry::Table(si, _)) => {
                self.set_expanded(si, false);
                if let Some(idx) = self.index_of_schema(schemas, si) {
                    self.list.select(Some(idx));
                }
            }
            None => {}
        }
    }

    pub fn expand(&mut self, schemas: &[Schema]) {
        if let Some(Entry::Schema(si)) = self.selected_entry(schemas) {
            self.set_expanded(si, true);
        }
    }

    fn set_expanded(&mut self, si: usize, value: bool) {
        if si < self.expanded.len() {
            self.expanded[si] = value;
        }
    }
}

#[derive(Debug)]
pub struct App {
    pub screen: Screen,
    pub running: bool,
    pub status: String,

    pub profiles: Vec<ConnectionProfile>,
    pub profile_list: ListState,
    pub profile_info_open: bool,
    pub connecting: bool,

    pub focus: Focus,
    pub sidebar: Sidebar,
    pub pool: Option<PgPool>,
    pub schemas: Vec<Schema>,
    pub loading_tables: bool,
    pub sidebar_rect: Rect,
    pub main_rect: Rect,

    pub grid: Grid,
    pub current_table: Option<(String, String)>,
    pub grid_area: Rect,
    pub grid_col_starts: Vec<u16>,

    pub editor: Editor,
    pub editor_filter: bool,
    pub viewer: Viewer,
    pub focus_mode: bool,
    pub focus_scroll: usize,
    pub viewer_page: usize,

    pub metadata_open: bool,
    pub loading_metadata: bool,
    pub metadata: Option<(String, Metadata)>,

    pub tx: UnboundedSender<DbEvent>,
    pub rx: UnboundedReceiver<DbEvent>,
}

impl App {
    pub fn new() -> Self {
        let profiles = config::load().map(|c| c.connections).unwrap_or_default();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let mut profile_list = ListState::default();
        if !profiles.is_empty() {
            profile_list.select(Some(0));
        }
        let status = if profiles.is_empty() {
            "no saved profiles — run: pluto add <name> <host> <user> <password> <db>".to_string()
        } else {
            String::new()
        };
        let mut editor = Editor::new();
        editor.history = config::load_history();
        Self {
            screen: Screen::Profiles,
            running: true,
            status,
            profiles,
            profile_list,
            profile_info_open: false,
            connecting: false,
            focus: Focus::Sidebar,
            sidebar: Sidebar::new(),
            pool: None,
            schemas: Vec::new(),
            loading_tables: false,
            sidebar_rect: Rect::default(),
            main_rect: Rect::default(),
            grid: Grid::new(),
            current_table: None,
            grid_area: Rect::default(),
            grid_col_starts: Vec::new(),
            editor,
            editor_filter: false,
            viewer: Viewer::new(),
            focus_mode: false,
            focus_scroll: 0,
            viewer_page: 10,
            metadata_open: false,
            loading_metadata: false,
            metadata: None,
            tx,
            rx,
        }
    }

    pub fn quit(&mut self) {
        self.running = false;
    }

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Sidebar => Focus::Main,
            Focus::Main => Focus::Sidebar,
        };
    }

    pub fn toggle_focus_mode(&mut self) {
        if self.grid.rows.is_empty() {
            self.status = "nothing to focus".to_string();
            return;
        }
        self.focus_mode = !self.focus_mode;
        self.focus_scroll = 0;
    }

    pub fn toggle_metadata(&mut self) {
        if self.current_table.is_none() {
            self.status = "select a table first".to_string();
            return;
        }
        self.metadata_open = !self.metadata_open;
        if self.metadata_open {
            self.load_metadata();
        }
    }

    fn load_metadata(&mut self) {
        let Some((schema, table)) = self.current_table.clone() else {
            return;
        };
        let label = format!("{schema}.{table}");
        if let Some((l, _)) = &self.metadata {
            if *l == label {
                return;
            }
        }
        let Some(pool) = self.pool.clone() else { return };
        self.loading_metadata = true;
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let event = match db::load_metadata(&pool, &schema, &table).await {
                Ok(meta) => DbEvent::MetadataLoaded { label, meta },
                Err(e) => DbEvent::MetadataFailed(e.to_string()),
            };
            let _ = tx.send(event);
        });
    }

    pub fn cycle_sort(&mut self) {
        if !self.grid.paged || self.current_table.is_none() {
            self.status = "sorting applies to table browsing".to_string();
            return;
        }
        if self.grid.columns.is_empty() {
            return;
        }
        let col = self.grid.selected_col;
        self.grid.sort = match self.grid.sort {
            Some((c, true)) if c == col => Some((c, false)),
            Some((c, false)) if c == col => None,
            _ => Some((col, true)),
        };
        self.status = match self.grid.sort {
            Some((c, true)) => format!("sorted by {} ASC", self.grid.columns[c]),
            Some((c, false)) => format!("sorted by {} DESC", self.grid.columns[c]),
            None => "sort cleared".to_string(),
        };
        if let Some((s, t)) = self.current_table.clone() {
            self.load_table_page(s, t, 0);
        }
    }

    pub fn open_cell_viewer(&mut self) {
        let row = self.grid.selected_row();
        let col = self.grid.selected_col;
        let Some(cell) = self.grid.rows.get(row).and_then(|r| r.get(col)) else {
            self.status = "no cell selected".to_string();
            return;
        };
        let col_name = self.grid.columns.get(col).cloned().unwrap_or_default();
        let content = cell.clone().unwrap_or_else(|| "(NULL)".to_string());
        let title = format!("{} — row {} — {}", self.grid.label, row + 1, col_name);
        self.viewer.open(title, content);
    }

    pub fn export_csv(&mut self) {
        if self.grid.columns.is_empty() {
            self.status = "nothing to export".to_string();
            return;
        }
        let visible: Vec<usize> = (0..self.grid.columns.len())
            .filter(|&i| !self.grid.hidden(i))
            .collect();

        let mut out = String::new();
        out.push_str(&csv_line(
            visible.iter().map(|&i| self.grid.columns[i].as_str()),
        ));
        for row in &self.grid.rows {
            out.push_str(&csv_line(
                visible
                    .iter()
                    .map(|&i| row.get(i).and_then(|c| c.as_deref()).unwrap_or("")),
            ));
        }

        let epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let safe: String = self
            .grid
            .label
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let safe = if safe.is_empty() { "query".to_string() } else { safe };
        let path = format!("pluto-{safe}-{epoch}.csv");

        match std::fs::write(&path, out) {
            Ok(()) => {
                self.status = format!("exported {} rows → {path}", self.grid.rows.len())
            }
            Err(e) => self.status = format!("export failed: {e}"),
        }
    }

    pub fn next_profile(&mut self) {
        if self.profiles.is_empty() {
            return;
        }
        let i = self.profile_list.selected().unwrap_or(0);
        self.profile_list.select(Some((i + 1) % self.profiles.len()));
    }

    pub fn prev_profile(&mut self) {
        if self.profiles.is_empty() {
            return;
        }
        let i = self.profile_list.selected().unwrap_or(0);
        let prev = if i == 0 { self.profiles.len() - 1 } else { i - 1 };
        self.profile_list.select(Some(prev));
    }

    pub fn delete_selected_profile(&mut self) {
        if self.profiles.is_empty() {
            return;
        }
        let i = self.profile_list.selected().unwrap_or(0);
        let removed = self.profiles.remove(i);
        if let Ok(mut cfg) = config::load() {
            cfg.connections.retain(|c| c.name != removed.name);
            let _ = config::save(&cfg);
        }
        self.status = format!("deleted profile '{}'", removed.name);
        if self.profiles.is_empty() {
            self.profile_list.select(None);
        } else {
            self.profile_list.select(Some(i.min(self.profiles.len() - 1)));
        }
    }

    pub fn connect_selected(&mut self) {
        if self.connecting {
            return;
        }
        let Some(profile) = self
            .profiles
            .get(self.profile_list.selected().unwrap_or(0))
            .cloned()
        else {
            return;
        };
        self.connecting = true;
        self.status = format!("connecting to {}…", profile.name);
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let event = match db::connect(&profile).await {
                Ok(pool) => DbEvent::Connected(pool),
                Err(e) => DbEvent::ConnectFailed(e.to_string()),
            };
            let _ = tx.send(event);
        });
    }

    pub fn select_table(&mut self, si: usize, ti: usize) {
        let names = self
            .schemas
            .get(si)
            .and_then(|s| s.tables.get(ti).map(|t| (s.name.clone(), t.clone())));
        if let Some((schema, table)) = names {
            self.focus = Focus::Main;
            self.focus_mode = false;
            self.grid.sort = None;
            self.grid.filter = None;
            self.current_table = Some((schema.clone(), table.clone()));
            self.load_table_page(schema, table, 0);
            if self.metadata_open {
                self.metadata = None;
                self.load_metadata();
            }
        }
    }

    fn load_table_page(&mut self, schema: String, table: String, page: usize) {
        let Some(pool) = self.pool.clone() else { return };
        let order = self
            .grid
            .sort
            .and_then(|(ci, asc)| self.grid.columns.get(ci).map(|c| (c.clone(), asc)));
        let filter = self.grid.filter.clone();
        self.grid.loading = true;
        self.grid.paged = true;
        self.grid.page = page;
        self.status = format!("loading {schema}.{table} (page {})…", page + 1);
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let event = match db::fetch_table_page(
                &pool,
                &schema,
                &table,
                page,
                PAGE_SIZE,
                order,
                filter,
            )
            .await
            {
                Ok(p) => DbEvent::TablePageLoaded {
                    columns: p.columns,
                    rows: p.rows,
                    total: p.total_rows,
                    page,
                },
                Err(e) => DbEvent::TableQueryFailed(e.to_string()),
            };
            let _ = tx.send(event);
        });
    }

    pub fn next_page(&mut self) {
        if self.grid.loading || !self.grid.paged || self.current_table.is_none() {
            return;
        }
        let next = self.grid.page + 1;
        if next * PAGE_SIZE >= self.grid.total_rows {
            self.status = "last page".to_string();
            return;
        }
        let (s, t) = self.current_table.clone().unwrap();
        self.load_table_page(s, t, next);
    }

    pub fn prev_page(&mut self) {
        if self.grid.loading || !self.grid.paged || self.grid.page == 0 || self.current_table.is_none() {
            return;
        }
        let (s, t) = self.current_table.clone().unwrap();
        self.load_table_page(s, t, self.grid.page - 1);
    }

    pub fn refresh_table(&mut self) {
        if let Some((s, t)) = self.current_table.clone() {
            self.load_table_page(s, t, self.grid.page);
        }
    }

    pub fn open_editor(&mut self) {
        self.editor_filter = false;
        self.editor.open();
    }

    pub fn open_filter_editor(&mut self) {
        if !self.grid.paged || self.current_table.is_none() {
            self.status = "open a table first".to_string();
            return;
        }
        self.editor_filter = true;
        self.editor.open();
        if let Some(f) = self.grid.filter.clone() {
            self.editor.set_text(f);
        }
    }

    pub fn run_editor_query(&mut self) {
        let text = self.editor.text().trim().to_string();
        self.editor.visible = false;

        if self.editor_filter {
            self.editor_filter = false;
            if !text.is_empty() {
                self.editor.push_history(text.clone());
                let _ = config::save_history(&self.editor.history);
            }
            if text.is_empty() {
                self.grid.filter = None;
                self.status = "filter cleared".to_string();
            } else {
                self.grid.filter = Some(text.clone());
                self.status = format!("filter: {text}");
            }
            if let Some((s, t)) = self.current_table.clone() {
                self.load_table_page(s, t, 0);
            }
            return;
        }

        if text.is_empty() {
            return;
        }
        self.editor.push_history(text.clone());
        let _ = config::save_history(&self.editor.history);

        let Some(pool) = self.pool.clone() else { return };
        self.focus = Focus::Main;
        self.focus_mode = false;
        self.grid.sort = None;
        self.grid.filter = None;
        self.metadata_open = false;
        self.current_table = None;
        self.grid.loading = true;
        self.grid.paged = false;
        self.grid.page = 0;
        self.grid.label = if text.chars().count() > 60 {
            let mut s: String = text.chars().take(57).collect();
            s.push('…');
            s
        } else {
            text.clone()
        };
        self.status = "running query…".to_string();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let event = match db::run_query(&pool, &text).await {
                Ok(p) => DbEvent::TablePageLoaded {
                    columns: p.columns,
                    rows: p.rows,
                    total: p.total_rows,
                    page: 0,
                },
                Err(e) => DbEvent::TableQueryFailed(e.to_string()),
            };
            let _ = tx.send(event);
        });
    }

    pub fn handle_db_event(&mut self, event: DbEvent) {
        match event {
            DbEvent::Connected(pool) => {
                self.connecting = false;
                self.pool = Some(pool.clone());
                self.screen = Screen::Main;
                self.status = "connected".to_string();
                self.loading_tables = true;

                let tx = self.tx.clone();
                tokio::spawn(async move {
                    let event = match db::load_tables(&pool).await {
                        Ok(tables) => DbEvent::TablesLoaded(tables),
                        Err(e) => DbEvent::TablesFailed(e.to_string()),
                    };
                    let _ = tx.send(event);
                });
            }
            DbEvent::ConnectFailed(msg) => {
                self.connecting = false;
                self.status = format!("connection failed: {msg}");
            }
            DbEvent::TablesLoaded(tables) => {
                self.loading_tables = false;
                let mut grouped: Vec<Schema> = Vec::new();
                for (schema, table) in tables {
                    match grouped.iter_mut().find(|s| s.name == schema) {
                        Some(s) => s.tables.push(table),
                        None => grouped.push(Schema {
                            name: schema,
                            tables: vec![table],
                        }),
                    }
                }
                self.sidebar.expanded = vec![true; grouped.len()];
                self.sidebar
                    .list
                    .select(if grouped.is_empty() { None } else { Some(0) });
                self.schemas = grouped;
            }
            DbEvent::TablesFailed(msg) => {
                self.loading_tables = false;
                self.status = format!("failed to load tables: {msg}");
            }
            DbEvent::TablePageLoaded {
                columns,
                rows,
                total,
                page,
            } => {
                self.grid.loading = false;
                if page == self.grid.page {
                    let label = self
                        .current_table
                        .as_ref()
                        .map(|(s, t)| format!("{s}.{t}"))
                        .unwrap_or_else(|| self.grid.label.clone());
                    self.grid.set_data(columns, rows, total, page, label);
                    self.focus_scroll = 0;
                    self.status = format!("{total} rows total");
                }
            }
            DbEvent::TableQueryFailed(msg) => {
                self.grid.loading = false;
                self.status = format!("query failed: {msg}");
            }
            DbEvent::MetadataLoaded { label, meta } => {
                self.loading_metadata = false;
                self.metadata = Some((label, meta));
            }
            DbEvent::MetadataFailed(msg) => {
                self.loading_metadata = false;
                self.metadata_open = false;
                self.status = format!("metadata failed: {msg}");
            }
        }
    }
}

fn csv_line<'a>(fields: impl Iterator<Item = &'a str>) -> String {
    let escaped: Vec<String> = fields.map(csv_escape).collect();
    let mut line = escaped.join(",");
    line.push('\n');
    line
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}