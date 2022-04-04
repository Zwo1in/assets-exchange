type ClientId = u16;
type TransactionId = u32;
type Amount = f64;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum TransactionType {
    Deposit,
    Withdrawal,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Transaction {
    r#type: TransactionType,
    client: ClientId,
    tx: TransactionId,
    amount: Amount,
}

fn main() {
    let csv = "type, client, tx, amount
deposit, 1, 1, 1.0
deposit, 2, 2, 2.0
deposit, 1, 3, 2.0
withdrawal, 1, 4, 1.5
withdrawal, 2, 5, 3.0
";

    csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(csv.as_bytes())
        .into_deserialize()
        .map(|res: Result<Transaction, _>| res.expect("Failed to read transaction"))
        .for_each(|tx| println!("{tx:?}"));
}
