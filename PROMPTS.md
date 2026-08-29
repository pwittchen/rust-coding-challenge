# Prompts

Log of prompts given to Claude Code in this repository.

## 2026-08-28 23:53:17

in the CLAUDE.md add note that each prompt I create should be saved in the PROMPTS.md file with date time and message

## 2026-08-28 23:57:02

remove all automatically provided specs from the PDF during the init in the CLAUDE.md file, analyze pdf file and then save it as a markdown in the .claude/SPEC.md file after that remove all deteails regarding any specified company names, brands, etc. Keep appropriate markdown formatting like headers, tables, code snippets, links, etc. When you're ready then delete pdf file.

## 2026-08-29 00:00:53

move SPEC.md file from the .claude/ sub-dir into the main dir and add reference to it in the CLAUDE.md

## 2026-08-29 00:06:40

In the CLAUDE.md add assumptions that each commit performed by claude should not contain any co-authored by AI text, should be clean and descriptive. When there is a lot of changes or changes are significant then commit message could be longer. When changes are smaller then it can be shorter. In addition to that, please create CC commit skill.

## 2026-08-29 00:07:53

/commit

## 2026-08-29 00:11:05

add libraries mentioned as useful in the SPEC.md to the Cargo.toml dependencies

## 2026-08-29 00:11:44

/commit

## 2026-08-29 00:15:22

I see the following warning for my CI GH action: build
Node.js 20 is deprecated. The following actions target Node.js 20 but are being forced to run on Node.js 24: actions/checkout@v4. For more information see: https://github.blog/changelog/2025-09-19-deprecation-of-node-20-on-github-actions-runners/ - can you have a look at it and fix it?

## 2026-08-29 00:15:51

/commit

## 2026-08-29 12:38:54

extract contents of the transactions.csv from the SPEC.md into the transactions.csv file

## 2026-08-29 12:39:17

/commit

## 2026-08-29 12:42:10

According to the SPEC.md, define all necessary data structures which are needed to resolve and implement this solution. I mean transaction, transactions, and transaction types. Also initialize empty transaction list.

## 2026-08-29 12:45:07

/commit

## 2026-08-29 12:51:11

Now, implement logic of loading transactions.csv file into the program, ensure you keep the contract described in the SPEC.md. Do not apply any logic yet. Just implement loading transactions from the file into the program. Keep csv file loading logic in the separate file to not mix it with the currently existing transactions.rs file to keep separation of the program responsibilities clear. Right now, due to the fact that transaction logic is not imeplement or being implemented yet, output empty accounts.csv file when user runs the program, but inside the program load all the transactions to the data structure.

## 2026-08-29 12:54:22

/commit

## 2026-08-29 12:55:55

in the CLAUDE.md add info that after implementing each change, we need to run cargo fmt, cargo build and cargo test, to ensure that program is properly formatted, builds and tests are passing

## 2026-08-29 12:56:21

/commit

## 2026-08-29 13:59:21

According to the description in the SPEC.md implement transaction logic. Keep all the logic in the separate file (engine.rs). Also implement printing output result to the file according to the guidelines in the SPEC.md so in the sample accounts.csv file should contain appropriate output data.

## 2026-08-29 14:06:06

/commit

## 2026-08-29 14:08:13

create README.md file with brief description of the program, information how to build it, format it, execute tests and run with sample transaction process. Also add info about project prerequisites needed to build and run the program.

## 2026-08-29 14:09:21

/commit

## 2026-08-29 14:10:03

update CLAUDE.md file according to the current project state

## 2026-08-29 14:12:47

in the CLAUDE.md add note that SPEC.md is ignored by the git, available only locally in the repo, but not pushed publicly

## 2026-08-29 14:13:11

/commit

## 2026-08-29 14:15:45

Please have a deep look at the all cases regarding transactions described in the SPEC.md and ensure all of them are covered. If you can figure out any other cases, which should be covered, also cover them, but first be sure that criteria in the SPEC.md are met. Ensure that every corner case regarding transaction logic, happy paths and negative paths are covered by tests. If any test is missing, then add it. Your goal is to make logic robust.

## 2026-08-29 14:24:06

add info about cargo clean to README.md

## 2026-08-29 14:25:33

have a look at the Scoring section in the SPEC.md - ensure that categories Basics and Completeness are covered properly

## 2026-08-29 14:30:20

/commit

## 2026-08-29 14:32:11

Extract code related to the account from the engine.rs and place it in the new account.rs file

## 2026-08-29 14:34:08

/commit

## 2026-08-29 14:35:44

Now ensure that criteria Safety and Robustness from the Scoring section in the SPEC.md are met

## 2026-08-29 14:42:11

/commit

## 2026-08-29 14:44:25

Have a look at the Efficiecty requirement in the Scoring section of the SPEC.md and esure appropriate criteria are met here according to this description

## 2026-08-29 14:55:42

/commit

## 2026-08-29 14:58:20

Please have a look at the Maintainability sub-section in the Scoring section in the SPEC.md and ensure its requirements are met

## 2026-08-29 15:02:43

/commit

## 2026-08-29 15:04:28

create score CC skill which will evalue project quality according to the points described in the Scoring section in the SPEC.md

## 2026-08-29 15:06:29

/commit

## 2026-08-29 15:08:32

In the CLAUDE.md there's an info about prompt log. Modify it, so when the user asks question about the project without intention of the modifying it, then prompt should not be logged into the file. Only prompts related to modification of the project, its source, tests and documenations should be logged. Pure questions should not be loggged.

## 2026-08-29 15:09:10

/commit

## 2026-08-29 15:16:17

Apply all fixes found by the /score skill

## 2026-08-29 15:27:12

run cargo lint command and apply fixes found by this command

## 2026-08-29 15:28:46

/commit

## 2026-08-29 15:34:54

apply fixes for the found issues

## 2026-08-29 15:42:38

/commit

## 2026-08-29 15:45:49

/commit
