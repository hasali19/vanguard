use std::ops::Deref;

use axum::async_trait;
use axum::extract::FromRequest;
use axum::http::StatusCode;
use color_eyre::eyre;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct Db(SqlitePool);

pub async fn connect(path: &str) -> eyre::Result<Db> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true);

    let pool = SqlitePool::connect_with(options).await?;

    sqlx::query(include_str!("schema.sql"))
        .execute(&pool)
        .await?;

    Ok(Db(pool))
}

impl Deref for Db {
    type Target = SqlitePool;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[async_trait]
impl<B: Send> FromRequest<B> for Db {
    type Rejection = (StatusCode, &'static str);

    async fn from_request(
        req: &mut axum::extract::RequestParts<B>,
    ) -> Result<Self, Self::Rejection> {
        req.extensions().get().cloned().ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Db not found in request extensions",
        ))
    }
}
