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
committed**; it is listed in `.gitignore`. Likewise, no source file,
comment, README, or commit message may name the hiring company, its products,
brands, or domains.

Status: as of the initial commit the repo is a bare `cargo new` scaffold
(`src/main.rs` still prints "Hello, world!").

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

## Commands

```sh
cargo run -- transactions.csv > accounts.csv   # required CLI contract
cargo build
cargo test
cargo fmt
cargo clippy -- -D warnings
```

Toolchain: Rust edition 2024 (rustc 1.94+). No dependencies declared yet;
`serde` and `csv` are the expected additions.
