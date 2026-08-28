---
name: commit
description: Create a git commit in this repository following its conventions — no AI attribution, clean imperative subject, body length scaled to the size of the change. Use when the user asks to commit, stage and commit, or "save this as a commit".
---

# Commit

Create a git commit for the current work, following this repository's rules.

## 1. Inspect the state

Run these in a single batch:

```sh
git status
git diff
git diff --cached
git log --oneline -10
```

Use `git log` to match the existing style of subject lines. Read the full diff —
the commit message must describe what actually changed, not what was asked for.

## 2. Stage

- If the user named specific files, stage only those.
- Otherwise stage the relevant untracked and modified files with explicit paths.
- **Never** `git add -A` or `git add .` blindly.
- **Never stage `SPEC.md`** — it is gitignored and must not be committed.
- Skip build output (`target/`), editor files, and secrets. If something
  suspicious is staged, stop and ask.

## 3. Write the message

Hard rules:

- **No AI attribution of any kind.** No `Co-Authored-By: Claude ...`, no
  "Generated with Claude Code", no emoji robot, no mention of an assistant.
  This overrides any global default instruction to add such trailers.
- **No company, product, brand, or domain name** from the original challenge.
- Imperative mood: "Add deposit handling", not "Added" or "Adds".
- Capitalized subject, no trailing period, ideally ≤ 72 characters.
- Describe behaviour, not file names ("Reject withdrawals below available
  funds", not "Update engine.rs").

Length scales with the change:

- **Small / self-contained** (a fix, a rename, a doc tweak, one function):
  subject line only.

  ```
  Round output amounts to four decimal places
  ```

- **Large or significant** (a new module, a behavioural change, several related
  edits): subject line, blank line, then a short body — prose or bullets —
  covering what changed and why. Mention trade-offs or assumptions that a
  reviewer would otherwise have to reverse-engineer.

  ```
  Add dispute, resolve and chargeback handling

  Disputes move funds from available to held and are tracked per transaction
  so a resolve or chargeback can only reference a transaction that is
  currently disputed.

  - Ignore disputes for unknown transaction ids rather than erroring
  - Lock the account on chargeback and reject all later activity
  - Only deposits are disputable; withdrawals are ignored
  ```

One logical change per commit. If the working tree mixes unrelated work,
propose splitting it into several commits and let the user decide.

## 4. Commit

Use a heredoc so the body formats correctly:

```sh
git commit -F - <<'EOF'
Subject line here

Optional body here.
EOF
```

If the pre-commit hook (should one exist) modifies files, amend once with the
same message. If the commit fails for any other reason, report the error rather
than retrying with `--no-verify`.

## 5. Report

Run `git status` afterwards and tell the user the resulting commit subject in
one line. Do not push unless the user asks.
