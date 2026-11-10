# Spindle conventions

- Treat `docs/spindle-architecture.md` and `docs/spindle-implementation-brief.md` as the source of truth for current architecture.
- Keep `docs/` focused on current material. Do not reintroduce a `docs/legacy/` or `docs/new/` directory.
- Keep `spindle-core` free of MCP transport concerns.
- Keep public tool/resource DTOs in `spindle-core`; MCP should parse, invoke
  services, and serialize responses.
- Keep root `skills/` as the source of truth for embedded skill assets.
- Keep SQLite migrations under `crates/spindle-adapters/migrations/`.
- Prefer explicit typed contracts over loose JSON blobs.
- Branch and revision workflows are public MCP surface area; keep docs, tests,
  and skills aligned when changing them.

## Agent skills

### Issue tracker

GitHub Issues in `urpey/spindle`, managed with the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

Default triage labels: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: `GLOSSARY.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.

## Branches and release

- `dev` is the integration branch. Feature and fix work branches from `dev` and
  merges back into `dev` through a pull request.
- `main` only moves at release time: the maintainer merges `dev` into `main`
  when they decide to release. Never merge a feature branch straight to `main`.
- Build a deployment from `main` at release time, not from a feature branch.
