use ratatui::layout::Rect;
use ratatui::widgets::ListState;
use sqlx::PgPool;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::config::{self, ConnectionProfile};
use crate::db::{self, JoinInfo, Metadata};
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
    JoinsLoaded {
        base: (String, String),
        joins: Vec<JoinInfo>,
    },
    JoinsFailed(String),
    JoinColumnsLoaded {
        base: (String, String),
        target: (String, String),
        base_cols: Vec<String>,
        target_cols: Vec<String>,
    },
    JoinColumnsFailed(String),
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
pub struct JoinPicker {
    pub base: (String, String),
    pub joins: Vec<JoinInfo>,
    pub selected: usize,
    pub loaded: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinSide {
    Base,
    Target,
}

#[derive(Debug)]
pub struct PickPair {
    pub replace: Option<usize>,
    pub side: JoinSide,
    pub chosen: Option<String>,
    pub sel: usize,
}

#[derive(Debug)]
pub struct TableSelect {
    pub items: Vec<(String, String)>,
    pub selected: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildRow {
    Pair(usize),
    AddPair,
    BaseCol(usize),
    TargetCol(usize),
    Run,
}

#[derive(Debug)]
pub struct JoinBuilder {
    pub base: (String, String),
    pub target: Option<(String, String)>,
    pub pairs: Vec<(String, String)>,
    pub base_cols: Vec<(String, bool)>,
    pub target_cols: Vec<(String, bool)>,
    pub loading: bool,
    pub selected: usize,
    pub picking: Option<PickPair>,
    pub target_pick: Option<TableSelect>,
}

impl JoinBuilder {
    pub fn new(base: (String, String)) -> Self {
        Self {
            base,
            target: None,
            pairs: Vec::new(),
            base_cols: Vec::new(),
            target_cols: Vec::new(),
            loading: false,
            selected: 0,
            picking: None,
            target_pick: None,
        }
    }

    pub fn rows(&self) -> Vec<BuildRow> {
        let mut rows = Vec::new();
        for i in 0..self.pairs.len() {
            rows.push(BuildRow::Pair(i));
        }
        rows.push(BuildRow::AddPair);
        for i in 0..self.base_cols.len() {
            rows.push(BuildRow::BaseCol(i));
        }
        for i in 0..self.target_cols.len() {
            rows.push(BuildRow::TargetCol(i));
        }
        rows.push(BuildRow::Run);
        rows
    }

    fn clamp(&mut self) {
        let n = self.rows().len();
        self.selected = if n == 0 { 0 } else { self.selected.min(n - 1) };
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

    pub join_picker: Option<JoinPicker>,
    pub join_builder: Option<JoinBuilder>,

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
            join_picker: None,
            join_builder: None,
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

    pub fn open_join_picker(&mut self) {
        if !self.grid.paged || self.current_table.is_none() {
            self.status = "open a table first".to_string();
            return;
        }
        let base = self.current_table.clone().unwrap();
        let Some(pool) = self.pool.clone() else { return };
        self.join_picker = Some(JoinPicker {
            base: base.clone(),
            joins: Vec::new(),
            selected: 0,
            loaded: false,
        });
        self.status = "looking for foreign keys…".to_string();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let event = match db::find_joins(&pool, &base.0, &base.1).await {
                Ok(joins) => DbEvent::JoinsLoaded { base, joins },
                Err(e) => DbEvent::JoinsFailed(e.to_string()),
            };
            let _ = tx.send(event);
        });
    }

    pub fn join_picker_next(&mut self) {
        if let Some(p) = &mut self.join_picker {
            let len = p.joins.len() + 1; // +1 = "+ any table…" row
            p.selected = (p.selected + 1) % len;
        }
    }

    pub fn join_picker_prev(&mut self) {
        if let Some(p) = &mut self.join_picker {
            let len = p.joins.len() + 1;
            p.selected = if p.selected == 0 {
                len - 1
            } else {
                p.selected - 1
            };
        }
    }

    pub fn select_join(&mut self) {
        let Some(picker) = &self.join_picker else {
            return;
        };
        let base = picker.base.clone();

        // last row: "+ any table…" — manual join, no FK needed
        if picker.selected >= picker.joins.len() {
            let mut items: Vec<(String, String)> = Vec::new();
            for s in &self.schemas {
                for t in &s.tables {
                    items.push((s.name.clone(), t.clone()));
                }
            }
            let mut builder = JoinBuilder::new(base);
            builder.target_pick = Some(TableSelect { items, selected: 0 });
            self.join_builder = Some(builder);
            self.status = "pick a table to join…".to_string();
            return;
        }

        let Some(join) = picker.joins.get(picker.selected).cloned() else {
            return;
        };
        // normalize pairs to (base_col, target_col)
        let pairs: Vec<(String, String)> = if join.outgoing {
            join.pairs.clone()
        } else {
            join.pairs
                .iter()
                .map(|(a, b)| (b.clone(), a.clone()))
                .collect()
        };
        let target = (join.schema.clone(), join.table.clone());
        self.open_join_builder(base, target, pairs);
    }

    fn open_join_builder(
        &mut self,
        base: (String, String),
        target: (String, String),
        pairs: Vec<(String, String)>,
    ) {
        let mut builder = JoinBuilder::new(base);
        builder.target = Some(target);
        builder.pairs = pairs;
        self.join_builder = Some(builder);
        self.fetch_join_columns();
    }

    fn fetch_join_columns(&mut self) {
        let Some(b) = &self.join_builder else { return };
        let base = b.base.clone();
        let Some(target) = b.target.clone() else {
            return;
        };
        let Some(pool) = self.pool.clone() else { return };
        if let Some(b) = &mut self.join_builder {
            b.loading = true;
        }
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let res = async {
                let base_cols = db::table_columns(&pool, &base.0, &base.1).await?;
                let target_cols = db::table_columns(&pool, &target.0, &target.1).await?;
                Ok::<_, anyhow::Error>((base_cols, target_cols))
            }
            .await;
            let event = match res {
                Ok((base_cols, target_cols)) => DbEvent::JoinColumnsLoaded {
                    base,
                    target,
                    base_cols,
                    target_cols,
                },
                Err(e) => DbEvent::JoinColumnsFailed(e.to_string()),
            };
            let _ = tx.send(event);
        });
    }

    pub fn builder_back(&mut self) {
        let Some(b) = &mut self.join_builder else { return };
        if b.picking.is_some() {
            b.picking = None;
            return;
        }
        self.join_builder = None;
    }

    pub fn builder_next(&mut self) {
        let Some(b) = &mut self.join_builder else { return };
        if let Some(t) = &mut b.target_pick {
            if !t.items.is_empty() {
                t.selected = (t.selected + 1) % t.items.len();
            }
            return;
        }
        if let Some(p) = &mut b.picking {
            let len = match p.side {
                JoinSide::Base => b.base_cols.len(),
                JoinSide::Target => b.target_cols.len(),
            };
            if len > 0 {
                p.sel = (p.sel + 1) % len;
            }
            return;
        }
        let n = b.rows().len();
        if n > 0 {
            b.selected = (b.selected + 1).min(n - 1);
        }
    }

    pub fn builder_prev(&mut self) {
        let Some(b) = &mut self.join_builder else { return };
        if let Some(t) = &mut b.target_pick {
            if !t.items.is_empty() {
                t.selected = if t.selected == 0 {
                    t.items.len() - 1
                } else {
                    t.selected - 1
                };
            }
            return;
        }
        if let Some(p) = &mut b.picking {
            let len = match p.side {
                JoinSide::Base => b.base_cols.len(),
                JoinSide::Target => b.target_cols.len(),
            };
            if len > 0 {
                p.sel = if p.sel == 0 { len - 1 } else { p.sel - 1 };
            }
            return;
        }
        if b.selected > 0 {
            b.selected -= 1;
        }
    }

    pub fn builder_toggle_col(&mut self) {
        let Some(b) = &mut self.join_builder else { return };
        if b.target_pick.is_some() || b.picking.is_some() {
            return;
        }
        let row = b.rows().get(b.selected).copied();
        match row {
            Some(BuildRow::BaseCol(i)) => b.base_cols[i].1 = !b.base_cols[i].1,
            Some(BuildRow::TargetCol(i)) => b.target_cols[i].1 = !b.target_cols[i].1,
            _ => {}
        }
    }

    pub fn builder_delete_pair(&mut self) {
        let Some(b) = &mut self.join_builder else { return };
        if b.target_pick.is_some() || b.picking.is_some() {
            return;
        }
        let row = b.rows().get(b.selected).copied();
        if let Some(BuildRow::Pair(i)) = row {
            b.pairs.remove(i);
            b.clamp();
            self.status = "join pair removed".to_string();
        }
    }

    pub fn builder_activate(&mut self) {
        let Some(b) = &mut self.join_builder else { return };

        // confirm manual target table
        if b.target_pick.is_some() {
            let ts = b.target_pick.take().unwrap();
            b.target = ts.items.get(ts.selected).cloned();
            b.pairs.clear();
            b.selected = 0;
            drop(b);
            self.fetch_join_columns();
            return;
        }

        // confirm a column in a pair pick
        if b.picking.is_some() {
            let p = b.picking.take().unwrap();
            let name = match p.side {
                JoinSide::Base => b.base_cols.get(p.sel).map(|(n, _)| n.clone()),
                JoinSide::Target => b.target_cols.get(p.sel).map(|(n, _)| n.clone()),
            };
            let Some(name) = name else { return };
            match p.side {
                JoinSide::Base => {
                    b.picking = Some(PickPair {
                        replace: p.replace,
                        side: JoinSide::Target,
                        chosen: Some(name),
                        sel: 0,
                    });
                }
                JoinSide::Target => {
                    let base_col = p.chosen.unwrap_or_default();
                    if let Some(i) = p.replace {
                        if let Some(pair) = b.pairs.get_mut(i) {
                            *pair = (base_col, name);
                        }
                    } else {
                        b.pairs.push((base_col, name));
                    }
                    b.clamp();
                    self.status = "join pair updated".to_string();
                }
            }
            return;
        }

        if b.loading {
            self.status = "loading columns…".to_string();
            return;
        }
        let row = b.rows().get(b.selected).copied();
        match row {
            Some(BuildRow::Pair(i)) => {
                b.picking = Some(PickPair {
                    replace: Some(i),
                    side: JoinSide::Base,
                    chosen: None,
                    sel: 0,
                });
            }
            Some(BuildRow::AddPair) => {
                b.picking = Some(PickPair {
                    replace: None,
                    side: JoinSide::Base,
                    chosen: None,
                    sel: 0,
                });
            }
            Some(BuildRow::BaseCol(i)) => b.base_cols[i].1 = !b.base_cols[i].1,
            Some(BuildRow::TargetCol(i)) => b.target_cols[i].1 = !b.target_cols[i].1,
            Some(BuildRow::Run) => {
                drop(b);
                self.run_join();
            }
            None => {}
        }
    }

    pub fn run_join(&mut self) {
        let Some(b) = &self.join_builder else { return };
        if b.picking.is_some() || b.target_pick.is_some() {
            return;
        }
        let Some(target) = b.target.clone() else {
            self.status = "pick a table to join first".to_string();
            return;
        };
        if b.pairs.is_empty() {
            self.status = "add at least one join column pair".to_string();
            return;
        }
        if b.pairs.iter().any(|(a, c)| a.is_empty() || c.is_empty()) {
            self.status = "join pair has an empty column".to_string();
            return;
        }
        let base_cols: Vec<String> = b
            .base_cols
            .iter()
            .filter(|c| c.1)
            .map(|c| c.0.clone())
            .collect();
        let target_cols: Vec<String> = b
            .target_cols
            .iter()
            .filter(|c| c.1)
            .map(|c| c.0.clone())
            .collect();
        if base_cols.is_empty() && target_cols.is_empty() {
            self.status = "include at least one column".to_string();
            return;
        }
        let sql = db::build_join_query(&b.base, &target, &base_cols, &target_cols, &b.pairs);
        let label = format!("{}.{} ⋈ {}.{}", b.base.0, b.base.1, target.0, target.1);
        self.join_builder = None;
        self.join_picker = None;
        self.execute_query(sql, label);
    }

    fn execute_query(&mut self, sql: String, label: String) {
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
        self.grid.label = label;
        self.status = "running query…".to_string();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let event = match db::run_query(&pool, &sql).await {
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

        let label = truncate_label(&text);
        self.execute_query(text, label);
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
            DbEvent::JoinsLoaded { base, joins } => {
                if let Some(p) = &mut self.join_picker {
                    if p.base == base {
                        p.loaded = true;
                        p.joins = joins;
                        p.selected = 0;
                        if p.joins.is_empty() {
                            self.status = format!(
                                "no foreign keys on {}.{} — pick any table to join",
                                base.0, base.1
                            );
                        }
                    }
                }
            }
            DbEvent::JoinsFailed(msg) => {
                self.join_picker = None;
                self.status = format!("join lookup failed: {msg}");
            }
            DbEvent::JoinColumnsLoaded {
                base,
                target,
                base_cols,
                target_cols,
            } => {
                if let Some(b) = &mut self.join_builder {
                    if b.base == base && b.target.as_ref() == Some(&target) {
                        b.base_cols = base_cols.into_iter().map(|c| (c, true)).collect();
                        b.target_cols = target_cols.into_iter().map(|c| (c, true)).collect();
                        b.loading = false;
                        b.selected = 0;
                        b.clamp();
                        self.status = "configure the join — space toggles columns, r runs".to_string();
                    }
                }
            }
            DbEvent::JoinColumnsFailed(msg) => {
                self.join_builder = None;
                self.status = format!("join builder failed: {msg}");
            }
        }
    }
}

fn truncate_label(sql: &str) -> String {
    if sql.chars().count() > 60 {
        let mut s: String = sql.chars().take(57).collect();
        s.push('…');
        s
    } else {
        sql.to_string()
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
