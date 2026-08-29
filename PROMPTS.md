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
