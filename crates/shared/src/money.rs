use serde::{Deserialize, Serialize};

use crate::currency::Currency;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Money {
    pub amount_minor: i64,
    pub currency: Currency,
}

impl Money {
    pub fn new(amount_minor: i64, currency: Currency) -> Result<Self, &'static str> {
        if amount_minor < 0 {
            return Err("amount must be non-negative");
        }

        Ok(Self { amount_minor, currency })
    }
}
