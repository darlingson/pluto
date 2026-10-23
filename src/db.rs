use sqlx::postgres::{PgConnectOptions, PgSslMode};
use sqlx::{Column, Executor, PgPool, Row};

use crate::config::ConnectionProfile;

fn ssl_mode(s: &str) -> PgSslMode {
    match s {
        "require" => PgSslMode::Require,
        "prefer" => PgSslMode::Prefer,
        _ => PgSslMode::Disable,
    }
}

fn quote_ident(schema: &str, table: &str) -> String {
    let q = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
    format!("{}.{}", q(schema), q(table))
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

pub struct TablePage {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Option<String>>>,
    pub total_rows: usize,
}

pub async fn fetch_table_page(
    pool: &PgPool,
    schema: &str,
    table: &str,
    page: usize,
    page_size: usize,
) -> anyhow::Result<TablePage> {
    let ident = quote_ident(schema, table);
    let offset = page * page_size;

     let describe = pool
        .describe(&format!("SELECT * FROM {ident} LIMIT 0"))
        .await?;
    let columns: Vec<String> = describe
        .columns()
        .iter()
        .map(|c| c.name().to_string())
        .collect();

    let sql = format!("SELECT * FROM {ident} LIMIT {page_size} OFFSET {offset}");
    let result_rows = sqlx::raw_sql(&sql).fetch_all(pool).await?;
    let rows: Vec<Vec<Option<String>>> = result_rows
        .iter()
        .map(|row| {
            (0..columns.len())
                .map(|i| match row.try_get::<Option<String>, _>(i) {
                    Ok(v) => v,
                    Err(e) => Some(format!("«decode error: {e}»")),
                })
                .collect()
        })
        .collect();

    let count_sql = format!("SELECT COUNT(*) FROM {ident}");
    let total: i64 = sqlx::raw_sql(&count_sql)
        .fetch_one(pool)
        .await?
        .try_get(0)?;

    Ok(TablePage {
        columns,
        rows,
        total_rows: total.max(0) as usize,
    })
}