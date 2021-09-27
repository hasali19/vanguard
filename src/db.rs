use color_eyre::eyre;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::SqlitePool;

pub async fn connect(path: &str) -> eyre::Result<SqlitePool> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true);

    let pool = SqlitePool::connect_with(options).await?;

    sqlx::query(include_str!("schema.sql"))
        .execute(&pool)
        .await?;

    Ok(pool)
}
