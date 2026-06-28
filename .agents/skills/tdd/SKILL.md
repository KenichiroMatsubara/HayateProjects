---
name: tdd
description: Test-driven development with red-green-refactor loop. Use when user wants to build features or fix bugs using TDD, mentions "red-green-refactor", wants integration tests, or asks for test-first development.
---

> **⚠️ Local overrides（このリポジトリ独自の上書き）**
> このスキルには同ディレクトリの `OVERRIDES.md` にリポジトリ独自の追加・変更がある。
> **本文を読む前に必ず `OVERRIDES.md` を読むこと。本文の指示と衝突する場合は `OVERRIDES.md` を優先する。**

# Test-Driven Development

## Philosophy

**Core principle**: Tests should verify behavior through public interfaces, not implementation details. Code can change entirely; tests shouldn't.

**Good tests** are integration-style: they exercise real code paths through public APIs. They describe _what_ the system does, not _how_ it does it. A good test reads like a specification - "user can checkout with valid cart" tells you exactly what capability exists. These tests survive refactors because they don't care about internal structure.

**Bad tests** are coupled to implementation. They mock internal collaborators, test private methods, or verify through external means (like querying a database directly instead of using the interface). The warning sign: your test breaks when you refactor, but behavior hasn't changed. If you rename an internal function and tests fail, those tests were testing implementation, not behavior.

See [tests.md](tests.md) for examples and [mocking.md](mocking.md) for mocking guidelines.

## Anti-Pattern: Horizontal Slices

**DO NOT write all tests first, then all implementation.** This is "horizontal slicing" - treating RED as "write all tests" and GREEN as "write all code."

This produces **crap tests**:

- Tests written in bulk test _imagined_ behavior, not _actual_ behavior
- You end up testing the _shape_ of things (data structures, function signatures) rather than user-facing behavior
- Tests become insensitive to real changes - they pass when behavior breaks, fail when behavior is fine
- You outrun your headlights, committing to test structure before understanding the implementation

**Correct approach**: Vertical slices via tracer bullets. One test → one implementation → repeat. Each test responds to what you learned from the previous cycle. Because you just wrote the code, you know exactly what behavior matters and how to verify it.

```
WRONG (horizontal):
  RED:   test1, test2, test3, test4, test5
  GREEN: impl1, impl2, impl3, impl4, impl5

RIGHT (vertical):
  RED→GREEN: test1→impl1
  RED→GREEN: test2→impl2
  RED→GREEN: test3→impl3
  ...
```

## Multiple issues in one request

When the request names **multiple issues** — a range like `x-y` (e.g. `3-7`),
or a list like `#3, #5, #8` — handle them **all together in a single branch and
a single pull request**. Do not open a branch or PR per issue.

- Use the one designated branch and one PR for the whole batch.
- Work through the issues **in order** (ascending issue number, or the order
  given), running the full TDD loop below for each one.
- Make a **separate commit for each issue** as you finish it, so the PR history
  shows one commit (or a focused group of commits) per issue. Reference the
  issue in each commit message (e.g. `Closes #3`).
- Advance each issue's progress marker independently (see below): mark an issue
  `status:implementing` when you start it and `status:implemented` when its
  tests are green — don't wait for the whole batch.
- Keep going until **every** issue in the range/list is cleared, then push the
  single branch and (if requested) open the single PR covering all of them.

### 【厳守】途中で止めて確認しない — 全部やり切ってから報告する

**指示されたタスク（単一 issue でも、複数 issue の範囲/リストでも）は、すべて完了し、すべてコミットし終えるまで、絶対に途中で止まってユーザに確認・許可を求めてはならない。**

- 「ここまでやりました、続けますか？」「次に進んでよいですか？」のような**進捗確認・続行許可の問い合わせを一切しない。** 黙って最後まで完遂する。複数 issue で最初の1件を終えた時点で止まって聞くのは重大な違反。**全 issue** を実装・コミットし終えるまで連続して進める。
- **ユーザへの確認・報告は、全タスクをコミットし終えた後に一度だけ行う。**
- 唯一の例外：実装を正しく続行できない**真の阻害要因**（要件が根本的に矛盾し推測で埋められない／不可逆な破壊的操作の承認が要る）に限り停止してよい。単なる進捗確認はこの例外に含まれない。

### issue 1件ごとにコンテキストを要約・圧縮してから次へ

複数 issue を処理するとき、**1件を完了（実装・テスト green・コミット）するたびに、次の issue に取り掛かる前に必ず現在のコンテキストウィンドウを要約して圧縮する。** 要約には「完了した issue 番号と要点・残りの issue・共有中の前提」を残し、不要な探索ログは捨てる。後続 issue を常に整理されたコンテキストで開始する。

（この節はこのリポジトリ独自の上書き。`OVERRIDES.md` 参照。）

## Workflow

### 1. Planning

When exploring the codebase, use the project's domain glossary so that test names and interface vocabulary match the project's language, and respect ADRs in the area you're touching.

Before writing any code:

- [ ] Confirm with user what interface changes are needed
- [ ] Confirm with user which behaviors to test (prioritize)
- [ ] Identify opportunities for [deep modules](deep-modules.md) (small interface, deep implementation)
- [ ] Design interfaces for [testability](interface-design.md)
- [ ] List the behaviors to test (not implementation steps)
- [ ] Get user approval on the plan

Ask: "What should the public interface look like? Which behaviors are most important to test?"

**You can't test everything.** Confirm with the user exactly which behaviors matter most. Focus testing effort on critical paths and complex logic, not every possible edge case.

### 2. Tracer Bullet

Write ONE test that confirms ONE thing about the system:

```
RED:   Write test for first behavior → test fails
GREEN: Write minimal code to pass → test passes
```

This is your tracer bullet - proves the path works end-to-end.

### 3. Incremental Loop

For each remaining behavior:

```
RED:   Write next test → fails
GREEN: Minimal code to pass → passes
```

Rules:

- One test at a time
- Only enough code to pass current test
- Don't anticipate future tests
- Keep tests focused on observable behavior

### 4. Refactor

After all tests pass, look for [refactor candidates](refactoring.md):

- [ ] Extract duplication
- [ ] Deepen modules (move complexity behind simple interfaces)
- [ ] Apply SOLID principles where natural
- [ ] Consider what new code reveals about existing code
- [ ] Run tests after each refactor step

**Never refactor while RED.** Get to GREEN first.

## Progress markers (issue tracker)

When the work corresponds to an issue on the tracker, advance its progress
marker as you go. Markers are GitHub `status:*` labels; "merged" is the issue's
closed state. See [status-markers.md](../../../docs/agents/status-markers.md).

- **On starting** (before the first RED) — set `status:implementing` (remove any
  other `status:*` label, create the label if missing). If `/search-issue`
  already marked it, leave it.
- **On finishing** — once all tests are green, the refactor pass is done, and the
  PR is pushed, move to `status:implemented` (remove `status:implementing`).
- **Do not close the issue yourself.** `closed` means the PR merged; closing is
  the merge step, not part of the TDD loop.

Skip this section entirely when there's no tracker issue (ad-hoc work).

## Checklist Per Cycle

```
[ ] Test describes behavior, not implementation
[ ] Test uses public interface only
[ ] Test would survive internal refactor
[ ] Code is minimal for this test
[ ] No speculative features added
```
