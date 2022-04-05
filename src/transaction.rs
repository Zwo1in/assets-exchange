pub type ClientId = u16;
pub type TransactionId = u32;

#[derive(Debug, Copy, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Transaction {
    pub r#type: TransactionType,
    pub client: ClientId,
    pub tx: TransactionId,
    pub amount: Amount,
}

#[derive(Debug, Copy, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Amount(#[serde(with = "serde_amount")] pub f64);

impl std::ops::Deref for Amount {
    type Target = f64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Amount {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

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
