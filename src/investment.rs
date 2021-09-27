use rust_decimal::Decimal;
use serde::Serialize;

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
