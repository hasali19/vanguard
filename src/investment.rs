use std::ops::Deref;
use std::str::FromStr;

use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::sqlite::{SqliteArguments, SqliteRow};
use sqlx::{Arguments, FromRow, Row};

use crate::db::Db;

#[derive(Debug, Serialize)]
pub struct Investment {
    pub name: String,
    pub ongoing_charge: Decimal,
    pub units: Decimal,
    pub avg_unit_cost: Decimal,
    pub last_price: Decimal,
    pub total_cost: Decimal,
    pub value: Decimal,
    pub change: Decimal,
}

impl<'r> FromRow<'r, SqliteRow> for Investment {
    fn from_row(row: &'r SqliteRow) -> Result<Self, sqlx::Error> {
        let get_decimal = |name: &str| {
            Decimal::from_str(row.try_get(name)?).map_err(|e| sqlx::Error::Decode(Box::new(e)))
        };

        Ok(Investment {
            name: row.try_get("name")?,
            ongoing_charge: get_decimal("ongoing_charge")?,
            units: get_decimal("units")?,
            avg_unit_cost: get_decimal("avg_unit_cost")?,
            last_price: get_decimal("last_price")?,
            total_cost: get_decimal("total_cost")?,
            value: get_decimal("value")?,
            change: get_decimal("change")?,
        })
    }
}

impl Investment {
    pub async fn all(db: &Db) -> sqlx::Result<Vec<Investment>> {
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

        sqlx::query_as(sql).fetch_all(db.deref()).await
    }

    pub async fn insert(db: &Db, items: Vec<Investment>) -> sqlx::Result<()> {
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

        let values = vec![values_placeholder; items.len()];
        let sql = format!("{} VALUES {}", insert, values.join(","));

        let mut args = SqliteArguments::default();

        for result in items {
            args.add(result.name);
            args.add(result.ongoing_charge.to_string());
            args.add(result.units.to_string());
            args.add(result.avg_unit_cost.to_string());
            args.add(result.last_price.to_string());
            args.add(result.total_cost.to_string());
            args.add(result.value.to_string());
            args.add(result.change.to_string());
        }

        sqlx::query_with(&sql, args).execute(db.deref()).await?;

        Ok(())
    }
}
