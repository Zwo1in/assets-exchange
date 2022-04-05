use std::collections::HashMap;
use thiserror::Error;

use crate::transaction::{Amount, ClientId, Transaction, TransactionId, TransactionType};

/// Possible errors that can happen when applying a transaction
#[derive(Error, Debug)]
pub enum TransactionError {
    #[error("Unsufficient funds to handle transaction `{0:?}`")]
    UnsufficientFunds(Transaction),
    #[error("Account is locked. Can't perform transaction.")]
    AccountLocked,
    #[error("Transaction `{0}` is already under dispute")]
    AlreadyDisputed(TransactionId),
    #[error("Transaction `{0}` is not under dispute")]
    NotDisputed(TransactionId),
    #[error("Transaction with ID `{0}` not found")]
    NotFound(TransactionId),
    #[error("Transaction with ID `{0}` already exist")]
    AlreadyExist(TransactionId),
}

/// Result type used when operating on account
pub type TransactionResult<T> = Result<T, TransactionError>;

/// Wrapper for transaction that remembers if there is an open dispute
#[derive(Debug)]
pub struct DisputableTransaction {
    transaction: Transaction,
    disputed: bool,
}

/// Model of user account
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
    /// Create a new account assigned to `client_id`
    pub fn new(client_id: ClientId) -> Self {
        Self {
            id: client_id,
            ..Default::default()
        }
    }

    /// Get id assigned to given account
    pub fn id(&self) -> ClientId {
        self.id
    }

    /// Put a transaction into tx_history
    pub fn save_tx(&mut self, tx: Transaction) -> TransactionResult<()> {
        if self.tx_history.contains_key(&tx.tx) {
            return Err(TransactionError::AlreadyExist(tx.tx));
        }
        self.tx_history.insert(
            tx.tx,
            DisputableTransaction {
                transaction: tx,
                disputed: false,
            },
        );
        Ok(())
    }

    /// Try to apply a transaction on user account
    pub fn apply(&mut self, tx: Transaction) -> TransactionResult<()> {
        if self.locked {
            return Err(TransactionError::AccountLocked);
        }
        match tx.r#type {
            TransactionType::Deposit => {
                self.available += tx.amount;
                self.total += tx.amount;
                self.save_tx(tx)
            }
            TransactionType::Withdrawal => {
                if self.available >= tx.amount {
                    self.available -= tx.amount;
                    self.total -= tx.amount;
                    self.save_tx(tx)
                } else {
                    Err(TransactionError::UnsufficientFunds(tx))
                }
            }
            _ => self.handle_disputes(tx),
        }
    }

    /// Handle disputing, resolving and charging back deposits and withdrawals
    fn handle_disputes(&mut self, current_tx: Transaction) -> TransactionResult<()> {
        let disputable_tx = if let Some(disputable_tx) = self.tx_history.get_mut(&current_tx.tx) {
            disputable_tx
        } else {
            return Err(TransactionError::NotFound(current_tx.tx));
        };
        // Do nothing when disputing already disputed transaction
        // or resolving / charging back not disputed transaction
        match (disputable_tx.disputed, current_tx.r#type) {
            (true, TransactionType::Dispute) => {
                return Err(TransactionError::AlreadyDisputed(current_tx.tx))
            }
            (false, TransactionType::Resolve) => {
                return Err(TransactionError::NotDisputed(current_tx.tx))
            }
            (false, TransactionType::Chargeback) => {
                return Err(TransactionError::NotDisputed(current_tx.tx))
            }
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
                    // When disputing a deposit transaction, check if client
                    // hasn't already withdrawn what he want to charge back
                    if self.available < disputable_tx.transaction.amount {
                        return Err(TransactionError::UnsufficientFunds(current_tx));
                    }
                    self.available -= disputable_tx.transaction.amount;
                    self.held += disputable_tx.transaction.amount;
                    disputable_tx.disputed = true;
                }
                TransactionType::Resolve => {
                    self.available += disputable_tx.transaction.amount;
                    self.held -= disputable_tx.transaction.amount;
                    disputable_tx.disputed = false;
                }
                TransactionType::Chargeback => {
                    self.total -= disputable_tx.transaction.amount;
                    self.held -= disputable_tx.transaction.amount;
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
                    self.total += disputable_tx.transaction.amount;
                    self.held += disputable_tx.transaction.amount;
                    disputable_tx.disputed = true;
                }
                // Resolving withdrawal
                // - held and total should decrease by the amount no longer disputed
                // - available amount shouldn't change
                TransactionType::Resolve => {
                    self.total -= disputable_tx.transaction.amount;
                    self.held -= disputable_tx.transaction.amount;
                    disputable_tx.disputed = false;
                }
                // Charging back withdrawal:
                // - available should increase by the amount disputed
                // - held should decrease by the amount disputed
                // - total shouldn't change
                TransactionType::Chargeback => {
                    self.available += disputable_tx.transaction.amount;
                    self.held -= disputable_tx.transaction.amount;
                    self.locked = true;
                }
                // Excluded back in apply
                _ => unreachable!(),
            },
            // Only deposit and withdrawal transactions are stored in history
            _ => unreachable!(),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deposit(amount: f64, tx: TransactionId) -> Transaction {
        Transaction {
            r#type: TransactionType::Deposit,
            amount: Amount(amount),
            tx,
            client: 0,
        }
    }

    fn withdrawal(amount: f64, tx: TransactionId) -> Transaction {
        Transaction {
            r#type: TransactionType::Withdrawal,
            amount: Amount(amount),
            tx,
            client: 0,
        }
    }

    fn dispute(tx: TransactionId) -> Transaction {
        Transaction {
            r#type: TransactionType::Dispute,
            tx,
            client: 0,
            amount: Amount(0.),
        }
    }

    fn resolve(tx: TransactionId) -> Transaction {
        Transaction {
            r#type: TransactionType::Resolve,
            tx,
            client: 0,
            amount: Amount(0.),
        }
    }

    fn chargeback(tx: TransactionId) -> Transaction {
        Transaction {
            r#type: TransactionType::Chargeback,
            tx,
            client: 0,
            amount: Amount(0.),
        }
    }

    #[test]
    fn transaction_id_should_be_unique() {
        let mut account = Account::default();
        account.apply(deposit(5., 0)).unwrap();

        assert!(account.apply(deposit(5., 0)).is_err());
    }

    #[test]
    fn withdrawal_with_sufficient_funds_should_charge_account() {
        let mut account = Account {
            available: Amount(5.),
            total: Amount(5.),
            locked: false,
            ..Default::default()
        };

        account.apply(withdrawal(4., 0)).unwrap();

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

        assert!(account.apply(withdrawal(5., 0)).is_err());
    }

    #[test]
    fn dispute_to_already_disputed_tx_should_fail() {
        let mut account = Account::default();
        account.apply(deposit(5., 0)).unwrap();

        account.apply(dispute(0)).unwrap();

        assert!(account.apply(dispute(0)).is_err());
    }

    #[test]
    fn dispute_to_deposit_should_freeze_funds() {
        let mut account = Account::default();
        account.apply(deposit(5., 0)).unwrap();

        account.apply(dispute(0)).unwrap();

        assert_eq!(account.total, Amount(5.));
        assert_eq!(account.available, Amount(0.));
        assert_eq!(account.held, Amount(5.));
    }

    #[test]
    fn dispute_to_deposit_should_not_work_if_funds_already_withdrawn() {
        let mut account = Account::default();
        account.apply(deposit(5., 0)).unwrap();
        account.apply(withdrawal(4., 1)).unwrap();

        assert!(account.apply(dispute(0)).is_err());
    }

    #[test]
    fn dispute_to_withdrawal_should_raise_held_funds() {
        let mut account = Account {
            available: Amount(5.),
            total: Amount(5.),
            ..Account::default()
        };
        account.apply(withdrawal(5., 0)).unwrap();

        account.apply(dispute(0)).unwrap();

        assert_eq!(account.total, Amount(5.));
        assert_eq!(account.available, Amount(0.));
        assert_eq!(account.held, Amount(5.));
    }

    #[test]
    fn resolving_and_charging_back_on_not_disputed_tx_should_fail() {
        let mut account = Account::default();
        account.apply(deposit(5., 0)).unwrap();

        assert!(account.apply(resolve(0)).is_err());
        assert!(account.apply(chargeback(0)).is_err());
    }

    #[test]
    fn resolving_disputed_deposit_should_revert_dispute() {
        let mut account = Account::default();
        account.apply(deposit(5., 0)).unwrap();
        account.apply(dispute(0)).unwrap();

        account.apply(resolve(0)).unwrap();

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
        account.apply(withdrawal(5., 0)).unwrap();
        account.apply(dispute(0)).unwrap();

        account.apply(resolve(0)).unwrap();

        assert_eq!(account.total, Amount(0.));
        assert_eq!(account.available, Amount(0.));
        assert_eq!(account.held, Amount(0.));
        assert_eq!(account.tx_history[&0].disputed, false);
    }

    #[test]
    fn charging_back_disputed_deposit_should_revert_transaction() {
        let mut account = Account::default();
        account.apply(deposit(5., 0)).unwrap();
        account.apply(dispute(0)).unwrap();

        account.apply(chargeback(0)).unwrap();

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
        account.apply(withdrawal(5., 0)).unwrap();
        account.apply(dispute(0)).unwrap();

        account.apply(chargeback(0)).unwrap();

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

        assert!(account.apply(deposit(555., 0)).is_err());
        assert!(account.apply(withdrawal(111., 0)).is_err());
        assert!(account.apply(dispute(0)).is_err());
    }
}
