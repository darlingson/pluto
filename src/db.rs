use sqlx::postgres::{PgConnectOptions, PgSslMode};
use sqlx::{PgPool, Row};

use crate::config::ConnectionProfile;

fn ssl_mode(s: &str) -> PgSslMode {
    match s {
        "require" => PgSslMode::Require,
        "prefer" => PgSslMode::Prefer,
        _ => PgSslMode::Disable,
    }
}

pub async fn connect(profile: &ConnectionProfile) -> anyhow::Result<PgPool> {
    let opts = PgConnectOptions::new()
        .host(&profile.host)
        .port(profile.port)
        .username(&profile.user)
        .password(&profile.password)
        .database(&profile.database)
        .ssl_mode(ssl_mode(&profile.ssl_mode));
    Ok(PgPool::connect_with(opts).await?)
}

pub async fn load_tables(pool: &PgPool) -> anyhow::Result<Vec<(String, String)>> {
    let rows = sqlx::query(
        "SELECT table_schema, table_name
         FROM information_schema.tables
         WHERE table_type = 'BASE TABLE'
           AND table_schema NOT IN ('pg_catalog', 'information_schema')
         ORDER BY table_schema, table_name",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|r| (r.get("table_schema"), r.get("table_name")))
        .collect())
}