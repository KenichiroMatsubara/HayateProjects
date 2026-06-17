---
name: to-issues
description: Break a plan, spec, or PRD into independently-grabbable issues on the project issue tracker using tracer-bullet vertical slices. Use when user wants to convert a plan into issues, create implementation tickets, or break down work into issues.
---

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

- **AFK** slices are implemented and merged with no human in the loop. To keep them that way they MUST avoid magic numbers: extract every tunable value into a named constant so the structure ships complete and the constants stand ready as the knobs a human can later turn. An AFK slice that would otherwise need a human tuning round is not done — it has leftover magic numbers.
- **完全人力 (fully-manual)** slices are done entirely by a human with no AI involvement: constant/parameter tuning, taste calls, design review, anything where AI in the loop is just noise.

When work seems to need a feedback loop, split it instead of marking it HITL: an AFK slice that lands the structure with named constants, followed by a 完全人力 slice that tunes those constants. Prefer AFK, and sequence each 完全人力 slice after the AFK slices it builds on.

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
- Do the AFK slices avoid magic numbers, with every tunable value pulled into a named constant?

Iterate until the user approves the breakdown.

### 5. Publish the issues to the issue tracker

For each approved slice, publish a new issue to the issue tracker. Use the issue body template below. These issues are considered ready for AFK agents, so publish them with the correct triage label unless instructed otherwise.

Publish issues in dependency order (blockers first) so you can reference real issue identifiers in the "Blocked by" field.

If the source was an existing issue, tag that parent issue with the `parent` label so it is identifiable as a parent at a glance, creating the label on first use if it does not exist yet:

```sh
gh label create "parent" --color 5319E7 --description "親イシュー — broken into child issues" 2>/dev/null || true
gh issue edit <parent-number> --add-label "parent"
```

<issue-template>
## Parent

A reference to the parent issue on the issue tracker (if the source was an existing issue, otherwise omit this section).

## What to build

A concise description of this vertical slice. Describe the end-to-end behavior, not layer-by-layer implementation.

Avoid specific file paths or code snippets — they go stale fast. Exception: if a prototype produced a snippet that encodes a decision more precisely than prose can (state machine, reducer, schema, type shape), inline it here and note briefly that it came from a prototype. Trim to the decision-rich parts — not a working demo, just the important bits.

## Acceptance criteria

- [ ] Criterion 1
- [ ] Criterion 2
- [ ] Criterion 3

## Blocked by

- A reference to the blocking ticket (if any)

Or "None - can start immediately" if no blockers.

</issue-template>

Do NOT close any parent issue or change its content. The only modification allowed on a parent issue is adding the `parent` label described above.
