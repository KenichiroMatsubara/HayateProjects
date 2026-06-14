# Agent Instructions

## Agent skills

Matt Pocock engineering skills are installed under `.agents/skills/` and `.claude/skills/`. Run `/setup-matt-pocock-skills` again only if you need to switch issue trackers or restart configuration from scratch.

### Issue tracker

GitHub Issues on `KenichiroMatsubara/HayateProjects` (via `gh` CLI). See `docs/agents/issue-tracker.md`.

### Triage labels

Canonical triage roles mapped to GitHub labels (`ready-for-agent`, `question`, `help wanted`, etc.). See `docs/agents/triage-labels.md`.

### Progress markers

Implementation progress is tracked with `status:*` labels (`none` / `implementing` / `implemented`) plus the issue's closed state (`closed` = merged). `/search-issue` picks the next issue to tackle; `/tdd` advances the marker as it works. See `docs/agents/status-markers.md`.

### Domain docs

Multi-context monorepo — `CONTEXT-MAP.md` at the root points to per-package `CONTEXT.md` files and ADR directories. See `docs/agents/domain.md`.
