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
- `Closes #n` only fires when a pull request merges into the default branch
  (`main`), and ticket PRs merge into `dev`. Close the issue by hand when its
  PR merges into `dev`, and link the merge commit in the closing comment.

### Release gate: `dev` → `main`

Agents never perform the `main` merge. The maintainer does, and only when they
decide to release, and only when every condition below holds on the tip of
`dev`:

1. **Green on `dev`**: `cargo fmt --all --check`,
   `cargo clippy --workspace --all-targets -- -D warnings`, and
   `cargo test --workspace` all pass.
2. **Scope closed**: every `ready-for-agent` issue this release claims is
   closed, and no `ready-for-human` issue is still waiting on this release.
3. **Reviewed**: every pull request merged into `dev` for this release was
   reviewed and approved; nothing entered `dev` by a direct push.
4. **Docs and skills match the code**: `docs/`, `skills/` and the tests describe
   the behaviour now on `dev` (the conventions at the top of this file).
5. **Verified running**: a release build started on a scratch data dir answered
   `/health`, `/console` and `/console/mcp` before the release is cut.
6. **Traceable**: the release is a merge commit `dev` → `main` — no squash and
   no history rewrite — so each ticket keeps its commits and its PR link.
7. **Release is not deployment**: merging to `main` ships nothing. Deploy the
   release build from `main` only when the maintainer asks for it.
