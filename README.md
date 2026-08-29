# rust-coding-challenge

[![Rust](https://github.com/pwittchen/rust-coding-challenge/actions/workflows/rust.yml/badge.svg)](https://github.com/pwittchen/rust-coding-challenge/actions/workflows/rust.yml)

A toy payments engine. It reads a CSV of transactions, applies them to
per-client accounts, and writes the final state of every account as CSV to
standard output.

Five kinds of transaction are supported:

| Type | Effect |
| --- | --- |
| `deposit` | Credits the client's account: available and total funds increase |
| `withdrawal` | Debits the client's account, unless the available funds do not cover it |
| `dispute` | Holds the funds of the referenced deposit: available decreases, held increases, total is unchanged |
| `resolve` | Releases the held funds of a disputed deposit |
| `chargeback` | Reverses the disputed deposit and freezes the account |

Records are read and applied one at a time, so the input is never held in memory
in full. Amounts use a fixed-point decimal rather than a float, so four decimal
places are represented exactly and repeated additions never accumulate a
rounding error.

## Prerequisites

- A Rust toolchain with Cargo — [rustup](https://rustup.rs) is the easiest way
  to get one. The crate uses edition 2024, which requires Rust 1.85 or newer; it
  is developed and tested on 1.94.
- The `rustfmt` and `clippy` components, if you want to format the code or lint
  it. `rustup` installs both by default:

  ```sh
  rustup component add rustfmt clippy
  ```

Nothing else is needed. The dependencies — `csv`, `serde` and `rust_decimal` —
are pure Rust and are fetched by Cargo on the first build.

## Building

```sh
cargo build             # debug build
cargo build --release   # optimized build
```

## Running

The input file is the first and only argument; the report goes to standard
output:

```sh
cargo run -- transactions.csv > accounts.csv
```

[`transactions.csv`](transactions.csv) is included as sample input:

```csv
type, client, tx, amount
deposit, 1, 1, 1.0
deposit, 2, 2, 2.0
deposit, 1, 3, 2.0
withdrawal, 1, 4, 1.5
withdrawal, 2, 5, 3.0
```

Running the command above writes `accounts.csv`:

```csv
client,available,held,total,locked
1,1.5000,0.0000,1.5000,false
2,2.0000,0.0000,2.0000,false
```

Client 2's withdrawal of `3.0` is rejected, because their available funds do not
cover it. Errors — a missing argument, an unreadable file, a malformed record —
are reported on standard error, and the program exits with a non-zero status so
that a failed run is never mistaken for an empty report.

## Formatting and linting

```sh
cargo fmt              # format the code
cargo fmt --check      # verify the formatting without changing anything
cargo clippy -- -D warnings
```

## Testing

```sh
cargo test
```

The unit tests live next to the code they cover: parsing and reading the input
in `src/transaction.rs` and `src/input.rs`, every transaction type and its edge
cases in `src/engine.rs`, and the shape and precision of the report in
`src/output.rs`. The engine tests drive the engine through the same CSV parsing
the binary uses, so they exercise the whole path from input row to account
state.

## Project layout

| File | Responsibility |
| --- | --- |
| `src/main.rs` | The CLI: argument handling, streaming the input into the engine, reporting errors |
| `src/transaction.rs` | The data model of a transaction and of the history a dispute refers back to |
| `src/input.rs` | Reading transactions from a CSV, as a lazy stream |
| `src/engine.rs` | All transaction logic: accounts, balances, disputes, chargebacks |
| `src/output.rs` | Writing the resulting account state as CSV |

## Assumptions

The specification leaves a few cases open. They are resolved the way a bank
would resolve them, and the reasoning is documented on the code that implements
each decision, in `src/engine.rs`:

- **Only deposits can be disputed.** A dispute moves funds from available to
  held, which only makes sense for money that was paid in; holding the amount of
  a withdrawal would seize funds the client never received. A dispute over a
  withdrawal is treated like one over an unknown transaction and ignored.
- **A dispute, resolve or chargeback must come from the client that owns the
  referenced transaction.** Otherwise one client could freeze another's account.
- **A frozen account accepts no further transactions of any kind.** A chargeback
  is the end of the account's activity until a human intervenes.
- **A transaction ID is used at most once.** IDs are globally unique, so a
  repeated one is an error on the partner's side, and honouring it would make a
  later dispute ambiguous.
- **A dispute may drive the available funds negative**, when the deposit under
  dispute has already been withdrawn. The total must not change, so the held
  amount has to come out of the available funds whether they cover it or not,
  leaving the client owing the difference.
- **A deposit or withdrawal without an amount, or with a negative one, is
  ignored.** A negative deposit is a withdrawal in disguise, and would bypass
  the check that the available funds cover it.

Anything the engine cannot apply is ignored and processing continues, as the
specification requires. A malformed CSV record, on the other hand, aborts the
run: it means the input itself cannot be trusted.
