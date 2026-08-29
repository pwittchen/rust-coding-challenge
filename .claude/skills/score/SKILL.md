---
name: score
description: Evaluate this repository against the challenge's scoring criteria (Basics, Completeness, Correctness, Safety and Robustness, Efficiency, Maintainability) and report a grade with evidence per category. Use when the user asks to score, grade, assess or review how well the project meets the challenge criteria.
---

# Score

Assess the project the way a reviewer would: build it, run it, read it, and
grade it against each scoring category. This is a **read-only assessment** —
report findings, do not fix them unless the user asks afterwards.

## 1. Read the criteria

The categories and what each one rewards are defined in the **Scoring** section
of `SPEC.md` at the repo root. Read that section first, every time — it is the
authority, and this file deliberately does not copy it (`SPEC.md` is gitignored
and must never be committed, quoted into a committed file, or summarized into
one). The rest of `SPEC.md` matters too: the CLI contract, the CSV formats,
precision, and the semantics of each transaction type are what "correct" is
measured against.

Grade the categories the spec actually lists, in the order it lists them. At
the time of writing they are Basics, Completeness, Correctness, Safety and
Robustness, Efficiency and Maintainability — if the spec has changed, follow
the spec, not this list.

## 2. Gather evidence

Never grade from reading alone. Run the checks first, in one batch:

```sh
cargo fmt --check
cargo build --release
cargo clippy --all-targets -- -D warnings
cargo test
```

Then exercise the CLI contract exactly as the spec states it:

```sh
cargo run -- transactions.csv
```

Record the actual output, exit status, and anything on stderr. Then probe the
edges, each with a small CSV written to the scratchpad directory (never into
the repo):

- header with spaces after the commas, and rows with padded fields
- amounts at four decimal places, and amounts with more places than that
- a withdrawal larger than the available balance
- a dispute, resolve and chargeback against a known deposit, in sequence
- a dispute, resolve and chargeback naming a transaction ID that never appeared
- a resolve and a chargeback against a transaction that is not under dispute
- activity on an account after a chargeback has locked it
- a duplicate transaction ID
- a malformed row (bad number, unknown type, missing column)
- an empty file, and a file that is a header only
- a missing input file, and no argument at all

For each, note whether the program's behaviour matches the spec, matches a
documented assumption in the README, or matches neither. The third case is a
finding.

For efficiency, generate a large input in the scratchpad — several million rows
spread over the full `u32` transaction ID and `u16` client ID ranges — and
measure it:

```sh
/usr/bin/time -l cargo run --release -- <big-input>.csv > /dev/null
```

Peak resident set is the number that matters: it should track the number of
distinct clients and stored transactions, not the number of input rows. Compare
it against a run over a tenth of the rows to see whether memory grows with file
size. State the measured figures in the report rather than asserting the code
streams.

## 3. Read for the manual half

Automated checks only reach Basics and part of Correctness. Read the source for
the rest:

- `src/` in full — it is small enough. Judge whether a reviewer who cannot ask
  questions could follow it: one concern per module, the reasoning next to the
  code it explains, no duplicated decision points.
- `README.md` — the spec asks the candidate to explain correctness, safety and
  assumptions there. Check that what it claims is actually true of the code,
  and that every assumption resolved in `src/engine.rs` is reflected in it.
- `Cargo.toml` and `clippy.toml` — the lint configuration is the safety
  argument; confirm it is enforced rather than described.
- `tests/cli.rs` and the `mod tests` blocks — check the complicated parts are
  covered (dispute state machine, locked accounts, precision, error paths), not
  just the happy path.
- Dependencies — each one should be justifiable in a take-home.

## 4. Grade

For each category give:

- **A grade**: Strong / Adequate / Weak.
- **Evidence**: the command output, file and line, or measurement behind it.
  `src/engine.rs:120` beats "the engine looks fine".
- **Gaps**: what a reviewer could reasonably mark down, however small.

Be a critic, not a cheerleader. A category with nothing to improve is rare; if
you claim one, the evidence has to carry it. Equally, do not invent faults —
a documented, defensible choice is not a defect, and neither is a deliberate
deviation the README explains.

## 5. Report

Output to the terminal, in this shape:

1. A one-line overall verdict.
2. A table: Category | Grade | One-line rationale.
3. A short section per category with the evidence and gaps.
4. A prioritized list of concrete fixes, highest value first, each naming the
   file it touches and what it would change.

Keep it terminal text unless the user asks for a document. Do not create a
report file in the repo, do not commit anything, and do not mention the hiring
company, its products or its domains anywhere in the output.

If the user then asks for the fixes, work through the prioritized list, and run
`cargo fmt`, `cargo build` and `cargo test` before reporting any of it as done.
