pub type ClientId = u16;
pub type TransactionId = u32;

/// Possible types of transactions
#[derive(Debug, Copy, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

/// Model of a single transaction
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Transaction {
    // Tbh I dislike having type as a field here instead of a Transaction being enclosed
    // in an enum, however csv-rs doesn't support reading internally tagged enums
    pub r#type: TransactionType,
    pub client: ClientId,
    pub tx: TransactionId,
    pub amount: Amount,
}

/// A new-type over f64 that ensures reading/writing amounts with 4 dec digits precision
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct Amount(#[serde(with = "serde_amount")] pub f64);

// Helper impl to make working with `Amount`s a bit nicer
impl std::ops::AddAssign for Amount {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

// Helper impl to make working with `Amount`s a bit nicer
impl std::ops::SubAssign for Amount {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

/// A module for serialize/deserialize functions used to meet contract of decimal digits precision
mod serde_amount {
    use serde::{Deserialize, Deserializer, Serializer};

    const DECIMAL_PLACES: i32 = 4;

    /// Serialize function that serializes f64 values rounded to 4 decimal places
    pub fn serialize<S>(val: &f64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let factor = 10.0_f64.powi(DECIMAL_PLACES);
        let val = (val * factor).round() / factor;
        serializer.serialize_f64(val)
    }

    /// Deserialize function that deserializes f64 values truncated to 4 decimal places
    pub fn deserialize<'de, D>(deserializer: D) -> Result<f64, D::Error>
    where
        D: Deserializer<'de>,
    {
        let factor = 10.0_f64.powi(DECIMAL_PLACES);
        let val = f64::deserialize(deserializer)?;
        let val = (val * factor).trunc() / factor;
        Ok(val)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialzed_amount_should_be_truncated() {
        [
            ("1", 1.0_f64),
            ("1.0", 1.0_f64),
            ("1.12341", 1.1234_f64),
            ("1.12349", 1.1234_f64),
        ]
        .into_iter()
        .for_each(|(input, expected)| {
            assert_eq!(expected, serde_json::from_str::<Amount>(input).unwrap().0)
        });
    }

    #[test]
    fn serialzed_amount_should_be_rounded() {
        [
            (1_f64, "1.0"),
            (1.0_f64, "1.0"),
            (1.12341_f64, "1.1234"),
            (1.12349_f64, "1.1235"),
        ]
        .into_iter()
        .for_each(|(input, expected)| {
            assert_eq!(
                expected,
                serde_json::to_string(&Amount(input)).unwrap().as_str()
            )
        });
    }
}
