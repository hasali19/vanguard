mod db;
mod investment;
mod scraper;

use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;

use axum::http::StatusCode;
use axum::routing::get;
use axum::{Extension, Json, Router, Server};
use chrono::Utc;
use color_eyre::eyre::{self, eyre};
use cron::Schedule;
use db::Db;
use investment::Investment;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

struct Credentials {
    username: String,
    password: String,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    tracing_subscriber::fmt()
        .pretty()
        .with_env_filter(if std::env::var_os("RUST_LOG").is_some() {
            EnvFilter::from_default_env()
        } else {
            EnvFilter::from("info,sqlx::query=warn")
        })
        .init();

    let credentials = load_credentials();
    let db_path = std::env::var("VANGUARD_DB").unwrap_or_else(|_| "vanguard.db".to_owned());
    let db = db::connect(&db_path).await?;

    let cron = "0 0 0,12 * * * *";
    let schedule = Schedule::from_str(cron).unwrap();

    tokio::spawn(job_runner(schedule, credentials, db.clone()));

    let app = Router::new()
        .route("/api/investments", get(get_investments))
        .layer(TraceLayer::new_for_http())
        .layer(Extension(db.clone()));

    Server::bind(&SocketAddr::from(([0, 0, 0, 0], 8000)))
        .serve(app.into_make_service())
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c()
                .await
                .expect("failed to install ctrl+c signal handler")
        })
        .await?;

    db.close().await;

    Ok(())
}

fn load_credentials() -> Credentials {
    Credentials {
        username: std::env::var("VANGUARD_USERNAME")
            .expect("VANGUARD_USERNAME env var is required"),
        password: std::env::var("VANGUARD_PASSWORD")
            .expect("VANGUARD_PASSWORD env var is required"),
    }
}

async fn get_investments(db: Db) -> Result<Json<Vec<Investment>>, StatusCode> {
    Ok(Json(Investment::all(&db).await.unwrap()))
}

#[tracing::instrument(skip(schedule, credentials, db))]
async fn job_runner(schedule: Schedule, credentials: Credentials, db: Db) {
    for time in schedule.upcoming(Utc) {
        tracing::info!("next job scheduled for {}", time);
        tokio::time::sleep((time - Utc::now()).to_std().unwrap()).await;

        if let Err(e) = run_job(&credentials, &db).await {
            tracing::error!("{}", e);
        }
    }
}

#[tracing::instrument(skip(credentials, db))]
async fn run_job(credentials: &Credentials, db: &Db) -> eyre::Result<()> {
    let mut tries = 0;
    let results = loop {
        match scraper::scrape_investment_data(&credentials.username, &credentials.password).await {
            Ok(results) => break results,
            Err(e) => {
                tracing::error!("job failed: {}", e);
                tries += 1;
                if tries < 3 {
                    tracing::info!("retrying in 5 mins");
                    tokio::time::sleep(Duration::from_secs(300)).await;
                } else {
                    return Err(eyre!("max retries attempted"));
                }
            }
        }
    };

    tracing::info!("job completed successfully");

    Investment::insert(db, results).await?;

    Ok(())
}
