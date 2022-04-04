use std::collections::HashMap;

pub type ClientId = u16;
pub type TransactionId = u32;

#[derive(Debug, Copy, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Amount(#[serde(with = "serde_amount")] f64);

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
        serializer.serialize_some(&val)
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
    r#type: TransactionType,
    client: ClientId,
    tx: TransactionId,
    amount: Amount,
}

#[derive(Debug)]
pub struct DisputableTransaction {
    transaction: Transaction,
    disputed: bool,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Account {
    id: ClientId,
    available: Amount,
    held: Amount,
    total: Amount,
    locked: bool,
    #[serde(skip)]
    tx_history: HashMap<TransactionId, DisputableTransaction>,
}

impl Default for Account {
    fn default() -> Self {
        Self {
            id: 0,
            available: Amount(0.),
            total: Amount(0.),
            held: Amount(0.),
            locked: false,
            tx_history: HashMap::new(),
        }
    }
}

impl Account {
    pub fn new(client_id: ClientId) -> Self {
        Self {
            id: client_id,
            ..Default::default()
        }
    }

    pub fn save_tx(&mut self, tx: Transaction) {
        self.tx_history.insert(
            tx.tx,
            DisputableTransaction {
                transaction: tx,
                disputed: false,
            },
        );
    }

    pub fn apply(&mut self, tx: Transaction) {
        match tx.r#type {
            TransactionType::Deposit => {
                *self.available += *tx.amount;
                *self.total += *tx.amount;
                self.save_tx(tx);
            }
            TransactionType::Withdrawal => {
                if *self.available >= *tx.amount {
                    *self.available -= *tx.amount;
                    *self.total -= *tx.amount;
                    self.save_tx(tx);
                }
            }
            _ => {
                if let Some(disputable_tx) = self.tx_history.get_mut(&tx.tx) {
                    // Do nothing when disputing already disputed transaction
                    // or resolving / charging back not disputed transaction
                    match (disputable_tx.disputed, tx.r#type) {
                        (true, TransactionType::Dispute) => return,
                        (false, TransactionType::Resolve) => return,
                        (false, TransactionType::Chargeback) => return,
                        _ => (),
                    }
                    match disputable_tx.transaction.r#type {
                        // All instructions regarding disputes felt like written for disputing
                        // deposit transactions, with
                        // - dispute meaning that transaction should be temporary reverted
                        // - resolve meaning that dispute should be reverted
                        // - chargeback meaning that transaction should be fully reverted
                        // The assumptions made for disputing withdrawal transactions were
                        // based on this understanding.
                        TransactionType::Deposit => match tx.r#type {
                            TransactionType::Dispute => {
                                *self.available -= *disputable_tx.transaction.amount;
                                *self.held += *disputable_tx.transaction.amount;
                                disputable_tx.disputed = true;
                            }
                            TransactionType::Resolve => {
                                *self.available += *disputable_tx.transaction.amount;
                                *self.held -= *disputable_tx.transaction.amount;
                                disputable_tx.disputed = false;
                            }
                            TransactionType::Chargeback => {
                                *self.total -= *disputable_tx.transaction.amount;
                                *self.held -= *disputable_tx.transaction.amount;
                                self.locked = true;
                            }
                            // Excluded by first match
                            _ => unreachable!(),
                        },
                        // For dealing with withdrawals the following assumptions were made
                        TransactionType::Withdrawal => match tx.r#type {
                            // Disputing withdrawal:
                            // - held and total should increase by a previously withdrawn amount
                            // - available amount shouldn't change
                            TransactionType::Dispute => {
                                *self.total += *disputable_tx.transaction.amount;
                                *self.held += *disputable_tx.transaction.amount;
                                disputable_tx.disputed = true;
                            }
                            // Resolving withdrawal
                            // - held and total should decrease by the amount no longer disputed
                            // - available amount shouldn't change
                            TransactionType::Resolve => {
                                *self.total -= *disputable_tx.transaction.amount;
                                *self.held -= *disputable_tx.transaction.amount;
                                disputable_tx.disputed = false;
                            }
                            // Charging back withdrawal:
                            // - available should increase by the amount disputed
                            // - held should decrease by the amount disputed
                            // - total shouldn't change
                            TransactionType::Chargeback => {
                                *self.available += *disputable_tx.transaction.amount;
                                *self.held -= *disputable_tx.transaction.amount;
                                self.locked = true;
                            }
                            // Excluded by first match
                            _ => unreachable!(),
                        },
                        // Only deposit and withdrawal transactions are stored in history
                        _ => unreachable!(),
                    }
                }
            }
        }
    }
}

pub struct Service {
    accounts: HashMap<ClientId, Account>,
}

impl Service {
    fn new() -> Self {
        Self {
            accounts: HashMap::new(),
        }
    }

    pub fn apply(&mut self, tx: Transaction) {
        self.accounts
            .entry(tx.client)
            .or_insert(Account::new(tx.client))
            .apply(tx);
    }
}

fn main() {
    let csv = "type, client, tx, amount
deposit, 1, 1, 1.123180
deposit, 2, 2, 2.0
deposit, 1, 3, 2.0
withdrawal, 1, 4, 1.5
withdrawal, 2, 5, 3.0
";
    let mut service = Service::new();

    csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(csv.as_bytes())
        .into_deserialize()
        .map(|res: Result<Transaction, _>| res.expect("Failed to read transaction"))
        .for_each(|tx| service.apply(tx));

    let mut csv_writer = csv::WriterBuilder::new().from_writer(std::io::stdout());

    for account in service.accounts.values() {
        csv_writer.serialize(account).expect(&format!(
            "Failed to print the state for account with client id: {}",
            account.id
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Default for Transaction {
        fn default() -> Self {
            Self {
                client: 0,
                tx: 0,
                r#type: TransactionType::Deposit,
                amount: Amount(0.),
            }
        }
    }

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

    #[test]
    fn withdrawal_with_sufficient_funds_should_charge_account() {
        let mut account = Account {
            available: Amount(5.),
            total: Amount(5.),
            locked: false,
            ..Default::default()
        };

        let tx = Transaction {
            r#type: TransactionType::Withdrawal,
            amount: Amount(4.),
            ..Default::default()
        };

        account.apply(tx);

        assert_eq!(account.total, Amount(1.));
        assert_eq!(account.available, Amount(1.));
    }

    #[test]
    fn withdrawal_with_unsufficient_funds_should_be_rejected() {
        let mut account = Account {
            available: Amount(4.),
            total: Amount(4.),
            ..Default::default()
        };
        let tx = Transaction {
            r#type: TransactionType::Withdrawal,
            amount: Amount(5.),
            ..Default::default()
        };

        account.apply(tx);

        assert_eq!(account.total, Amount(4.));
        assert_eq!(account.available, Amount(4.));
    }

    #[test]
    fn dispute_to_deposit_should_freeze_funds() {
        let mut account = Account::default();
        account.apply(Transaction {
            r#type: TransactionType::Deposit,
            amount: Amount(5.),
            ..Default::default()
        });

        account.apply(Transaction {
            r#type: TransactionType::Dispute,
            tx: 0,
            ..Default::default()
        });

        assert_eq!(account.total, Amount(5.));
        assert_eq!(account.available, Amount(0.));
        assert_eq!(account.held, Amount(5.));
        assert_eq!(account.tx_history[&0].disputed, true);
    }

    #[test]
    fn dispute_to_withdrawal_should_raise_held_funds() {
        let mut account = Account {
            available: Amount(5.),
            total: Amount(5.),
            ..Account::default()
        };
        account.apply(Transaction {
            r#type: TransactionType::Withdrawal,
            amount: Amount(5.),
            ..Default::default()
        });

        account.apply(Transaction {
            r#type: TransactionType::Dispute,
            tx: 0,
            ..Default::default()
        });

        assert_eq!(account.total, Amount(5.));
        assert_eq!(account.available, Amount(0.));
        assert_eq!(account.held, Amount(5.));
        assert_eq!(account.tx_history[&0].disputed, true);
    }

    #[test]
    fn resolving_disputed_deposit_should_revert_dispute() {
        let mut account = Account::default();
        account.apply(Transaction {
            r#type: TransactionType::Deposit,
            amount: Amount(5.),
            ..Default::default()
        });
        account.apply(Transaction {
            r#type: TransactionType::Dispute,
            tx: 0,
            ..Default::default()
        });

        account.apply(Transaction {
            r#type: TransactionType::Resolve,
            tx: 0,
            ..Default::default()
        });

        assert_eq!(account.total, Amount(5.));
        assert_eq!(account.available, Amount(5.));
        assert_eq!(account.held, Amount(0.));
        assert_eq!(account.tx_history[&0].disputed, false);
    }

    #[test]
    fn resolving_disputed_withdrawal_should_revert_dispute() {
        let mut account = Account {
            available: Amount(5.),
            total: Amount(5.),
            ..Account::default()
        };
        account.apply(Transaction {
            r#type: TransactionType::Withdrawal,
            amount: Amount(5.),
            ..Default::default()
        });
        account.apply(Transaction {
            r#type: TransactionType::Dispute,
            tx: 0,
            ..Default::default()
        });

        account.apply(Transaction {
            r#type: TransactionType::Resolve,
            tx: 0,
            ..Default::default()
        });

        assert_eq!(account.total, Amount(0.));
        assert_eq!(account.available, Amount(0.));
        assert_eq!(account.held, Amount(0.));
        assert_eq!(account.tx_history[&0].disputed, false);
    }

    #[test]
    fn charging_back_disputed_deposit_should_revert_transaction() {
        let mut account = Account::default();
        account.apply(Transaction {
            r#type: TransactionType::Deposit,
            amount: Amount(5.),
            ..Default::default()
        });
        account.apply(Transaction {
            r#type: TransactionType::Dispute,
            tx: 0,
            ..Default::default()
        });

        account.apply(Transaction {
            r#type: TransactionType::Chargeback,
            tx: 0,
            ..Default::default()
        });

        assert_eq!(account.total, Amount(0.));
        assert_eq!(account.available, Amount(0.));
        assert_eq!(account.held, Amount(0.));
        assert_eq!(account.locked, true);
    }

    #[test]
    fn charging_back_disputed_withdrawal_should_revert_transaction() {
        let mut account = Account {
            available: Amount(5.),
            total: Amount(5.),
            ..Account::default()
        };
        account.apply(Transaction {
            r#type: TransactionType::Withdrawal,
            amount: Amount(5.),
            ..Default::default()
        });
        account.apply(Transaction {
            r#type: TransactionType::Dispute,
            tx: 0,
            ..Default::default()
        });

        account.apply(Transaction {
            r#type: TransactionType::Chargeback,
            tx: 0,
            ..Default::default()
        });

        assert_eq!(account.total, Amount(5.));
        assert_eq!(account.available, Amount(5.));
        assert_eq!(account.held, Amount(0.));
        assert_eq!(account.locked, true);
    }
}
