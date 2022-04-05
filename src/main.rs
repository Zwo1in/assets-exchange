use std::collections::HashMap;

pub(crate) mod account;
pub(crate) mod transaction;

use account::{Account, TransactionResult};
use transaction::{ClientId, Transaction};

/// An exchanging service is a container for all created user accounts
///
/// It handles dispatching transactions to correct accounts as well as
/// creating new accounts where needed
pub struct Service {
    accounts: HashMap<ClientId, Account>,
}

impl Service {
    /// Create a new service
    fn new() -> Self {
        Self {
            accounts: HashMap::new(),
        }
    }

    /// Dispatch a transaction to correct account and create one if it doesn't exist yet
    pub fn apply(&mut self, tx: Transaction) -> TransactionResult<()> {
        self.accounts
            .entry(tx.client)
            .or_insert(Account::new(tx.client))
            .apply(tx)
    }
}

/// Parse commandline arguments and apply all transactions from given csv to accounts
///
/// Output all the accounts as a csv on the process's stdout
/// Output all warnings regarding failed transactions on the process's stderr
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let input_file = match args.len() {
        2 => &args[1],
        _ => {
            eprintln!("Usage: {} <path_to_csv_with_transactions>", args[0]);
            std::process::exit(1);
        }
    };

    let mut service = Service::new();

    csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_path(input_file)
        .expect(&format!("Couldn't open file {}", input_file))
        .into_deserialize()
        .map(|res: Result<Transaction, _>| res.expect("Failed to read transaction"))
        .for_each(|tx| {
            if let Err(e) = service.apply(tx) {
                eprintln!("warn - {e}");
            }
        });

    let mut csv_writer = csv::WriterBuilder::new().from_writer(std::io::stdout());

    for account in service.accounts.values() {
        csv_writer.serialize(account).expect(&format!(
            "Failed to print the state for account with client id: {}",
            account.id()
        ));
    }
}
