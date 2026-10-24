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

fn quote_col(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

fn quote_ident(schema: &str, table: &str) -> String {
    format!("{}.{}", quote_col(schema), quote_col(table))
}

fn quote_literal(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
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

#[derive(Debug)]
pub struct TablePage {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Option<String>>>,
    pub total_rows: usize,
}

#[derive(Debug)]
pub struct Metadata {
    pub columns: TablePage,
    pub indexes: TablePage,
}

pub async fn run_query(pool: &PgPool, sql: &str) -> anyhow::Result<TablePage> {
    let result_rows = sqlx::raw_sql(sql).fetch_all(pool).await?;

    let columns: Vec<String> = if let Some(first) = result_rows.first() {
        first.columns().iter().map(|c| c.name().to_string()).collect()
    } else {
        match pool.describe(sql).await {
            Ok(d) => d.columns().iter().map(|c| c.name().to_string()).collect(),
            Err(_) => Vec::new(),
        }
    };

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

    let total_rows = rows.len();
    Ok(TablePage {
        columns,
        rows,
        total_rows,
    })
}

#[allow(clippy::too_many_arguments)]
pub async fn fetch_table_page(
    pool: &PgPool,
    schema: &str,
    table: &str,
    page: usize,
    page_size: usize,
    order: Option<(String, bool)>,
    filter: Option<String>,
) -> anyhow::Result<TablePage> {
    let ident = quote_ident(schema, table);
    let where_clause = filter.map(|f| format!(" WHERE {f}")).unwrap_or_default();
    let order_clause = order
        .map(|(col, asc)| format!(" ORDER BY {} {}", quote_col(&col), if asc { "ASC" } else { "DESC" }))
        .unwrap_or_default();
    let sql = format!(
        "SELECT * FROM {ident}{where_clause}{order_clause} LIMIT {page_size} OFFSET {}",
        page * page_size
    );
    let mut result = run_query(pool, &sql).await?;

    let count_sql = format!("SELECT COUNT(*) FROM {ident}{where_clause}");
    let total: i64 = sqlx::raw_sql(&count_sql)
        .fetch_one(pool)
        .await?
        .try_get(0)?;
    result.total_rows = total.max(0) as usize;
    Ok(result)
}

pub async fn load_metadata(pool: &PgPool, schema: &str, table: &str) -> anyhow::Result<Metadata> {
    let s = quote_literal(schema);
    let t = quote_literal(table);

    let cols_sql = format!(
        "SELECT c.column_name AS name,
                c.data_type AS data_type,
                c.is_nullable AS is_nullable,
                c.column_default AS column_default,
                EXISTS (
                    SELECT 1
                    FROM information_schema.table_constraints tc
                    JOIN information_schema.key_column_usage kcu
                      ON kcu.constraint_schema = tc.constraint_schema
                     AND kcu.constraint_name = tc.constraint_name
                    WHERE tc.constraint_type = 'PRIMARY KEY'
                      AND tc.table_schema = c.table_schema
                      AND tc.table_name = c.table_name
                      AND kcu.column_name = c.column_name
                ) AS is_pk
         FROM information_schema.columns c
         WHERE c.table_schema = {s} AND c.table_name = {t}
         ORDER BY c.ordinal_position"
    );
    let columns = run_query(pool, &cols_sql).await?;

    let idx_sql = format!(
        "SELECT indexname AS name, indexdef AS definition
         FROM pg_indexes
         WHERE schemaname = {s} AND tablename = {t}
         ORDER BY indexname"
    );
    let indexes = run_query(pool, &idx_sql).await?;

    Ok(Metadata { columns, indexes })
}