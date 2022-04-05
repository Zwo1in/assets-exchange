use std::collections::HashMap;

pub(crate) mod account;
pub(crate) mod transaction;

use account::{Account, TransactionResult};
use transaction::{ClientId, Transaction};

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
        .flexible(true)
        .from_path(input_file)
        .expect(&format!("Couldn't open file {}", input_file))
        .into_records()
        .map(|res| res.expect("Failed to decode record as utf8"))
        .map(deserialize_record)
        .map(|res| res.expect("Failed to read transaction"))
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

/// Convert `csv::StringRecord` to a valid `Transaction`
///
/// In case that transaction is one of `dispute`, `resolve`, `chargeback`, the `amount`
/// field can be missing in input as it is not meaningful in this context. In those cases
/// to correctly deserialize a record, a placeholder `0.0` value is pushed in it's place
/// so that `StringRecord::deserialize` will still work.
fn deserialize_record(mut record: csv::StringRecord) -> Result<Transaction, csv::Error> {
    let tx_type = record.get(0).expect("An empty record as an input");
    match tx_type {
        "dispute" | "resolve" | "chargeback" => {
            if record.len() == 3 {
                record.push_field("0.0");
            }
        }
        _ => (),
    }
    let header = csv::StringRecord::from(vec!["type", "client", "tx", "amount"]);
    record.deserialize(Some(&header))
}
