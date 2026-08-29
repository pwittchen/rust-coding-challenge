---
name: score
description: Evaluate this repository against the challenge's scoring criteria (Basics, Completeness, Correctness, Safety and Robustness, Efficiency, Maintainability) and report a grade with evidence per category. Use when the user asks to score, grade, assess or review how well the project meets the challenge criteria.
---

# Score

Assess the project the way a reviewer would: build it, run it, read it, and
grade it against each scoring category. This is a **read-only assessment** —
report findings, do not fix them unless the user asks afterwards.

The question this answers is "is it good enough to submit?", not "what else
could be changed?". A mature project reaches a point where the remaining
observations are matters of taste, and the right answer is to stop. Report what
would genuinely cost points; note anything else in passing and leave it alone.

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
documented assumption in the README, or matches neither. Only the third case is
a finding, and it is a critical or important one — behaviour the README already
explains is a decision, not a defect, however you would have decided it.

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
- **Findings**, if there are any, each classified by severity below.

### Severity

The point of the grade is to decide whether the project is finished, so every
finding has to be sorted into one of three buckets. Be strict about what earns
the top two.

- **Critical** — it costs points, or breaks something a reviewer will run.
  Wrong output for a valid input; a violated CLI contract; a panic, a failed
  build, a failing test, a clippy denial; a transaction case not handled at all;
  memory that grows with the size of the input rather than with the state that
  has to be kept. Must be fixed.
- **Important** — a real gap a competent reviewer would probably mark down.
  A documented claim that is not true of the code; a corner case the spec names
  that nothing exercises; a complicated path with no test; behaviour that would
  plausibly fail an automated grader's sample. Worth fixing.
- **Minor** — everything else. Wording, prose length, naming, formatting of
  documentation, a micro-optimization, a different-but-equivalent design, a
  ratio of tests to code, a preference about how something is phrased or
  organized. **Mention these in one line and move on.** They are not defects,
  they do not lower a grade, and they do not go in the fix list.

When unsure between Important and Minor, it is Minor. A finding is only
Important if you can name the concrete consequence — what a reviewer would
observe, or what input would misbehave. "A reviewer might prefer…" is Minor by
definition.

### Judgement

Do not invent faults. A documented, defensible choice is not a defect, and
neither is a deliberate deviation the README explains. A category with no
critical or important findings is graded Strong with nothing listed against it —
that is a normal outcome for finished work, not a failure to look hard enough,
and padding it with minor observations to look thorough is exactly the thing
this section exists to prevent.

Equally, do not soften a real one: a critical finding stays critical however
polished the surrounding code is.

### When the project is done

If a pass turns up no critical and no important findings, say so plainly in the
verdict — the project meets the criteria, the corner cases are covered, and **no
further code changes are warranted**. Recommend stopping. Do not assemble a fix
list out of minor observations to give the next session something to do; the
correct output at that point is a grade, the evidence for it, and a
recommendation to leave the code alone.

## 5. Report

Output to the terminal, in this shape:

1. A one-line overall verdict. When there is nothing critical or important, say
   so here — that the project meets the criteria and needs no further changes.
2. A table: Category | Grade | One-line rationale.
3. A short section per category with the evidence, and any critical or important
   findings against it.
4. **Fixes worth making** — the critical and important findings only, highest
   value first, each naming the file it touches and what it would change. Omit
   this section entirely when there are none; do not write "none" and then list
   things anyway.
5. **Minor observations** — at most a handful of one-line notes, under a heading
   that says plainly they are not worth acting on. Skip the section if nothing
   comes to mind. Never expand one into a paragraph of justification.

Keep it terminal text unless the user asks for a document. Do not create a
report file in the repo, do not commit anything, and do not mention the hiring
company, its products or its domains anywhere in the output.

If the user then asks for the fixes, work through the fixes-worth-making list
only — a request to "apply the fixes" does not extend to the minor observations
unless the user names one. Run `cargo fmt`, `cargo build`, `cargo test` and
`cargo lint` before reporting any of it as done.
