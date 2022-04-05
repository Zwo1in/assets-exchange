use std::collections::HashMap;

pub(crate) mod account;
pub(crate) mod transaction;

use account::Account;
use transaction::{ClientId, Transaction};

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
    let csv = "\
type,       client, tx, amount
deposit,    1,      1,  1.123180
deposit,    2,      2,  2.0
deposit,    1,      3,  2.0
withdrawal, 1,      4,  0.5
withdrawal, 2,      5,  3.0
dispute,    1,      3,  0
dispute,    2,      2,  0
resolve,    1,      3,  0
chargeback, 2,      2,  0
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
            account.id()
        ));
    }
}
