use std::collections::HashMap;

use crate::transaction::{Amount, ClientId, Transaction, TransactionId, TransactionType};

#[derive(Debug)]
pub struct DisputableTransaction {
    transaction: Transaction,
    disputed: bool,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Account {
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

    pub fn id(&self) -> ClientId {
        self.id
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
        if self.locked {
            return;
        }
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
            _ => self.handle_disputes(tx),
        }
    }

    fn handle_disputes(&mut self, current_tx: Transaction) {
        if let Some(disputable_tx) = self.tx_history.get_mut(&current_tx.tx) {
            // Do nothing when disputing already disputed transaction
            // or resolving / charging back not disputed transaction
            match (disputable_tx.disputed, current_tx.r#type) {
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
                TransactionType::Deposit => match current_tx.r#type {
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
                    // Excluded back in apply
                    _ => unreachable!(),
                },
                // For dealing with withdrawals the following assumptions were made
                TransactionType::Withdrawal => match current_tx.r#type {
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
                    // Excluded back in apply
                    _ => unreachable!(),
                },
                // Only deposit and withdrawal transactions are stored in history
                _ => unreachable!(),
            }
        }
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

    #[test]
    fn no_transaction_should_take_effect_on_locked_account() {
        let mut account = Account {
            locked: true,
            ..Account::default()
        };

        account.apply(Transaction {
            r#type: TransactionType::Deposit,
            amount: Amount(100.),
            ..Default::default()
        });
        account.apply(Transaction {
            r#type: TransactionType::Withdrawal,
            amount: Amount(55.55),
            tx: 1,
            ..Default::default()
        });
        account.apply(Transaction {
            r#type: TransactionType::Dispute,
            tx: 0,
            ..Default::default()
        });

        assert_eq!(account.total, Amount(0.));
        assert_eq!(account.available, Amount(0.));
        assert_eq!(account.held, Amount(0.));
        assert_eq!(account.locked, true);
    }
}
