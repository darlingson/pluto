use ratatui::widgets::TableState;
use unicode_width::UnicodeWidthStr;

pub const PAGE_SIZE: usize = 100;
pub const MAX_COL_WIDTH: usize = 32;
pub const MAX_MANUAL_WIDTH: usize = 80;
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
    pub paged: bool,
    pub label: String,
    pub viewport_rows: usize,
    pub sort: Option<(usize, bool)>,
    pub filter: Option<String>,
    pub base_widths: Vec<usize>,
    pub manual_widths: Vec<Option<usize>>,
    pub col_hidden: Vec<bool>,
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
            paged: true,
            label: String::new(),
            viewport_rows: 10,
            sort: None,
            filter: None,
            base_widths: Vec::new(),
            manual_widths: Vec::new(),
            col_hidden: Vec::new(),
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
        let computed = compute_widths(&columns, &rows);
        self.base_widths = computed.clone();
        self.col_widths = computed;
        let n = columns.len();
        self.manual_widths = vec![None; n];
        self.col_hidden = vec![false; n];
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

    pub fn hidden(&self, ci: usize) -> bool {
        self.col_hidden.get(ci).copied().unwrap_or(false)
    }

    pub fn widen_col(&mut self) {
        let Some(&cur) = self.col_widths.get(self.selected_col) else {
            return;
        };
        let w = (cur + 2).min(MAX_MANUAL_WIDTH);
        self.col_widths[self.selected_col] = w;
        if let Some(m) = self.manual_widths.get_mut(self.selected_col) {
            *m = Some(w);
        }
    }

    pub fn narrow_col(&mut self) {
        let Some(&cur) = self.col_widths.get(self.selected_col) else {
            return;
        };
        let w = cur.saturating_sub(2).max(MIN_COL_WIDTH);
        self.col_widths[self.selected_col] = w;
        if let Some(m) = self.manual_widths.get_mut(self.selected_col) {
            *m = Some(w);
        }
    }

    pub fn toggle_hide_col(&mut self) {
        let ci = self.selected_col;
        if ci >= self.col_hidden.len() {
            return;
        }
        let visible = (0..self.columns.len()).filter(|&i| !self.hidden(i)).count();
        if !self.hidden(ci) && visible <= 1 {
            return;
        }
        self.col_hidden[ci] = !self.hidden(ci);
    }

    pub fn reset_layout(&mut self) {
        self.col_widths = self.base_widths.clone();
        for m in &mut self.manual_widths {
            *m = None;
        }
        for h in &mut self.col_hidden {
            *h = false;
        }
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
        let mut c = self.selected_col;
        while c + 1 < self.columns.len() {
            c += 1;
            if !self.hidden(c) {
                self.selected_col = c;
                return;
            }
        }
    }

    pub fn prev_col(&mut self) {
        let mut c = self.selected_col;
        while c > 0 {
            c -= 1;
            if !self.hidden(c) {
                self.selected_col = c;
                return;
            }
        }
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