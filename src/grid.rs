use ratatui::widgets::TableState;
use unicode_width::UnicodeWidthStr;

pub const PAGE_SIZE: usize = 100;
pub const MAX_COL_WIDTH: usize = 32;
pub const MIN_COL_WIDTH: usize = 4;

#[derive(Debug)]
pub struct Grid {
    pub table_state: TableState,
    pub selected_col: usize,
    pub columns: Vec<String>,
    pub col_widths: Vec<usize>,
    pub rows: Vec<Vec<Option<String>>>,
    pub total_rows: usize,
    pub page: usize,
    pub loading: bool,
    pub label: String,
    pub viewport_rows: usize,
}

impl Grid {
    pub fn new() -> Self {
        let mut table_state = TableState::default();
        table_state.select(None);
        Self {
            table_state,
            selected_col: 0,
            columns: Vec::new(),
            col_widths: Vec::new(),
            rows: Vec::new(),
            total_rows: 0,
            page: 0,
            loading: false,
            label: String::new(),
            viewport_rows: 10,
        }
    }

    pub fn set_data(
        &mut self,
        columns: Vec<String>,
        rows: Vec<Vec<Option<String>>>,
        total: usize,
        page: usize,
        label: String,
    ) {
        self.col_widths = compute_widths(&columns, &rows);
        self.columns = columns;
        self.rows = rows;
        self.total_rows = total;
        self.page = page;
        self.label = label;
        self.selected_col = 0;
        self.table_state
            .select(if self.rows.is_empty() { None } else { Some(0) });
    }

    pub fn selected_row(&self) -> usize {
        self.table_state.selected().unwrap_or(0)
    }

    pub fn selected_cell(&self) -> Option<&str> {
        self.rows
            .get(self.selected_row())
            .and_then(|r| r.get(self.selected_col))
            .and_then(|c| c.as_deref())
    }

    pub fn next_row(&mut self) {
        if self.selected_row() + 1 < self.rows.len() {
            self.table_state.select(Some(self.selected_row() + 1));
        }
    }

    pub fn prev_row(&mut self) {
        let r = self.selected_row();
        self.table_state.select(Some(r.saturating_sub(1)));
    }

    pub fn next_col(&mut self) {
        if self.selected_col + 1 < self.columns.len() {
            self.selected_col += 1;
        }
    }

    pub fn prev_col(&mut self) {
        self.selected_col = self.selected_col.saturating_sub(1);
    }

    pub fn top(&mut self) {
        if !self.rows.is_empty() {
            self.table_state.select(Some(0));
        }
    }

    pub fn bottom(&mut self) {
        if !self.rows.is_empty() {
            self.table_state.select(Some(self.rows.len() - 1));
        }
    }

    pub fn half_page_down(&mut self) {
        let step = (self.viewport_rows / 2).max(1);
        let r = (self.selected_row() + step).min(self.rows.len().saturating_sub(1));
        self.table_state.select(Some(r));
    }

    pub fn half_page_up(&mut self) {
        let step = (self.viewport_rows / 2).max(1);
        self.table_state
            .select(Some(self.selected_row().saturating_sub(step)));
    }
}

fn display_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

fn compute_widths(columns: &[String], rows: &[Vec<Option<String>>]) -> Vec<usize> {
    let mut widths: Vec<usize> = columns
        .iter()
        .map(|c| display_width(c).clamp(MIN_COL_WIDTH, MAX_COL_WIDTH))
        .collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            let w = display_width(cell.as_deref().unwrap_or("NULL")).min(MAX_COL_WIDTH);
            if i < widths.len() && w > widths[i] {
                widths[i] = w;
            }
        }
    }
    widths
}