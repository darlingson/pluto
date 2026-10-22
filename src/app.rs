use ratatui::widgets::ListState;
use sqlx::PgPool;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::config::{self, ConnectionProfile};
use crate::db;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Profiles,
    Main,
}

#[derive(Debug)]
pub enum DbEvent {
    Connected(PgPool),
    ConnectFailed(String),
    TablesLoaded(Vec<(String, String)>),
    TablesFailed(String),
}

#[derive(Debug)]
pub struct Schema {
    pub name: String,
    pub tables: Vec<String>,
}

#[derive(Debug)]
pub struct App {
    pub screen: Screen,
    pub running: bool,
    pub status: String,

    pub profiles: Vec<ConnectionProfile>,
    pub profile_list: ListState,
    pub connecting: bool,

    pub pool: Option<PgPool>,
    pub schemas: Vec<Schema>,
    pub loading_tables: bool,

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
        Self {
            screen: Screen::Profiles,
            running: true,
            status,
            profiles,
            profile_list,
            connecting: false,
            pool: None,
            schemas: Vec::new(),
            loading_tables: false,
            tx,
            rx,
        }
    }

    pub fn quit(&mut self) {
        self.running = false;
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
                self.schemas = grouped;
            }
            DbEvent::TablesFailed(msg) => {
                self.loading_tables = false;
                self.status = format!("failed to load tables: {msg}");
            }
        }
    }
}