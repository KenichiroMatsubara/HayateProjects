---
name: make-pr
description: Create a pull request that automatically links the issue worked on in this session so that merging closes it (GitHub closing keyword, Closes #123), and check/resolve merge conflicts against the base branch before merge. Use when the user wants to open a PR tied to an issue, auto-close an issue on merge, link a PR to a ticket, asks "Issue と紐付けてプルリクを作って", "マージしたら Issue を閉じたい", "コンフリ解消して", or invokes /make-pr.
---

# Make PR

Open a pull request that:

1. **Auto-links the issue handled in this session** so merging it closes the
   issue automatically (via a GitHub closing keyword in the PR body), and
2. **Is verified mergeable** — check whether the branch will conflict with the
   base branch and resolve the conflicts before it can be merged cleanly.

## How the issue linkage works

GitHub closes an issue when a PR **merged into the default branch** contains a
closing keyword followed by the issue reference in its **description or a commit
message**. Supported keywords (case-insensitive):

```
close   closes   closed
fix     fixes    fixed
resolve resolves resolved
```

References may be `#123` (same repo) or `owner/repo#123` (cross-repo).
List multiple issues separately — `Closes #1, closes #2` (one keyword each).

> Only a merge into the **default branch** closes the issue. A PR targeting a
> non-default base will link but not auto-close until that branch merges to default.

## Process

### 1. Identify the issue handled in this session

Determine which issue this work addresses **from the session itself** — the
issue the user picked up, referenced, or that drove the changes on this branch.
Prefer this automatic detection over asking. Fall back to an explicit argument
or URL only when the session has no clear issue.

Confirm it with `mcp__github__issue_read` to verify it exists and read its
title/scope so the PR actually addresses it. If multiple issues were handled,
link each one.

### 2. Prepare the branch and changes

- Make sure the work is committed on the designated feature branch (never the
  default branch) and pushed to origin.
- If there are no changes yet, implement the issue first, then commit and push.

### 3. Check for merge conflicts against the base branch

Before (or right after) opening the PR, verify the branch merges cleanly into
the base (default) branch and resolve any conflicts:

1. Fetch the latest base branch: `git fetch origin <base>`.
2. Detect conflicts without committing a merge:
   ```bash
   git merge --no-commit --no-ff origin/<base>
   ```
   - **No conflict** → `git merge --abort` and continue; the branch is clean.
   - **Conflict** → the command reports conflicted paths (also `git diff
     --name-only --diff-filter=U`). Resolve them:
     1. Open each conflicted file and reconcile both sides, preserving the intent
        of this branch's changes **and** the base branch's changes. Remove all
        `<<<<<<<`, `=======`, `>>>>>>>` markers.
     2. `git add` the resolved files.
     3. Commit the merge (`git commit`) and `git push -u origin <branch>`.
3. After resolving, re-run the check to confirm the branch is now conflict-free.

> If a conflict resolution is ambiguous or risks dropping someone else's work
> (e.g. large or architecturally significant changes), surface it to the user
> with `AskUserQuestion` instead of guessing.

For an **already-open** PR, you can also confirm mergeability via
`mcp__github__pull_request_read` (`mergeable` / `mergeable_state`) and bring the
branch up to date with `mcp__github__update_pull_request_branch`.

### 4. Create the PR with a closing keyword

Open the PR with `mcp__github__create_pull_request`. The body **must** contain a
closing keyword line referencing the session's issue:

```md
## Summary
<what this PR does>

Closes #123
```

Checklist:

- [ ] `head` = feature branch, `base` = repo default branch
- [ ] PR body contains `Closes #<n>` (or fixes/resolves) on its own line
- [ ] Issue number matches the issue handled in this session (step 1)
- [ ] One keyword per issue if closing several
- [ ] Branch merges cleanly into base (step 3) — no conflicts

### 5. Verify the link and mergeability

After creation, confirm via `mcp__github__pull_request_read` that:

- the linked issue shows under "Development" / the body keyword is present, and
- `mergeable_state` is clean (no conflicts).

Report the PR URL, which issue it will close on merge, and that it is
conflict-free.

## If the PR already exists

Edit the description with `mcp__github__update_pull_request` to add the
`Closes #<n>` line — GitHub re-parses the body and links the issue. Then run the
conflict check (step 3) and update the branch if it has fallen behind base.

## Notes

- Do NOT create a PR unless the user asked for one.
- Keyword in the **PR description** is preferred over commit messages (visible,
  editable, survives squash).
- Issue and PR must live in the same repository for `#123`; use
  `owner/repo#123` otherwise.
- Always resolve conflicts so the merge preserves **both** branches' intent;
  when in doubt, ask rather than overwrite.
