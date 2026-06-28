---
name: to-issues
description: Break a plan, spec, or PRD into independently-grabbable issues on the project issue tracker using tracer-bullet vertical slices. Use when user wants to convert a plan into issues, create implementation tickets, or break down work into issues.
---

> **⚠️ Local overrides（このリポジトリ独自の上書き）**
> このスキルには同ディレクトリの `OVERRIDES.md` にリポジトリ独自の追加・変更がある。
> **本文を読む前に必ず `OVERRIDES.md` を読むこと。本文の指示と衝突する場合は `OVERRIDES.md` を優先する。**

# To Issues

Break a plan into independently-grabbable issues using vertical slices (tracer bullets).

The issue tracker and triage label vocabulary should have been provided to you — run `/setup-matt-pocock-skills` if not.

## Process

### 1. Gather context

Work from whatever is already in the conversation context. If the user passes an issue reference (issue number, URL, or path) as an argument, fetch it from the issue tracker and read its full body and comments.

### 2. Explore the codebase (optional)

If you have not already explored the codebase, do so to understand the current state of the code. Issue titles and descriptions should use the project's domain glossary vocabulary, and respect ADRs in the area you're touching.

### 3. Draft vertical slices

Break the plan into **tracer bullet** issues. Each issue is a thin vertical slice that cuts through ALL integration layers end-to-end, NOT a horizontal slice of one layer.

Slices are either 'AFK' or '完全人力 (fully-manual)'. **HITL slices are banned** — never create a slice that mixes AI implementation with a human feedback loop (e.g. an agent tuning constants while a human reviews the values).

- **AFK** slices just get the implementation done end-to-end, with no human in the loop. The constant *values* may be arbitrary/placeholder — getting them right is NOT AFK's job. The one rule is that every tunable value MUST be a named constant, never an inline magic number, so a human can tune it afterward. An AFK slice is "done" when the implementation lands, even if the numbers are still guesses.
- **完全人力 (fully-manual)** slices are done entirely by a human with no AI involvement: tuning those constant values, taste calls, design review — anything where AI in the loop is just noise.

When work seems to need a feedback loop, never mark it HITL — that half-measure is pure friction. Split it: an AFK slice that finishes the implementation with named constants at placeholder values, followed by a 完全人力 slice that tunes them. Prefer AFK, and sequence each 完全人力 slice after the AFK slices it builds on.

<vertical-slice-rules>
- Each slice delivers a narrow but COMPLETE path through every layer (schema, API, UI, tests)
- A completed slice is demoable or verifiable on its own
- Prefer many thin slices over few thick ones
</vertical-slice-rules>

### 4. Quiz the user

Present the proposed breakdown as a numbered list. For each slice, show:

- **Title**: short descriptive name
- **Type**: AFK / 完全人力 (fully-manual)
- **Blocked by**: which other slices (if any) must complete first
- **User stories covered**: which user stories this addresses (if the source material has them)

Ask the user:

- Does the granularity feel right? (too coarse / too fine)
- Are the dependency relationships correct?
- Should any slices be merged or split further?
- Are the correct slices marked as AFK and 完全人力 (fully-manual)? (No HITL slices — any feedback-loop work split into AFK + 完全人力.)
- Do the AFK slices avoid magic numbers, with every tunable value pulled into a named constant? (Placeholder values are fine — tuning is a 完全人力 follow-up.)

Iterate until the user approves the breakdown.

### 5. Publish the issues to the issue tracker

For each approved slice, publish a new issue to the issue tracker. Use the issue body template below. These issues are considered ready for AFK agents, so publish them with the correct triage label unless instructed otherwise.

Publish issues in dependency order (blockers first) so you can reference real issue identifiers in the **Blocked by** line at the top of each issue.

The **Blocked by** line goes at the very top of the body on purpose: at execution time it is the only thing a human reads to decide "can I start this now?". Keep it to a bare list of issue references — no prose.

If the source was an existing issue (the parent), do BOTH of the following for each child:

1. Tag the parent with the `parent` label so it is identifiable at a glance (kept for back-compat), creating the label on first use if it does not exist yet:

   ```sh
   gh label create "parent" --color 5319E7 --description "親イシュー — broken into child issues" 2>/dev/null || true
   gh issue edit <parent-number> --add-label "parent"
   ```

2. Register the child as a **native GitHub sub-issue** of the parent. This is what drives GitHub's "N of M done" progress bar in the issue list and the parent auto-close workflow (see [ADR-0003](../../../docs/adr/0003-issue-parent-child-via-native-subissues-autoclose.md)). The sub-issue API needs the child's **REST id** (`.id`), NOT its issue number:

   ```sh
   CHILD_ID=$(gh api repos/{owner}/{repo}/issues/<child-number> --jq .id)
   gh api repos/{owner}/{repo}/issues/<parent-number>/sub_issues -F sub_issue_id="$CHILD_ID"
   ```

   Do NOT write a `## Parent` section in the child body — the native sub-issue relationship replaces it.

<issue-template>
> **Blocked by:** #123, #456

(Use `> **Blocked by:** None — can start immediately` when there are no blockers. This blockquote MUST be the first line of the body.)

## What to build

A concise description of this vertical slice. Describe the end-to-end behavior, not layer-by-layer implementation.

Avoid specific file paths or code snippets — they go stale fast. Exception: if a prototype produced a snippet that encodes a decision more precisely than prose can (state machine, reducer, schema, type shape), inline it here and note briefly that it came from a prototype. Trim to the decision-rich parts — not a working demo, just the important bits.

## Acceptance criteria

- [ ] Criterion 1
- [ ] Criterion 2
- [ ] Criterion 3

</issue-template>

Do NOT close any parent issue or edit its body. The only modifications allowed on a parent are adding the `parent` label and registering children as sub-issues (both above). The parent is closed automatically by the auto-close workflow once all its children close (ADR-0003) — never close it by hand.
