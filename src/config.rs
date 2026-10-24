use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ConnectionProfile {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub database: String,
    pub ssl_mode: String,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Config {
    pub connections: Vec<ConnectionProfile>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct History {
    pub queries: Vec<String>,
}

pub fn config_path() -> anyhow::Result<PathBuf> {
    let dir = dirs::config_dir().ok_or_else(|| anyhow::anyhow!("no config directory found"))?;
    Ok(dir.join("pluto").join("config.toml"))
}

pub fn load() -> anyhow::Result<Config> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(Config::default());
    }
    let contents = std::fs::read_to_string(path)?;
    Ok(toml::from_str(&contents)?)
}

pub fn save(config: &Config) -> anyhow::Result<()> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let contents = toml::to_string_pretty(config)?;
    std::fs::write(&path, contents)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

pub fn history_path() -> anyhow::Result<PathBuf> {
    let dir = dirs::data_local_dir().ok_or_else(|| anyhow::anyhow!("no data directory found"))?;
    Ok(dir.join("pluto").join("history.toml"))
}

pub fn load_history() -> Vec<String> {
    let Ok(path) = history_path() else {
        return Vec::new();
    };
    if !path.exists() {
        return Vec::new();
    }
    let Ok(contents) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(history) = toml::from_str::<History>(&contents) else {
        return Vec::new();
    };
    history.queries
}

pub fn save_history(queries: &[String]) -> anyhow::Result<()> {
    let path = history_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let recent: Vec<String> = queries.iter().rev().take(500).rev().cloned().collect();
    let contents = toml::to_string_pretty(&History { queries: recent })?;
    std::fs::write(&path, contents)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}