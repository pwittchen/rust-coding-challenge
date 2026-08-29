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

The debug profile is built with the optimizer on ([`Cargo.toml`](Cargo.toml)),
because the engine is normally run through `cargo run`, and an unoptimized build
takes about eight times as long on the same input. The debug assertions and the
arithmetic overflow checks stay on, so the profile still does what it is for.

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
a failed run is never mistaken for an empty report. See
[Safety and robustness](#safety-and-robustness) for what is treated as an error
and what is merely ignored.

## Formatting and linting

```sh
cargo fmt              # format the code
cargo fmt --check      # verify the formatting without changing anything
cargo clippy --all-targets -- -D warnings
```

The lints that keep the code free of `unsafe`, of panics, and of floating-point
arithmetic are declared in [`Cargo.toml`](Cargo.toml) and described under
[Safety and robustness](#safety-and-robustness); the one that keeps every public
item documented is described under [Maintainability](#maintainability). CI runs
the formatting check, the lints, and the tests on every push and pull request.

## Testing

```sh
cargo test
```

The unit tests live next to the code they cover: argument handling in
`src/main.rs`, parsing and reading the input in `src/transaction.rs` and
`src/input.rs`, the balance arithmetic in `src/account.rs`, every transaction
type and its edge cases in `src/engine.rs`, and the shape and precision of the
report in `src/output.rs`. The engine tests
drive the engine through the same CSV parsing the binary uses, so they exercise
the whole path from input row to account state.

`tests/cli.rs` covers what the unit tests cannot see: it runs the compiled
binary and asserts on the contract a caller actually observes — the report on
stdout, the diagnostics on stderr, and the exit status — for a well-formed
input, an input with no rows, a malformed record, an unreadable file, an
invocation that does not name exactly one file, and a report whose destination
closes before it can be written, which is the one failure that can only be
provoked from outside the process.

Every transaction type is covered on both its happy path and its negative ones:
a withdrawal that is not covered, that is covered only by held funds, or that
would leave an already negative balance; a dispute over an unknown transaction,
over a withdrawal, over a deposit that was itself rejected, over one that is
already disputed, or raised by another client; a resolve or a chargeback for a
transaction that is not under dispute, that was already settled, or that belongs
to somebody else; a chargeback that freezes the account and leaves the total
negative; and further transactions arriving on a frozen account. The boundaries
are covered too: the largest client and transaction IDs, four-decimal precision,
amounts finer than that, an amount with more significant digits than a float
could hold — which is read and kept exactly — balances that would overflow, and
the largest balance an account can hold, both rendered directly and reached
through a chargeback that drives it negative, since a balance the engine accepts
but the report cannot print would be a failure on the last line of an otherwise
successful run. One test runs two engines on two threads, which is what a server
serving many streams at once would do. One test runs the sample input above and
asserts the report documented for it, so the example and the code cannot drift
apart.

## Safety and robustness

The rules below are not conventions the code is reviewed against — they are
denied package-wide in [`Cargo.toml`](Cargo.toml) and checked in CI, so breaking
one fails the build.

**No `unsafe`.** `unsafe_code` is set to `forbid`, which not even a local
`#[allow]` can switch off again. Nothing here needs it.

**Nothing outside the tests may panic.** `unwrap`, `expect`, `panic!` and
slice indexing are denied. Every failure is a value the caller has to deal
with, so no input can take the process down part-way through a report, and a
failed run produces a message rather than a backtrace. `main` is the single
place that turns an error into a message on stderr and a non-zero exit status.

The lint is worth reading for exactly what it is: it covers the code in this
crate, and it cannot see inside a dependency. A library function that panics
internally is still a panic, and denying `panic` here does not catch it — the
same gap that makes the rule about checked arithmetic below a separate line
rather than a consequence of this one. Where a dependency's contract has such an
edge, the crate stays off it deliberately: rendering a balance to a fixed number
of decimal places is the one place this arose, and
[`src/output.rs`](src/output.rs) lays the digits out itself rather than asking
the decimal for a precision, because that path builds the text in a fixed 32-byte
buffer and panics on a balance of 28 or more integer digits — a balance an
account will accept. The comment on the function says so, and tests in that
module render `Decimal::MAX` and a chargeback that drives a balance of that size
negative.

**No floats.** `float_arithmetic` is denied, so a balance cannot silently lose
a fraction of a cent: the type system rejects the arithmetic rather than the
reviewer having to spot it. Amounts are `rust_decimal::Decimal`, which
represents four decimal places exactly.

**No arithmetic that can panic.** `arithmetic_side_effects` is denied, so adding
two balances has to be a checked operation. A plain `+` on a decimal panics on
overflow, and denying `panic` does not catch that — it covers the macro, not an
operator that panics inside — so the rule is stated separately rather than
assumed. The single exemption, in [`clippy.toml`](clippy.toml), is negating an
amount: a decimal carries its sign as a flag, so flipping it cannot overflow and
has nothing to check.

**Balance changes are checked and all-or-nothing.** `Account` keeps its
balances private, and every mutation goes through one checked helper that
computes the new available and held amounts, verifies that they and their sum
are representable, and only then stores them. An operation that would overflow
leaves the account exactly as it was and reports that it did nothing, so a
balance is never left half-updated, `total = available + held` always holds,
and no later arithmetic on a stored balance can overflow. The engine has no way
to move money except through those operations.

### How errors are handled

Failures fall into two classes, and they are treated differently on purpose.

**A transaction that cannot be applied is ignored, and processing continues.**
An unknown transaction ID, a resolve for something that is not under dispute, a
withdrawal the available funds do not cover — the specification calls these
errors on the partner's side, and one bad row is no reason to discard the rest
of the file.

**A record that cannot be parsed aborts the run.** An unknown transaction type,
a client ID that does not fit a `u16`, an amount that is not a decimal: the
input is not what it claims to be, so nothing after that point can be trusted
either. The run stops with a message on stderr naming the file, the line, and
what was wrong with it, and exits non-zero.

The report is written only after the whole input has been consumed, so a run
that fails writes **nothing** to stdout — a partial report can never be
mistaken for the account state. A report that cannot be written in full, in
turn, is itself an error rather than a silent truncation. The end-to-end tests
in [`tests/cli.rs`](tests/cli.rs) pin all of this down by running the binary and
asserting on its streams and exit status.

### Resource use under hostile input

No input can exhaust the stack, because there is no recursion anywhere. What an
input can grow is the engine's memory, and only along the two axes measured
under [Efficiency](#efficiency): one account per client, and one record per
deposit that could still be disputed. Neither is bounded by the size of the
file, but the second is bounded only by `u32::MAX` — the range of a transaction
ID. The CSV reader also imposes no limit on the size of a single field, so one
absurdly long field could still allocate. A server taking these streams from the
network would address both outside the engine — a cap on the record size at the
edge, and a persistent, ageing-out store behind the history — rather than by
changing the transaction logic.

## Efficiency

**The input is a stream, not a data structure.** The CSV reader decodes one
record at a time into a buffer it reuses, `main` applies each record as it
arrives, and the report is written at the end from the accounts alone. The size
of the file is not a bound on memory: a 100 GB file is no different from a 100
KB one.

**Memory tracks what can still be disputed, not what has been read.** The engine
keeps one account per client — at most `u16::MAX` of them, 36 bytes each — and
one 20-byte record per applied deposit, holding only the client, the amount, and
where the deposit stands in the dispute lifecycle. Withdrawals are not retained
at all, since nothing can refer back to one. Two runs over inputs of the same
size show the difference:

| Input (5 million records, ~137 MB) | Time | Peak memory |
| --- | --- | --- |
| Withdrawals only — nothing is disputable | 1.0 s | 7 MB |
| 55% deposits — 2.75 million disputable records | 1.4 s | 216 MB |

Measured on an M4 Max with a release build; the same runs take 2.1 s through
`cargo run`. The second row is the honest worst case: transaction IDs are `u32`,
so a stream of distinct deposits can grow the history to `u32::MAX` records.
That growth is inherent in being able to dispute any earlier deposit, which is
why the record is kept as small as it is.

**Amounts are read from the digits, not guessed at.** Left to itself, the CSV
reader decides what each field looks like and hands a numeric-looking one over
as an `f64`, which would both round amounts on the way in — `123456789012345.6789`
came back as `123456789012345.67` — and cost time inferring a type per field.
The amount column is therefore decoded straight from its text
([`src/transaction.rs`](src/transaction.rs)), which is exact and cut about 20%
off the total runtime.

**The history is hashed with a hasher that cannot be gamed.** Transaction IDs
are `u32`, which a faster non-cryptographic hasher would suit — but they are
chosen by whoever sends the input, so that hasher would let a hostile partner
pick IDs that all land in one bucket and turn every lookup into a linear scan.
The standard library's hasher is kept for that reason, not by default.

**One engine per stream, no shared state.** The engine is an ordinary value that
owns its accounts and its history; there is no global, no lock, and no shared
mutability anywhere in the crate. Reading is generic over `Read`, so the same
code path serves a file, a socket, or a test fixture. A server taking thousands
of concurrent streams gives each one its own engine and runs them in parallel
with no coordination at all — a unit test in [`src/engine.rs`](src/engine.rs)
does exactly that across threads. The bound worth watching in that setting is
the disputable history summed over the live streams, which a server would
address with a persistent, ageing-out store rather than by changing the
transaction logic.

## Maintainability

The code is meant to be read by someone the author cannot answer questions to,
so it is arranged to be read rather than merely to work.

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
`src/account.rs` is barely a hundred lines and every mutation funnels through a
single checked helper. Neither can be broken by a change to the CSV handling at
either end, and the CSV handling knows nothing about disputes.

**The reasoning sits on the code it explains.** Each decision the specification
leaves open is documented on the method that implements it, so the "why" is
found by reading the "what" — the [Assumptions](#assumptions) below summarize
those comments rather than replacing them. Names come from the domain
(`deposit`, `hold`, `release`, `reverse`, `disputable`) and amounts, client IDs
and transaction IDs are named types rather than bare integers.

**Guardrails are declared, not remembered.** The lints in
[`Cargo.toml`](Cargo.toml) mean the panic-free, overflow-checked and float-free
rules cannot be broken by someone who has not read this file, and `missing_docs`
means a new public item without an explanation fails the build. There is one
place the CSV reader is configured, one place a balance can change, and one
place an error becomes a message — so a change lands in exactly one spot.

**The documentation is tested.** The sample input and the report shown under
[Running](#running) are asserted by a test in `src/output.rs`, so the example a
reader trusts cannot quietly drift away from what the program does.

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
  is the end of the account's activity until a human intervenes. Funds held for
  a dispute that was still open at that moment therefore stay held: no later
  resolve can release them. That is the point of a freeze — money in the middle
  of a claim is exactly what should not move on its own while the account is
  waiting to be looked at.
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
- **A row carrying a field beyond the four columns is read, and the extra field
  dropped.** Accepting a row that stops after `tx` — which a dispute may — means
  accepting a longer one too, and that is the right way round: all four columns
  are still required by name and still parsed, so a row with something extra
  alongside them is not ambiguous. A header that fails to name one of the four
  is a different matter and aborts the run, since it is what the fields are read
  by.

Anything the engine cannot apply is ignored and processing continues, as the
specification requires. A malformed CSV record, on the other hand, aborts the
run: it means the input itself cannot be trusted.
