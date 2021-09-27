mod db;
mod investment;
mod scraper;

use std::str::FromStr;
use std::time::Duration;

use atium::logger::Logger;
use atium::respond::RespondRequestExt;
use atium::state::State;
use atium::{async_trait, endpoint, Handler, Request, StatusCode};
use chrono::Utc;
use color_eyre::eyre;
use cron::Schedule;
use serde_json::json;
use sqlx::sqlite::{SqliteArguments, SqliteRow};
use sqlx::{Arguments, Row, SqlitePool};

struct Credentials {
    username: String,
    password: String,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    tracing_subscriber::fmt()
        .pretty()
        .with_env_filter("info,sqlx::query=warn")
        .init();

    let credentials = load_credentials();
    let db_path = std::env::var("VANGUARD_DB").unwrap_or_else(|_| "vanguard.db".to_owned());
    let db = db::connect(&db_path).await?;

    let cron = "0 0 0,12 * * * *";
    let schedule = Schedule::from_str(cron).expect("invalid cron expression");

    tokio::spawn(job_runner(schedule, credentials, db.clone()));

    let app = atium::compose!(
        Logger::default(),
        State(db.clone()),
        ErrorHandler,
        get_investments
    );

    atium::run(([0, 0, 0, 0], 8000), app).await?;

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

struct ErrorHandler;

#[async_trait]
impl Handler for ErrorHandler {
    async fn run(&self, req: Request, next: &dyn atium::Next) -> Request {
        let mut req = next.run(req).await;

        if let Some(e) = req.take_ext::<eyre::Report>() {
            req.respond(StatusCode::INTERNAL_SERVER_ERROR)
                .body(e.to_string());
        }

        req
    }
}

#[endpoint]
async fn get_investments(req: &mut Request) -> eyre::Result<()> {
    let db: &SqlitePool = req.ext().unwrap();

    let sql = "
        SELECT
            name,
            CAST(ongoing_charge AS TEXT) AS ongoing_charge,
            CAST(units AS TEXT) AS units,
            CAST(avg_unit_cost AS TEXT) AS avg_unit_cost,
            CAST(last_price AS TEXT) AS last_price,
            CAST(total_cost AS TEXT) AS total_cost,
            CAST(value AS TEXT) AS value,
            CAST(change AS TEXT) AS change
        FROM investments
    ";

    let investments = sqlx::query(sql)
        .map(|row: SqliteRow| {
            json!({
                "name": row.get::<String, _>("name"),
                "ongoing_charge": row.get::<String, _>("ongoing_charge"),
                "units": row.get::<String, _>("units"),
                "avg_unit_cost": row.get::<String, _>("avg_unit_cost"),
                "last_price": row.get::<String, _>("last_price"),
                "total_cost": row.get::<String, _>("total_cost"),
                "value": row.get::<String, _>("value"),
                "change": row.get::<String, _>("change"),
            })
        })
        .fetch_all(db)
        .await?;

    req.ok().json(&investments)?;

    Ok(())
}

async fn job_runner(schedule: Schedule, credentials: Credentials, db: SqlitePool) {
    for time in schedule.upcoming(Utc) {
        tracing::info!("next job scheduled for {}", time);
        tokio::time::sleep((time - Utc::now()).to_std().unwrap()).await;

        if let Err(e) = run_job(&credentials, &db).await {
            tracing::error!("{}", e);
        }
    }
}

async fn run_job(credentials: &Credentials, db: &SqlitePool) -> eyre::Result<()> {
    let results = loop {
        match scraper::scrape_investment_data(&credentials.username, &credentials.password).await {
            Ok(results) => break results,
            Err(e) => {
                tracing::error!("job failed: {}", e);
                tracing::info!("retrying in 5 mins");
                tokio::time::sleep(Duration::from_secs(300)).await;
            }
        }
    };

    tracing::info!("job completed successfully");

    let insert = "
        INSERT INTO investments (
            name,
            ongoing_charge,
            units,
            avg_unit_cost,
            last_price,
            total_cost,
            value,
            change
        )
    ";

    let values_placeholder = "(
        ?,
        CAST(? AS DECIMAL),
        CAST(? AS DECIMAL),
        CAST(? AS DECIMAL),
        CAST(? AS DECIMAL),
        CAST(? AS DECIMAL),
        CAST(? AS DECIMAL),
        CAST(? AS DECIMAL)
    )";

    let values = vec![values_placeholder; results.len()];
    let sql = format!("{} VALUES {}", insert, values.join(","));

    let mut args = SqliteArguments::default();

    for result in results {
        args.add(result.name);
        args.add(result.ongoing_charge.to_string());
        args.add(result.units.to_string());
        args.add(result.avg_unit_cost.to_string());
        args.add(result.last_price.to_string());
        args.add(result.total_cost.to_string());
        args.add(result.value.to_string());
        args.add(result.change.to_string());
    }

    sqlx::query_with(&sql, args).execute(db).await?;

    Ok(())
}
