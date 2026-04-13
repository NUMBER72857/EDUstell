use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Currency {
    Usdc,
    Fiat(String),
}

impl Currency {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Usdc => "USDC",
            Self::Fiat(code) => code.as_str(),
        }
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for Currency {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "USDC" => Ok(Self::Usdc),
            other if other.len() == 3 => Ok(Self::Fiat(other.to_owned())),
            _ => Err("invalid currency"),
        }
    }
}
