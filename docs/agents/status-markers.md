# Progress Markers (進捗マーカー)

Tracks how far an issue has moved through implementation. **Orthogonal to triage
roles** (`ready-for-agent`, `question`, etc. — see [triage-labels.md](triage-labels.md)):
triage answers "is this ready to pick up?", a progress marker answers "how far
along is it?".

There are four states. Three are GitHub labels; the last is the issue's own
closed state, so "merged?" is answered by "is the issue closed?".

| Marker         | Representation               | Meaning                                                       |
| -------------- | ---------------------------- | ------------------------------------------------------------ |
| `none`         | no `status:*` label          | 未着手 — nobody has started                                   |
| `implementing` | `status:implementing` label  | 実装中 — being worked on (e.g. a `/tdd` session is in flight) |
| `implemented`  | `status:implemented` label   | 実装済み — code done, tests green, PR pushed, awaiting merge  |
| `closed`       | issue is **closed**          | merged — the PR landed and the issue was closed              |

## Rules

- An issue carries **at most one** `status:*` label at a time. Moving forward
  means removing the old label and adding the new one.
- An **open** issue with no `status:*` label is `none`.
- `closed` is never a label — it is the GitHub closed state. Closing an issue
  supersedes any `status:*` label; remove the stale `status:*` label when you
  close (or let it be, the closed state wins).
- Markers only move forward in normal flow:
  `none → implementing → implemented → closed`. Moving backward (e.g. a PR was
  reverted, or work stalled and was abandoned) is allowed — just set the label
  that reflects reality and note why in a comment.

## Labels

`status:implementing` and `status:implemented` may not exist on GitHub yet.
Create them on first use:

```sh
gh label create "status:implementing" --color FBCA04 --description "実装中 — work in progress" 2>/dev/null || true
gh label create "status:implemented"  --color 0E8A16 --description "実装済み — pushed, awaiting merge" 2>/dev/null || true
```

## Who sets what

- `/tdd` advances the marker as it works (see the tdd skill).
- `/search-issue` reads markers to decide what to surface, and can set
  `status:implementing` when handing an issue off to implementation.
- Closing is done by whoever merges the PR (`gh issue close`, or GitHub's
  "Closes #N" auto-close on merge).
