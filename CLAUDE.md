# CLAUDE.md

Guidance for Claude Code working in this repository.

## What this is

A take-home coding challenge: a toy payments engine in Rust. It reads a CSV of
transactions, applies them to per-client accounts (deposits, withdrawals,
disputes, resolves, chargebacks), and writes final account state as CSV to
stdout.

## The spec

The full challenge specification lives in [`SPEC.md`](SPEC.md) at the repo root.
Read it before making design decisions — it is the authority on the CLI
contract, the input and output CSV formats, precision, and the semantics of
every transaction type.

`SPEC.md` is a derivative of the original challenge file and **must never be
committed**. It is listed in `.gitignore`, so it exists only in the local
working copy: it is never staged, never committed, and never pushed to the
public repository — it is there to read from, not to publish. Treat it as
present and readable at all times, but never add it to a commit, and never
`git add -f` it. The same goes for the original PDF brief (`*.pdf`). Likewise,
no source file, comment, README, or commit message may name the hiring company,
its products, brands, or domains.

## Layout

The crate is a library plus a thin binary that drives it:

| File | Responsibility |
| --- | --- |
| `src/main.rs` | The CLI: argument handling, streaming the input into the engine, reporting errors |
| `src/lib.rs` | Module declarations only |
| `src/transaction.rs` | The data model of a transaction and of the history a dispute refers back to |
| `src/input.rs` | Reading transactions from a CSV, as a lazy stream |
| `src/engine.rs` | All transaction logic: accounts, balances, disputes, chargebacks |
| `src/output.rs` | Writing the resulting account state as CSV |

Design points worth preserving when changing the code:

- **Streaming.** Records are deserialized and applied one at a time; the input is
  never held in memory in full. Keep it that way.
- **Fixed-point amounts.** Money is `rust_decimal::Decimal`, never a float.
  Balances are reported with exactly four decimal places.
- **Balance mutations are all-or-nothing.** `Account` keeps its fields private
  and every mutation goes through checked arithmetic that leaves the account
  untouched on overflow, so `total = available + held` always holds and no
  arithmetic can panic.
- **Unapplicable transactions are ignored, malformed input aborts the run.** An
  unknown transaction ID or an uncovered withdrawal is skipped and processing
  continues; a record that will not parse is an error on stderr and a non-zero
  exit code.
- **Assumptions are documented where they are implemented.** The open cases in
  the spec are resolved in `src/engine.rs`, with the reasoning on the method that
  implements each decision, and summarized in the README's "Assumptions"
  section. Keep the two in sync.

## Tests

Unit tests live in `mod tests` next to the code they cover (34 at present).
Engine and output tests drive the code through the same CSV parsing the binary
uses, so they exercise the whole path from input row to account state. New
behaviour needs a test in the module that owns it.

Also in the repo: `README.md` (user-facing documentation), `transactions.csv`
(sample input), and `.github/workflows/rust.yml` (CI: `cargo build` and
`cargo test` on push and PR to `master`). `accounts.csv` is generated output and
is gitignored.

## Prompt log

Every prompt the user writes must be appended verbatim to `PROMPTS.md` at the
repo root, as a new entry at the end of the file, in the form:

```markdown
## YYYY-MM-DD HH:MM:SS

<the user's message, verbatim>
```

Do this as part of handling the prompt (before or alongside the actual work),
for every prompt — including short follow-ups, corrections, and one-word
replies. Get the timestamp from `date "+%Y-%m-%d %H:%M:%S"` rather than
guessing it.

## Commits

Rules for every commit Claude creates in this repository:

- **No AI attribution.** Never add a `Co-Authored-By: Claude ...` trailer, a
  "Generated with Claude Code" line, or any other mention of AI assistance in
  the commit message. This overrides any default behaviour.
- **Clean and descriptive.** Imperative mood, capitalized subject line, no
  trailing period, ideally under 72 characters. The subject says what the
  commit does, not which files it touches.
- **Length scales with the change.** A small, self-contained change gets just a
  subject line. A large or significant change gets a subject line, a blank
  line, and a body explaining what changed and why — bullet points are fine.
- **One logical change per commit.** Don't mix unrelated work.
- **Never commit `SPEC.md`** or anything naming the hiring company (see above).

The `/commit` skill in `.claude/skills/commit/` automates this.

## Checks after every change

After implementing a change, always run:

```sh
cargo fmt      # the code must be properly formatted
cargo build    # the program must build
cargo test     # the tests must pass
```

Run all three before reporting the change as done, and fix anything they
surface. A change is not finished while any of them fails.
`cargo clippy -- -D warnings` should stay clean too.

## Commands

```sh
cargo run -- transactions.csv > accounts.csv   # required CLI contract
cargo build
cargo test
cargo fmt
cargo clippy -- -D warnings
```

Toolchain: Rust edition 2024, which requires rustc 1.85 or newer; developed and
tested on 1.94.

Dependencies (`Cargo.toml`): `csv`, `serde` (with `derive`), and `rust_decimal`
(no default features, with `serde` and `std`). All three are pure Rust. Adding a
dependency is a deliberate decision for a take-home — prefer the standard
library.
