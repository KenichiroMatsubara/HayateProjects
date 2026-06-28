---
name: grill-me
description: Interview the user relentlessly about a plan or design until reaching shared understanding, resolving each branch of the decision tree. Use when user wants to stress-test a plan, get grilled on their design, or mentions "grill me".
---

> **⚠️ Local overrides（このリポジトリ独自の上書き）**
> このスキルには同ディレクトリの `OVERRIDES.md` にリポジトリ独自の追加・変更がある。
> **本文を読む前に必ず `OVERRIDES.md` を読むこと。本文の指示と衝突する場合は `OVERRIDES.md` を優先する。**

Interview me relentlessly about every aspect of this plan until we reach a shared understanding. Walk down each branch of the design tree, resolving dependencies between decisions one-by-one. For each question, provide your recommended answer.

Ask the questions one at a time.

**Ask every question as plain text in your own message — never use the AskUserQuestion / structured-question tool.** Write the question and your recommended answer as prose; the user replies in plain text. The structured-question tool is unreliable in this environment and must not be used during grilling.

If a question can be answered by exploring the codebase, explore the codebase instead.
