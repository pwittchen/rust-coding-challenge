# rust-coding-challenge

[![Rust](https://github.com/pwittchen/rust-coding-challenge/actions/workflows/rust.yml/badge.svg)](https://github.com/pwittchen/rust-coding-challenge/actions/workflows/rust.yml)

A toy payments engine. It reads a CSV of transactions, applies them to
per-client accounts, and writes the final state of every account as CSV to
standard output.

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

A Rust toolchain with Cargo — [rustup](https://rustup.rs) is the easiest way to
get one. The crate uses edition 2024, which requires Rust 1.85 or newer; it is
developed and tested on 1.94. `rustfmt` and `clippy` are needed to format and
lint, and `rustup` installs both by default.

The dependencies — `csv`, `serde` and `rust_decimal` — are pure Rust and are
fetched by Cargo on the first build.

## Building

```sh
cargo build             # debug build
cargo build --release   # optimized build
cargo clean             # remove target/
```

The debug profile is built with the optimizer on ([`Cargo.toml`](Cargo.toml)),
because the engine is normally run through `cargo run` and an unoptimized build
takes about eight times as long. The debug assertions and the overflow checks
stay on.

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

Client 2's withdrawal of `3.0` is rejected for insufficient funds; client 3's
dispute was resolved; client 4's dispute is still open, so its amount is held;
client 5 charged back, which emptied and froze the account, so their last
deposit was ignored.

Errors — a missing argument, an unreadable file, a malformed record — are
reported on standard error and the program exits non-zero, so a failed run is
never mistaken for an empty report. See
[Safety and robustness](#safety-and-robustness) for what is treated as an error
and what is merely ignored.

## Formatting and linting

```sh
cargo fmt --check                            # verify the formatting
cargo clippy --all-targets -- -D warnings    # the denied lints, as CI runs them
cargo lint                                   # the pedantic pass, advisory
```

`cargo lint` is a Cargo alias defined in
[`.cargo/config.toml`](.cargo/config.toml). It runs Clippy with the `pedantic`
group on as warnings, which is a wider net than the CI command above.

The build-breaking lints are declared in [`Cargo.toml`](Cargo.toml): they are
described under [Safety and robustness](#safety-and-robustness), except
`missing_docs`, which is described under [Maintainability](#maintainability). CI
runs the formatting check, the lints and the tests on every push and pull
request.

## Testing

```sh
cargo test
```

Unit tests live next to the code they cover: argument handling in `src/main.rs`,
parsing in `src/transaction.rs` and `src/input.rs`, the balance arithmetic in
`src/account.rs`, every transaction type in `src/engine.rs`, and the shape and
precision of the report in `src/output.rs`. The engine tests drive the engine
through the same CSV parsing the binary uses, so they cover the whole path from
input row to account state.

[`tests/cli.rs`](tests/cli.rs) covers what the unit tests cannot see: it runs
the compiled binary and asserts on stdout, stderr and the exit status, including
a report whose destination closes before it can be written.

Every transaction type is covered on its happy path and its negative ones — an
uncovered withdrawal, a dispute over an unknown transaction or another client's
deposit, a resolve for something not under dispute, transactions arriving on a
frozen account — and so are the boundaries: the largest client and transaction
IDs, amounts finer than four decimal places, an amount with more significant
digits than a float could hold, and the largest balance an account can hold.

One test goes wider than any single rule: it generates four thousand
transactions from a deterministic generator and, after every one, asserts that a
dispute or resolve left the money exactly where it was, that a deposit never
reduced it and a withdrawal or chargeback never increased it, and that no
account holds a negative amount. Another runs two engines on two threads, as a
server serving many streams would. The sample input above and the example in the
crate's own documentation are both asserted by tests, so neither can drift from
what the program does.

## Safety and robustness

The rules below are not conventions the code is reviewed against. They are
denied package-wide in [`Cargo.toml`](Cargo.toml) and checked in CI, so breaking
one fails the build.

**No `unsafe`.** `unsafe_code` is set to `forbid`, which not even a local
`#[allow]` can switch off again.

**Nothing outside the tests may panic.** `unwrap`, `expect`, `panic!` and slice
indexing are denied. Every failure is a value the caller has to deal with, and
`main` is the single place that turns one into a message on stderr and a
non-zero exit status.

The lint covers this crate and cannot see inside a dependency. Where a
dependency's contract has such an edge, the crate stays off it: asking a decimal
to render with a fixed precision builds the text in a 32-byte buffer and panics
on a balance of 28 or more integer digits, which an account will accept, so
[`src/output.rs`](src/output.rs) lays the digits out itself.

**No floats.** `float_arithmetic` is denied, so a balance cannot silently lose a
fraction of a cent. Amounts are `rust_decimal::Decimal`, which represents four
decimal places exactly.

**No arithmetic that can panic.** `arithmetic_side_effects` is denied, so adding
two balances has to be a checked operation: a plain `+` on a decimal panics on
overflow, and denying `panic` does not catch that, since it covers the macro
rather than an operator that panics inside. The single exemption, in
[`clippy.toml`](clippy.toml), is negating an amount, which cannot overflow.

**Balance changes are checked and all-or-nothing.** `Account` keeps its balances
private, and every mutation goes through one checked helper that verifies the
new amounts and their sum are representable before storing them. An operation
that would overflow leaves the account exactly as it was, so
`total = available + held` always holds and no later arithmetic on a stored
balance can overflow. The engine cannot move money any other way.

### How errors are handled

**A transaction that cannot be applied is ignored, and processing continues.**
An unknown transaction ID, a resolve for something not under dispute, an
uncovered withdrawal: the specification calls these errors on the partner's
side, and one bad row is no reason to discard the rest of the file.

**A record that cannot be parsed aborts the run.** An unknown transaction type,
a client ID that does not fit a `u16`, an amount that is not a decimal. The
input is not what it claims to be, so nothing after that point can be trusted.
The run stops with a message naming the file, the line and the problem, and
exits non-zero.

The line is drawn at rows whose meaning cannot be recovered rather than at rows
that are merely untidy, so surrounding whitespace, a field beyond the four
columns and the capitalization of the `type` column are all tolerated.

The report is written only after the whole input has been consumed, so a failed
run writes nothing to stdout and a partial report can never be mistaken for the
account state. A report that cannot be written in full is itself an error rather
than a silent truncation.

### Resource use under hostile input

There is no recursion anywhere, so no input can exhaust the stack. What an input
can grow is memory, along the two axes measured under
[Efficiency](#efficiency): one account per client, and one record per deposit
that could still be disputed — bounded only by `u32::MAX`, the range of a
transaction ID. The CSV reader also imposes no limit on the size of a single
field. A server taking these streams from the network would address both outside
the engine, with a cap on the record size at the edge and a persistent,
ageing-out store behind the history.

## Efficiency

**The input is a stream, not a data structure.** The CSV reader decodes one
record at a time into a buffer it reuses, `main` applies each record as it
arrives, and the report is written at the end from the accounts alone. A 100 GB
file is no different from a 100 KB one.

**Memory tracks what can still be disputed, not what has been read.** The engine
keeps one account per client (at most `u16::MAX`, 36 bytes each) and one record
per applied deposit, holding only the client, the amount and the dispute state.
The record is 20 bytes, or about 78 once its key and the hash table's overhead
are counted. Withdrawals are not retained at all, since nothing can refer back
to one:

| Input (5 million records, ~137 MB) | Time | Peak memory |
| --- | --- | --- |
| Withdrawals only — nothing is disputable | 1.0 s | 7 MB |
| 55% deposits — 2.75 million disputable records | 1.4 s | 216 MB |

Measured on an M4 Max with a release build; the same runs take 2.1 s through
`cargo run`. The second row is the honest worst case, and that growth is
inherent in being able to dispute any earlier deposit, which is why the record
is kept as small as it is.

**Amounts are read from the digits, not guessed at.** Left to itself, the CSV
reader hands a numeric-looking field over as an `f64`, which both rounds amounts
on the way in (`123456789012345.6789` came back as `123456789012345.67`) and
costs time inferring a type per field. The amount column is decoded straight
from its text ([`src/transaction.rs`](src/transaction.rs)), which is exact and
cut about 20% off the total runtime.

**The history is hashed with a hasher that cannot be gamed.** Transaction IDs
are `u32`, which a faster non-cryptographic hasher would suit, but they are
chosen by whoever sends the input: that hasher would let a hostile partner pick
IDs that all land in one bucket and turn every lookup into a linear scan.

**One engine per stream, no shared state.** The engine owns its accounts and its
history; there is no global, no lock and no shared mutability in the crate.
Reading is generic over `Read`, so the same code path serves a file, a socket or
a test fixture. A server taking thousands of concurrent streams gives each one
its own engine and runs them in parallel with no coordination at all.

## Maintainability

**One transaction travels in a straight line.** The modules are worth reading in
the order it takes through them:

```text
CSV row --> input --> transaction --> engine --> account --> output --> CSV row
```

| File | Responsibility |
| --- | --- |
| `src/main.rs` | The CLI: argument handling, streaming the input into the engine, reporting errors |
| `src/lib.rs` | The map of the crate: the diagram above, and what each module is for |
| `src/transaction.rs` | The data model of a transaction and of the history a dispute refers back to |
| `src/input.rs` | Reading transactions from a CSV, as a lazy stream |
| `src/account.rs` | A client's account: its balances and the checked operations that move them |
| `src/engine.rs` | All transaction logic: deposits, withdrawals, disputes, chargebacks |
| `src/output.rs` | Writing the resulting account state as CSV |
| `tests/cli.rs` | End-to-end tests of the binary: its streams and exit status |

**Every rule about money lives in one module, and every rule about balances in
another.** To see what a chargeback does, `src/engine.rs` has one short method
per transaction type and nothing else. To see that the arithmetic is sound,
`src/account.rs` is barely a hundred lines. Neither can be broken by a change to
the CSV handling at either end.

**The reasoning sits on the code it explains.** Each decision the specification
leaves open is documented on the method that implements it; the
[Assumptions](#assumptions) below summarize those comments rather than replacing
them. Names come from the domain (`deposit`, `hold`, `release`, `reverse`), and
amounts, client IDs and transaction IDs are named types rather than bare
integers.

**Guardrails are declared, not remembered.** The lints in
[`Cargo.toml`](Cargo.toml) mean the panic-free, overflow-checked and float-free
rules cannot be broken by someone who has not read this file, and `missing_docs`
means a new public item without an explanation fails the build. There is one
place the CSV reader is configured, one place a balance can change and one place
an error becomes a message, so a change lands in exactly one spot.

## Assumptions

The specification leaves a few cases open. They are resolved the way a bank
would resolve them, and the reasoning is documented on the code that implements
each decision, in `src/engine.rs`:

- **Only deposits can be disputed.** A dispute moves funds from available to
  held, which only makes sense for money that was paid in. A dispute over a
  withdrawal is treated like one over an unknown transaction and ignored.
- **A dispute, resolve or chargeback must come from the client that owns the
  referenced transaction.** Otherwise one client could freeze another's account.
- **A frozen account accepts no further transactions of any kind.** A chargeback
  ends the account's activity until a human intervenes. Funds held for a dispute
  that was still open at that moment therefore stay held: money in the middle of
  a claim is exactly what should not move on its own.
- **A deposit reusing the ID of an earlier one is ignored.** IDs are globally
  unique, so a repeated one is an error on the partner's side, and honouring it
  would make a later dispute ambiguous. Withdrawals are not recorded at all, so
  their IDs are not tracked.
- **A deposit or withdrawal opens the account of the client it names**, even when
  the transaction itself is rejected. Disputes, resolves and chargebacks never
  open an account, since they refer back to a transaction whose client already
  has one.
- **A dispute may drive the available funds negative**, when the disputed deposit
  has already been withdrawn. The total must not change, so the held amount comes
  out of the available funds whether they cover it or not.
- **A deposit or withdrawal without an amount, or with a negative one, is
  ignored.** A negative deposit is a withdrawal in disguise and would bypass the
  check that the available funds cover it. Zero is neither, and is applied: it
  moves no money, but it still takes its transaction ID.
- **The `type` column is read without regard to its capitalization.** The case of
  the word carries no meaning in this format, so treating `Deposit` as malformed
  would discard an entire file over an unambiguous spelling. A word that is not
  one of the five still ends the run.
- **An amount carrying more than four decimal places is cut to four**, not
  rounded. This only bites on a malformed row, and cutting keeps every balance
  exactly as precise as the report that prints it, so the reported `available`
  and `held` always add up to the reported `total`.
- **A row carrying a field beyond the four columns is read, and the extra field
  dropped.** Accepting a row that stops after `tx` — which a dispute may — means
  accepting a longer one too. A header that fails to name one of the four
  columns is a different matter and aborts the run, since it is what the fields
  are read by.

## Prompt log

This project was written with the help of Claude Code, and
[`PROMPTS.md`](PROMPTS.md) is the record of that: every prompt that asked for a
change to the repository, verbatim, in order, and stamped with the time it was
given. The git history says what changed; the prompt log says what was asked
for.
