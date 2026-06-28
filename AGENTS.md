# Agent Instructions

## Agent skills

Matt Pocock engineering skills are vendored under `.agents/skills/` and `.claude/skills/`, pinned in `skills-lock.json` (a fixed upstream commit). They are **not auto-updated**. Run `/setup-matt-pocock-skills` again only if you need to switch issue trackers or restart configuration from scratch.

Repo-specific tuning of a vendored skill lives in that skill's `OVERRIDES.md` (referenced from the top of its `SKILL.md`); on conflict, `OVERRIDES.md` wins. To review and pull upstream changes safely, run `/update-matt-pocock-skills` — it diffs upstream at the pinned commit, risk-scans every change, preserves `OVERRIDES.md`, and applies only what you approve. Locally-authored skills (`inherit-prompt`, `make-pr`, `search-issue`, `update-matt-pocock-skills`) are not from Matt Pocock and are not tracked in `skills-lock.json`. See ADR-0005.

### Issue tracker

GitHub Issues on `KenichiroMatsubara/HayateProjects` (via `gh` CLI). See `docs/agents/issue-tracker.md`.

### Triage labels

Canonical triage roles mapped to GitHub labels (`ready-for-agent`, `question`, `help wanted`, etc.). See `docs/agents/triage-labels.md`.

### Progress markers

Implementation progress is tracked with `status:*` labels (`none` / `implementing` / `implemented`) plus the issue's closed state (`closed` = merged). `/search-issue` picks the next issue to tackle; `/tdd` advances the marker as it works. See `docs/agents/status-markers.md`.

### Domain docs

Multi-context monorepo — `CONTEXT-MAP.md` at the root points to per-package `CONTEXT.md` files and ADR directories. See `docs/agents/domain.md`.
