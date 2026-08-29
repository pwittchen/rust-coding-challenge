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
cargo clean             # remove target/, the build artifacts of both profiles
```

## Running

The input file is the first and only argument; the report goes to standard
output:

```sh
cargo run -- transactions.csv > accounts.csv
```

[`transactions.csv`](transactions.csv) is included as sample input. It exercises
every transaction type and every outcome the report can show:

```csv
type, client, tx, amount
deposit, 1, 1, 1.0
deposit, 2, 2, 2.0
deposit, 1, 3, 2.0
withdrawal, 1, 4, 1.5
withdrawal, 2, 5, 3.0
deposit, 3, 6, 5.0
deposit, 3, 7, 1.5
dispute, 3, 6,
resolve, 3, 6,
deposit, 4, 8, 4.0
dispute, 4, 8,
deposit, 5, 9, 3.0
dispute, 5, 9,
chargeback, 5, 9,
deposit, 5, 10, 1.0
```

Running the command above writes `accounts.csv`:

```csv
client,available,held,total,locked
1,1.5000,0.0000,1.5000,false
2,2.0000,0.0000,2.0000,false
3,6.5000,0.0000,6.5000,false
4,0.0000,4.0000,4.0000,false
5,0.0000,0.0000,0.0000,true
```

Reading down the report:

- client 2's withdrawal of `3.0` is rejected, because their available funds do
  not cover it;
- client 3's dispute was resolved, so their funds are available again;
- client 4's dispute is still open, so its amount is held and their total is
  unchanged;
- client 5 charged back, which emptied and froze the account — their last
  deposit arrived after the freeze and was ignored.

Errors — a missing argument, an unreadable file, a malformed record — are
reported on standard error, and the program exits with a non-zero status so that
a failed run is never mistaken for an empty report.

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

The unit tests live next to the code they cover: argument handling in
`src/main.rs`, parsing and reading the input in `src/transaction.rs` and
`src/input.rs`, every transaction type and its edge cases in `src/engine.rs`,
and the shape and precision of the report in `src/output.rs`. The engine tests
drive the engine through the same CSV parsing the binary uses, so they exercise
the whole path from input row to account state.

Every transaction type is covered on both its happy path and its negative ones:
a withdrawal that is not covered, that is covered only by held funds, or that
would leave an already negative balance; a dispute over an unknown transaction,
over a withdrawal, over a deposit that was itself rejected, over one that is
already disputed, or raised by another client; a resolve or a chargeback for a
transaction that is not under dispute, that was already settled, or that belongs
to somebody else; a chargeback that freezes the account and leaves the total
negative; and further transactions arriving on a frozen account. The boundaries
are covered too: the largest client and transaction IDs, four-decimal precision,
amounts finer than that, and balances that would overflow. One test runs the
sample input above and asserts the report documented for it, so the example and
the code cannot drift apart.

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
- **A dispute, resolve, or chargeback must come from the client that owns the
  referenced transaction.** Otherwise, one client could freeze another's account.
- **A frozen account accepts no further transactions of any kind.** A chargeback
  is the end of the account's activity until a human intervenes.
- **A deposit reusing the ID of an earlier one is ignored.** IDs are globally
  unique, so a repeated one is an error on the partner's side, and honouring it
  would make a later dispute ambiguous. Withdrawals are not recorded at all, so
  nothing can refer back to one and their IDs are not tracked.
- **A deposit or a withdrawal opens the account of the client it names**, even
  when the transaction itself is rejected: the client exists as far as the input
  is concerned, and an attempt to move money they do not have leaves them with
  an empty account rather than with none at all. Disputes, resolves, and
  chargebacks never open an account, because they only refer back to a
  transaction whose client already has one.
- **A dispute may drive the available funds negative**, when the deposit under
  dispute has already been withdrawn. The total must not change, so the held
  amount has to come out of the available funds whether they cover it or not,
  leaving the client owing the difference.
- **A deposit or withdrawal without an amount, or with a negative one, is
  ignored.** A negative deposit is a withdrawal in disguise, and would bypass
  the check that the available funds cover it.
- **An amount carrying more than four decimal places is cut to four**, not
  rounded. The input is specified to carry no more than four, so this only bites
  on a malformed row. Cutting it keeps every balance exactly as precise as the
  report that prints it — otherwise a fraction too small to show could still be
  counted, and the reported `available` and `held` would no longer add up to the
  reported `total` — and it never credits a client a fraction they did not send.

Anything the engine cannot apply is ignored and processing continues, as the
specification requires. A malformed CSV record, on the other hand, aborts the
run: it means the input itself cannot be trusted.
