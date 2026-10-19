use sqlx::{Row, PgPool};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let url = std::env::var("DATABASE_URL")?;
    let pool = PgPool::connect(&url).await?;

    let rows = sqlx::query(
        "SELECT table_schema, table_name
         FROM information_schema.tables
         WHERE table_type = 'BASE TABLE'
           AND table_schema NOT IN ('pg_catalog', 'information_schema')
         ORDER BY table_schema, table_name",
    )
    .fetch_all(&pool)
    .await?;

    for row in &rows {
        let schema: String = row.get("table_schema");
        let name: String = row.get("table_name");
        println!("{schema}.{name}");
    }

    Ok(())
}
