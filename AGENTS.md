# Agent Instructions

## Agent skills

Matt Pocock engineering skills are vendored under `.agents/skills/` and `.claude/skills/`, pinned in `skills-lock.json` (a fixed upstream commit). They are **not auto-updated**. Run `/setup-matt-pocock-skills` again only if you need to switch issue trackers or restart configuration from scratch.

Repo-specific tuning of a vendored skill lives in that skill's `km_arrange.md` (referenced from the top of its `SKILL.md`); on conflict, `km_arrange.md` wins. To review and pull upstream changes safely, run `/update-matt-pocock-skills` — it diffs upstream at the pinned commit, risk-scans every change, preserves `km_arrange.md`, and applies only what you approve. Locally-authored skills (`inherit-prompt`, `make-pr`, `search-issue`, `update-matt-pocock-skills`) are not from Matt Pocock and are not tracked in `skills-lock.json`. See ADR-0005.

`graphify` is a third-party skill from a different upstream (`Graphify-Labs/graphify`, PyPI `graphifyy`). It is vendored with the upstream installer, not by hand, and is not tracked in `skills-lock.json` — see ADR-0009 for the update path and the risk review.

### Issue tracker

GitHub Issues on `KenichiroMatsubara/HayateProjects` (via `gh` CLI). See `docs/agents/issue-tracker.md`.

### Triage labels

Canonical triage roles mapped to GitHub labels (`ready-for-agent`, `question`, `help wanted`, etc.). See `docs/agents/triage-labels.md`.

### Progress markers

Implementation progress is tracked with `status:*` labels (`none` / `implementing` / `implemented`) plus the issue's closed state (`closed` = merged). `/search-issue` picks the next issue to tackle; `/tdd` advances the marker as it works. See `docs/agents/status-markers.md`.

### Domain docs

Multi-context monorepo — `CONTEXT-MAP.md` at the root points to per-package `CONTEXT.md` files and ADR directories. See `docs/agents/domain.md`.

## graphify

Knowledge-graph skill (`/graphify`). The repo ships only the skill files — the CLI is a per-developer prerequisite:

```bash
uv tool install graphifyy   # or: pipx install graphifyy
```

`graphify-out/` is build output and is git-ignored. Build it locally with `/graphify .`; the rules below apply only once `graphify-out/graph.json` exists.

- For codebase questions, first run `graphify query "<question>"`. Use `graphify path "<A>" "<B>"` for relationships and `graphify explain "<concept>"` for focused concepts. These return a scoped subgraph, usually much smaller than `GRAPH_REPORT.md` or raw grep output.
- Dirty `graphify-out/` files are expected after incremental updates and are not a reason to skip graphify. Only skip it if the task is about stale or incorrect graph output, or the user says not to use it.
- If `graphify-out/wiki/index.md` exists, use it for broad navigation instead of raw source browsing.
- Read `graphify-out/GRAPH_REPORT.md` only for broad architecture review, or when query/path/explain do not surface enough context.
- After modifying code, run `graphify update .` to keep the graph current (AST-only, no API cost).

Code extraction is local AST parsing and needs no API key. Semantic extraction runs only for docs, papers, and images, and calls the Gemini API **only if** `GEMINI_API_KEY` / `GOOGLE_API_KEY` is already set in the environment; otherwise the running agent does that work itself. See ADR-0009.
