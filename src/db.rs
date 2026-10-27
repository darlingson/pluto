use sqlx::postgres::{PgConnectOptions, PgRow, PgSslMode};
use sqlx::{Column, Executor, PgPool, Row, TypeInfo, ValueRef};

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

    let mut columns: Vec<String> = if let Some(first) = result_rows.first() {
        first.columns().iter().map(|c| c.name().to_string()).collect()
    } else {
        match pool.describe(sql).await {
            Ok(d) => d.columns().iter().map(|c| c.name().to_string()).collect(),
            Err(_) => Vec::new(),
        }
    };

    // dedupe column names (e.g. SELECT b.*, t.* gives two "Id"s)
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for c in &mut columns {
        let n = seen.entry(c.clone()).or_insert(0);
        if *n > 0 {
            *c = format!("{c}_{}", *n + 1);
        }
        *n += 1;
    }

    let rows: Vec<Vec<Option<String>>> = result_rows
        .iter()
        .map(|row| {
            (0..columns.len())
                .map(|i| decode_cell(row, i))
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

// raw_sql runs on the simple protocol, so Postgres returns every value in
// text format on the wire. Reading the raw value side-steps sqlx's
// type-compatibility check and renders any column type (int4, bool,
// timestamptz, arrays, ...) the way psql would show it.
fn decode_cell(row: &PgRow, i: usize) -> Option<String> {
    match row.try_get_raw(i) {
        Ok(raw) => {
            if raw.is_null() {
                None
            } else {
                match raw.as_str() {
                    Ok(s) => Some(s.to_owned()),
                    // only reachable in binary format, which raw_sql never uses
                    Err(_) => Some(format!("«{}»", raw.type_info().name())),
                }
            }
        }
        Err(e) => Some(format!("«decode error: {e}»")),
    }
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

#[derive(Debug, Clone)]
pub struct JoinInfo {
    pub outgoing: bool,
    pub schema: String,
    pub table: String,
    pub pairs: Vec<(String, String)>,
}

pub async fn find_joins(
    pool: &PgPool,
    schema: &str,
    table: &str,
) -> anyhow::Result<Vec<JoinInfo>> {
    let s = quote_literal(schema);
    let t = quote_literal(table);

    let fk_sql = |base_is_source: bool| {
        let filter = if base_is_source {
            format!("ns.nspname = {s} AND cls.relname = {t}")
        } else {
            format!("ns_ref.nspname = {s} AND cls_ref.relname = {t}")
        };
        format!(
            "SELECT con.conname AS cname,
                    ns.nspname AS child_schema, cls.relname AS child_table,
                    ns_ref.nspname AS ref_schema, cls_ref.relname AS ref_table,
                    col_from.attname AS from_col, col_to.attname AS to_col
             FROM pg_constraint con
             JOIN pg_class cls ON cls.oid = con.conrelid
             JOIN pg_namespace ns ON ns.oid = cls.relnamespace
             JOIN pg_class cls_ref ON cls_ref.oid = con.confrelid
             JOIN pg_namespace ns_ref ON ns_ref.oid = cls_ref.relnamespace
             JOIN unnest(con.conkey) WITH ORDINALITY AS k(attnum, ord) ON TRUE
             JOIN unnest(con.confkey) WITH ORDINALITY AS r(attnum, ord) ON r.ord = k.ord
             JOIN pg_attribute col_from ON col_from.attrelid = cls.oid AND col_from.attnum = k.attnum
             JOIN pg_attribute col_to ON col_to.attrelid = cls_ref.oid AND col_to.attnum = r.attnum
             WHERE con.contype = 'f'
               AND con.conrelid <> con.confrelid
               AND {filter}
             ORDER BY con.conname, k.ord"
        )
    };

    let mut joins: Vec<JoinInfo> = Vec::new();

    // outgoing: base (child side) references other tables
    let outgoing = run_query(pool, &fk_sql(true)).await?;
    for row in &outgoing.rows {
        let get = |i: usize| row.get(i).and_then(|c| c.clone()).unwrap_or_default();
        let (schema, table) = (get(3), get(4)); // ref side
        let pair = (get(5), get(6));
        if let Some(j) = joins
            .iter_mut()
            .find(|j| j.outgoing && j.schema == schema && j.table == table)
        {
            j.pairs.push(pair);
        } else {
            joins.push(JoinInfo {
                outgoing: true,
                schema,
                table,
                pairs: vec![pair],
            });
        }
    }

    // incoming: other tables (child side) reference base
    let incoming = run_query(pool, &fk_sql(false)).await?;
    for row in &incoming.rows {
        let get = |i: usize| row.get(i).and_then(|c| c.clone()).unwrap_or_default();
        let (schema, table) = (get(1), get(2)); // child side
        let pair = (get(5), get(6));
        if let Some(j) = joins
            .iter_mut()
            .find(|j| !j.outgoing && j.schema == schema && j.table == table)
        {
            j.pairs.push(pair);
        } else {
            joins.push(JoinInfo {
                outgoing: false,
                schema,
                table,
                pairs: vec![pair],
            });
        }
    }

    Ok(joins)
}

pub fn build_join_query(base_schema: &str, base_table: &str, join: &JoinInfo) -> String {
    let base = quote_ident(base_schema, base_table);
    let target = quote_ident(&join.schema, &join.table);
    let on = join
        .pairs
        .iter()
        .map(|(a, b)| {
            if join.outgoing {
                format!("b.{} = t.{}", quote_col(a), quote_col(b))
            } else {
                format!("t.{} = b.{}", quote_col(a), quote_col(b))
            }
        })
        .collect::<Vec<_>>()
        .join(" AND ");
    format!("SELECT b.*, t.* FROM {base} b JOIN {target} t ON {on}")
}