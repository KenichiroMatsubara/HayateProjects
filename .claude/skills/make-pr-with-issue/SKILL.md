---
name: make-pr-with-issue
description: Create a pull request that is linked to an issue so that merging the PR automatically closes the issue, using GitHub closing keywords (Closes #123). Use when the user wants to open a PR tied to an issue, auto-close an issue on merge, link a PR to a ticket, asks "Issue と紐付けてプルリクを作って", "マージしたら Issue を閉じたい", or invokes /make-pr-with-issue.
---

# Make PR With Issue

Open a pull request that closes its issue automatically when merged, by placing a
GitHub **closing keyword** in the PR body.

## How the linkage works

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

### 1. Resolve the issue

Get the issue number from the user (argument, URL, or conversation). Fetch it
with `mcp__github__issue_read` to confirm it exists and read its title/scope so
the PR actually addresses it.

### 2. Prepare the branch and changes

- Make sure the work is committed on the designated feature branch (never the
  default branch) and pushed to origin.
- If there are no changes yet, implement the issue first, then commit and push.

### 3. Create the PR with a closing keyword

Open the PR with `mcp__github__create_pull_request`. The body **must** contain a
closing keyword line referencing the issue:

```md
## Summary
<what this PR does>

Closes #123
```

Checklist:

- [ ] `head` = feature branch, `base` = repo default branch
- [ ] PR body contains `Closes #<n>` (or fixes/resolves) on its own line
- [ ] Issue number matches the issue from step 1
- [ ] One keyword per issue if closing several

### 4. Verify the link

After creation, confirm via `mcp__github__pull_request_read` that the linked
issue shows under "Development" / the body keyword is present. Report the PR URL
and which issue it will close on merge.

## If the PR already exists

Edit the description with `mcp__github__update_pull_request` to add the
`Closes #<n>` line — GitHub re-parses the body and links the issue.

## Notes

- Do NOT create a PR unless the user asked for one.
- Keyword in the **PR description** is preferred over commit messages (visible,
  editable, survives squash).
- Issue and PR must live in the same repository for `#123`; use
  `owner/repo#123` otherwise.
