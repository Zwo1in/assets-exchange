# A simple assets exchange service

## Correctness

The program's correctness is ensured by the set of unit tests that verify variations of
transactions applied on account.

There were some missing bits in specification so some additional assumptions were made:

- Dispute, as described only makes sense when disputed transaction is a Deposit.
  For this case, an analogous set of behaviours was implemented and documented inline for
  disputing Withdrawals.

- Dispute, when mistakenly approved could cost service provider money. Consider a case
  where client deposits money, then withdraws them and opens a dispute for deposit transaction.
  Client could potentially charge back deposited money that were already withdrawn.

  Example input:

  ```
  type,       client, tx, amount
  deposit,    1,      1,  5.0
  withdrawal, 1,      2,  5.0
  dispute,    1,      1,  0.0
  chargeback, 1,      1,  0.0
  ```


## Efficiency

Transactions are handled as a stream of operations read from the file one by one thanks to Rust's
`Read` trait implementation on `File`.


## Error handling

Error handling is done with Rust's builtin arsenal of Error trait and Result type.
All errors encountered during transaction processing are treated as warnings, displayed on
program's stderr without suspending execution.


## Some doubts :)

The `amount` field doesn't make sense for `dispute`, `resolve` and `chargeback`. It'd be nice to be
able not to provide it in csv input, like

```
type,    client, tx, amount
deposit, 1,      1,  5.0
dispute, 1,      1
```

However due to the fact that `serde` is not the best friend of `csv` format it couldn't be possible
without a manual deserialization implementations. Usage of `serde` and `csv` with out of the box
behaviour feels so convenient that this case wasn't handled and amounts are required in csv for all
transactions.
