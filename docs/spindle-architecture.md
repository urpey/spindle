# Spindle architecture quick start

This guide gives you the fastest accurate path into the current Spindle code
architecture. Read this first if you are new to the repository, then jump to
deep plan docs only when you need historical context.

## What Spindle is

Spindle is a local-first MCP server for story planning, drafting, revision,
and continuity validation. The architecture centers on one rule: writing
tools must return actionable validation output without leaking storage or
transport concerns across crate boundaries.

## Core framing

Spindle treats continuity checks as a truth-oracle workflow:

1. Capture typed world and story facts in persistent records.
2. Build scene-scoped context from branch-aware data.
3. Run validator passes against scene text and metadata.
4. Surface findings directly in write-tool responses and consistency scans.
5. Expand deterministic validator coverage as those interfaces land.

This framing replaces older "best effort linting" assumptions.

## Six architecture pillars

The current architecture is easiest to reason about as six pillars.

1. Typed data over ad hoc blobs.
2. Branch-aware reads and writes as default behavior.
3. Validator framework rollout across shared context and finding contracts.
4. Thin transport layer over service-level business logic.
5. Explicit invalidation for continuity and consistency-related caches.
6. Documentation and skill prompts that match shipped interfaces.

## Workspace map

The workspace has five active crates with clear ownership boundaries.

- `crates/spindle-core`: shared models and validator contracts.
- `crates/spindle-adapters`: repository + services + model routing + guidance.
- `crates/spindle-skills`: embedded `skills/` asset packaging.
- `crates/spindle-mcp`: MCP tool/resource wiring and process entrypoint.
- `crates/spindle-harness`: operator automation over MCP for batch drafting,
  checkpoints, and resume workflows.

## Hexagonal Shape

Spindle uses a pragmatic hexagonal split. The dependency direction should be:

```text
spindle-mcp / spindle-harness -> spindle-adapters -> spindle-core
spindle-adapters -> spindle-skills
```

`spindle-core` owns public DTOs, domain value types, context-bundle contracts,
subject snapshots, and validator contracts. It must not depend on MCP,
SQLite, model-agent runtimes, filesystem layout, or embedded asset plumbing.

`spindle-adapters` owns outbound and application-service concerns: SQLite
persistence, import/export persistence, model routing, embeddings, guidance
lookup, and orchestration that combines repositories with pure formatting or
validation helpers.

`spindle-mcp` is an inbound adapter. It should parse MCP arguments, apply
session/defaulting and mutation-serialization policy, invoke service methods,
convert errors into MCP results, and expose resource/tool schemas. It should
not own story business rules, persistence query details, or public story DTOs.

`spindle-harness` is another inbound adapter, but as an operator/client over
the public MCP surface. It should not reach into repositories or SQLite.

`spindle-skills` is static embedded asset packaging only. Versioned skill
manifests and nested references are embedded at build time;
`docs/skill-manifests.md` describes dependency and profile-alignment guards.

## Boundary Audit

Current audit findings:

- No reverse dependency from `spindle-core` to adapters, MCP, SQLite, or
  runtime model code was found.
- `spindle-mcp` previously owned several public tool/envelope DTOs
  (`set_active_project`, `init_grok_skills`, `get_writer_state`, and
  `get_scene_context`). These contracts now live in `spindle-core`.
- `spindle-mcp` previously assembled writer-state and scene-context envelopes,
  including markdown rendering and standards insertion. That presentation
  assembly now lives behind `SqliteSpindleService`; MCP only invokes the
  service and serializes the result.
- MCP still owns session defaulting, tool schema sanitization, lock scoping,
  and Grok skill-file installation. Those are transport/client-adapter
  concerns and should stay out of core unless they become reusable public
  contracts. Input and output tool schemas are scrubbed of schemars'
  non-standard numeric `format` annotations (`uint`, `int32`, `float`, …) at
  the generation seam so strict JSON-Schema clients do not warn per field.
  Finalized schema lint, structural validation and transport discovery-byte
  guards are described in `docs/tool-surface-guards.md`. Generated profiles
  omit explicitly selected large output schemas; full typed response schemas
  remain available through the v1 routes in `docs/tool-response-contracts.md`.

The service layer currently lives under:

- `crates/spindle-adapters/src/sqlite/service.rs`
- `crates/spindle-adapters/src/sqlite/project_resources.rs`
- `crates/spindle-adapters/src/sqlite/source_bridge.rs`
- `crates/spindle-adapters/src/format.rs`

`service.rs` retains the service type, shared state, and helpers. Cohesive
`impl SqliteSpindleService` blocks live under `sqlite/service/`: `context.rs`,
`consistency.rs`, `validators.rs`, `generation.rs`, `branches.rs`, `branch_snapshots.rs`,
`canon_mining.rs`, `canon_ratification.rs`, `plan_amendments.rs`, `replan.rs`,
`export.rs`, `reader_sim.rs`, `imports.rs`, and `import_hydration.rs`, alongside
entity, scene, research, and resource modules. Family tests live in each
family's `tests.rs` (audit tests are grouped under `consistency/`);
`service/tests.rs` holds shared fixtures. Supporting
modules keep cohesive adapter concerns out of that file: `project_resources.rs`
owns `read_project_resource` pagination envelopes and record-to-JSON resource
projection helpers; `source_bridge.rs` owns external source lookups; and
`format.rs` owns pure presentation formatting shared by service methods.

Architecture-sensitive boundaries are enforced by
`crates/spindle-core/tests/architecture_boundaries.rs`. The guard test checks
that:

- `spindle-core` does not import adapters, MCP, SQLite, or skill packaging.
- `spindle-skills` stays static asset packaging only.
- `spindle-harness` does not import SQLite service/repository internals.
- `spindle-mcp` does not define public `*Input`, `*Output`, or `*Envelope`
  DTO structs.

Run the focused guard with:

```bash
cargo test -p spindle-core --test architecture_boundaries
```

It is also covered by the normal workspace test command.

## Validator architecture (in progress)

Continuity behavior is centered in service-layer checks and consistency
tooling. Validator-oriented plumbing exists in adapter services, but do not
assume the full target validator DTO and output surface is available on every
write tool response yet.

SQLite Phase-4 validator cache rows live in `validator_finding`. Cache hits are
keyed by branch, scene, `validator_id`, `scene_text_hash`, and a validator
context hash derived from the relevant branch-scoped facts, rules, characters,
timeline events, temporal interventions, and style context. Service write paths
that alter canon, voice, style, or timeline state must resolve the matching
validator cache id, and read paths must use explicit branch-aware repository
queries. The continuity health resource
`bible://projects/{project_id}/continuity/health` exposes open findings,
resolved cache rows, branch lineage, orphaned temporal interventions, and
duplicate active canonical-fact keys for operational checks.

## Scene write path

The normal write path keeps generation, persistence, and validation separate
but connected:

1. Client calls scene-writing tools (`save_scene_draft`, `revise_scene`,
   `commit_scene_changes`).
2. Services persist branch-local scene updates through repository methods.
3. Services derive warnings, diffs, and consistency-related results.
4. Tool responses return scene IDs and currently shipped response fields from
   `crates/spindle-core/src/models.rs`.

`check_consistency` remains the broad audit surface for branch-scoped
consistency review.

Canon checks and writes for an inherited scene use the active descendant
branch, including child-only facts and overrides. Scene relationship deltas are
replaced on recommit; removing a pair from the continuity package reverses its
prior contribution. Branch-local relationship totals and their per-scene delta
ledger travel together through save-point snapshots and restores. Older snapshots
without a ledger restore with an empty ledger; their historical contributions
cannot be reconstructed. Canonical fact replacements retain their predecessor
links through save-point restore and merge; merging applies supersession only to
facts on the target branch. Voice profiles are keyed to their owning character,
so inherited characters' profiles are read-only until branch-local profile storage
is available.

`export_bible` and save-point snapshots are **full operator backups**, not
reader-facing artifacts: they include the project's tables (project- and
branch-scoped rows), so secret canonical-fact text (including the Character
Design hidden layer and unconscious desire) is present in the exported
`canonical_fact` rows. That is pre-existing backup behaviour and is why those
files are written under the data directory, never into reader exports; the
model-facing read tools still withhold the same facts. The payload now also
carries `character_design` (branch-scoped, restored with the branch) and
`story_spine` (project-level, export-only: a branch rewind never deletes the
accepted spine).

## Drafting packet foundation

`spindle-core/src/drafting.rs` owns `DraftingPacketV1`, the scene card, source
provenance and assembly report contracts. `spindle-adapters/src/drafting.rs`
selects whole records and renders exact messages with a model-verified tokenizer
interface. It rejects unknown token budgets and mandatory overflow. An internal opt-in HTTP drafting path now persists exact-byte artifacts and
defines a consumer-side accepted-text extractor trait for P2. It requires a
model-verified tokenizer and is not exposed through authoring runs; legacy
generation remains unchanged. See
`docs/drafting-and-revision-design.md` for the staged integration and release gates.

### McKee writer visibility and the filled packet

Writer-facing reads for a McKee project return only what the current Scene
needs. `get_chapter_briefing` filters `chapter_plan.scenes` to the requested
Scene and marks the narrowing additively with `chapter_plan.scene_scope`
(`mode: "current_scene_only"`, the resolved Scene, the planned count and the
exposed count; the field is skipped for non-McKee projects). When `scene_order`
is omitted the briefing resolves the writer cursor's next Scene in that chapter,
so a later-Scene briefing does not silently fall back to Scene 1.
`get_scene_context` and the briefing's bundled scene context expose
`novel.planned_scene` (the current plan), `novel.realized_scenes` (earlier
written Scenes of the chapter, as summaries for now) and
`novel.character_design_slice` (the tested contradiction, conscious desire and
planner-written behaviour cues). The slice type cannot represent the Character
Design hidden layer, the unconscious desire, their secret fact ids or
concealment notes, and every non-planning carrier still strips those secret
facts. Later Scenes' plans are omitted. Promise reads apply one redaction rule
(`spindle_core::promise_visibility`) to every writer-facing projection: the
scene-context/briefing due list, `get_writer_state`, subject snapshots
(`get_entity`, `get_character_snapshot`, continuity sheets) and the
`bible://projects/{id}/narrative-promises` resource. The planned payoff
placement, payoff Turning Beat, target Act and payoff-encoding notes never
appear; promises not yet planted at the cursor are dropped from due lists; the
promise's nature stays visible.

`prepare_scene` fills the packet's scene card from the current planned Scene
instead of legacy placeholders: primary Value Axis, opening and closing Charge,
opposition, Gap, the Turning Beat and structured Beats as required beats.
Additive `gap` and `turning_beat` fields carry the typed plan.
`chapter.planned_scene_count` / `is_closer` are chapter-position metadata and
`emotion.entry` keeps the POV's realised state, while `emotion.exit` honestly
says `unchanged (no planned emotion shift)` (a McKee plan has no emotion field;
the planned end state is the closing Charge in `value_shift.after`). Only due
**Key Facts** (recognised by the reserved `key_fact.<kind>:` predicate
namespace; a Scene is due when the placement's `scene_order` equals the Scene's
or is a chapter-level `None`) become `canon` hard constraints with their value,
with the unique fact id as the constraint id. A due **secret** Key Fact goes
through the same circle-of-trust resolver as scene context: it ships only inside
the `[SECRETS IN PLAY]` envelope when a circle member is present, and is
withheld with an explicit `due_key_facts` unavailable-pass entry when nobody who
knows it is present. An ordinary planned fact is never a packet constraint. A
promise being planted or reinforced appears by nature, never by payoff location.
A Scene with no planned value plan produces an unavailable packet entry with a
reason, not placeholder text. Both the default (read-tools) path and the
critique path see the same data. Unmarked projects keep their previous briefing,
scene context and packet serialization; see
[planning engine](planning-engine-design.md) for the field rules.

## Editorial controller, host tasks, and budget ledger

`spindle_core::drafting::editorial` defines the opt-in editorial controller
contract: ordered stages, bounded hybrid host tasks, exact admission
reservations (per scene and per run), and prose-free attribution events linked
to `model_call` ids. `spindle-adapters` persists the controller as a locked,
atomically replaced JSON artifact and exposes hybrid submission/judging plus
agent-parity draft and bounded-revision service methods. A missing automated
judge keeps the incumbent and returns `pending_manual_judgment`; host token
usage that is unknown keeps the configured maximum charged. `spindle-harness`
persists resolved opt-in settings and reports controller status, remaining
caps, pending host tasks, and the next action/reservation in `status`/`resume`
dry runs. Legacy runs and `EditorialPolicy::Legacy` remain byte-compatible and
are still the default; see `docs/drafting-and-revision-design.md`.

## Chapter pass, line edit, and book advisory

The opt-in editorial controller also carries an optional `line_edit` pass
(diction/syntax/dialogue polish on the cleared `line_edit` route, still gated by
deterministic checks and the swapped-order judge), a bounded chapter advisory
critique over deterministic scene windows with explicit coverage, and
author-selected chapter edits that reuse the scene acceptance gate with bounded
chapter context in both judge orders. Repetition memory builds only from
accepted prose (never rejected candidates), checkpoint reader/dual-persona notes
adapt into critic findings, and the book advisory queue reports summary gaps,
pacing drops, repeated devices, and unresolved promises without rewriting
anything. Both optional model passes default off; see
`docs/drafting-and-revision-design.md`.

## The authoring-run editorial loop

The authoring supervisor (`spindle-harness`, driven through the `authoring_*`
MCP tools) runs a bounded per-scene editorial loop, one `authoring_execute_next`
call per step. The full spine is **draft → verify → revise → commit → mine →
annotate beats → chapter summary → replan → checkpoint**. The state machine
lives in `spindle-harness/src/plan.rs` (`NextAction` / `determine_next_action`);
it only grows — existing phase semantics
(`pending → draft_saved → changes_committed → beats_annotated`) are unchanged and
pre-upgrade runs resume unaffected. The current run flow, tool-by-tool, is
documented in `docs/authoring-supervisor.md`; this section states the shipped
shape.

The loop's default posture is **hybrid**: for General/Teen/Mature scenes
`authoring_execute_next` returns a host-draft instruction and the active
assistant drafts in-chat and saves via `authoring_save_scene_draft` (with its
required continuity package); `mode: "agent"` opts into full route offload.
Every automated pass has a host fallback, so a project that configures no routes
behaves as pure hybrid.

Four of the loop's steps beyond the classic draft/commit/annotate/summary spine
are **opt-in per run**, each defaulting to off so a run that opts into nothing is
byte-identical to the pre-evolution flow:

| Run knob (`authoring_start_run`) | Default | Enabled behavior |
| --- | --- | --- |
| `mining_policy` | `disabled` | `propose_all` inserts a `mine canon` step between commit and annotate; each committed scene is mined into staged canon deltas (§ staging below). |
| `max_revise_attempts` | `0` | `1` or `2` inserts a deterministic `verify scene` step after each draft; a finding ≥ `warning` sends the scene back to the same draft route (or the host, in hybrid) as a bounded `revise` before commit. |
| `checkpoint_policy` | `manual` | `auto_advisory` / `auto_strict` let the harness self-clear a checkpoint in-process on a severity threshold instead of blocking on `await_checkpoint_review`. |
| `replan_policy` | `disabled` | `propose_all` inserts a `replan future plans` step after each chapter summary; staged plan amendments chase the outline to realized reality (§ staging below). |

Verify is deterministic: it runs the scene-scoped subset
`spindle_core::models::SCENE_VERIFY_CHECKS` (knowledge_timing, chronology,
temporal_coherence Tier 1, quantity/currency/affordability, tone,
content-boundary, secret_leak, canonical-fact consistency, the four McKee
realized-report checks below, and the prose-drift / world-rule / voice / style
Phase-4 validators — no model calls; deep/model tiers stay checkpoint-scoped for
cost). The loop converges or stops: the same finding
set is never revised twice (an unchanged re-verify parks the scene), and parked
findings inherit to the checkpoint. Every pass records an **honest** per-scene /
per-chapter outcome in `authoring_status` (`mine_status`, `verify_status`,
`replan_status`, `auto_outcome`, …): a skip or error never reads as a clean pass,
and mining/verify/replan never block the run.

The auto checkpoint policies run the deep `check_consistency`, record the audit,
run sampled dual-persona reviews via the `review` route, and (under an auto
policy only) a cumulative reader simulation, then approve iff the deep-consistency
severity clears the policy threshold (`auto_advisory`: nothing ≥ `warning`;
`auto_strict`: zero findings). A blocked auto-checkpoint stays `pending_review`,
so the manual 4-step escape hatch still clears it. Preconditions are enforced at
`authoring_prepare_run`: an auto policy requires the `review` route resolve
rating-cleared for every rating in the run's range, and `mining_policy` requires
the mine-or-review ladder cover every planned rating — an offload gap fails at
prepare, not mid-run.

## The rating-clearance chokepoint (explicit-content offload)

Every model pass that carries scene prose is dispatched through **one** gate,
`resolve_cleared_route` in `crates/spindle-adapters/src/ai.rs`, which wraps route
resolution and additionally verifies the resolved agent's declared `ratings`
cover the scene's content rating (normalized compare) before any prose leaves the
process. No call site resolves a prose-bearing route directly. The prose-bearing
set is the `PROSE_BEARING_ROUTES` constant — `{draft, mine, extract, line_edit,
reader_sim, review}`. When a route cannot serve a rating the pass **skips
honestly** (a `pass_skipped` journal event + an honest status/finding), falls back
to the host, or degrades per the pass's ladder — it is never silently downgraded
to an uncleared model, and Spindle never rewrites explicit prose to make it
routable.

Two documented refinements the code carries beyond the naïve "gate by route
name":

- **Split appendix.** The explicit gate is uniform across all prose-bearing
  routes, but the *system-prompt appendix* is not: `draft` receives
  `EXPLICIT_DRAFT_SYSTEM_APPENDIX` (which instructs on-page adult prose), while
  every other prose-bearing route (miner, auditor, reviewer) receives
  `EXPLICIT_ANALYSIS_SYSTEM_APPENDIX` instead — the invariant is uniform *gating*,
  not a uniform prompt (instructing a miner to write adult prose would be wrong).
- **Two exemptions.** The manuscript-import routes (`IMPORT_ROUTES` =
  `{import_extract, import_synthesize}`) are deliberately **not** in
  `PROSE_BEARING_ROUTES`: content ratings do not exist until analysis runs, and
  import is a direct operator action on the operator's own manuscript, so the
  guard is an advisory nudge (`import_chair_clearance_advisories`), not gating.
  The **replan** differ is non-prose-bearing (summaries + metadata only), so no
  rating clearance applies to it either. The two pre-existing model-backed deep
  tiers that predate the chokepoint (intra-scene `deep_temporal_coherence_issues`,
  semantic `deep_world_rule_compliance_issues`) now stamp each scene's rating on
  their `review` dispatch and honest-skip on `RatingNotCovered`, closing the
  former `rating: None` bypass.

The normative specification is `docs/spindle-evolution-design.md` §4; a recording
fake-router contract test pins that no request carrying an explicit scene's prose
ever reaches an uncleared agent.

## Independent canon extraction (shadow contracts)

`spindle-core::canon` defines evidence-bound, versioned claims, explicit coverage
for ten predicate families, and source-preserving comparisons with legacy drafter
packages. `spindle-adapters::canon_extraction` supplies an independent `extract`
invocation and shared host acceptance gate. The invocation receives exact scene
bytes/hash, known entity display terms and preceding ratified evidence, with no
drafter package. Invalid quotes/UTF-8 offsets, hashes, resolved IDs, coverage and
payloads reject the report; missing/uncleared routes and incomplete work remain
explicit pending/failed outcomes. Unresolved labels remain unresolved proposals.

These are shadow Rust integration APIs with no default authoring cutover or new
MCP tool. Claims, agreement and confidence do not write canon. The initial engine
uses rating-cleared non-agentic HTTP routes, a 2,000-output-token cap, 60-second
foreground timeout and conservative input-byte guard. Token/cost/latency rollout
gates remain unmeasured. Provider model, configured agent identity and existing
model-call attribution remain distinct. The accepted-text consumer interface is
owned by P1; `IndependentCanonExtractor` implements it without replacing its
exact-text/receipt/retry binding. Its local task cache and optional typed
`canon_audit` survive successful restart, reject changed-input task reuse, and
revalidate evidence on load. Compatibility write arrays stay empty. Host reports
can complete pending tasks through the same gate without replacing completed
reports. Existing callers without an audit retain their legacy serialization.
See ADR 0005 and `docs/canon-memory-design.md` for the remaining event-store and
ratification stages.

`spindle_harness::artifacts::LegacyPackageExtractor` implements the same port for
an existing drafter-supplied `GeneratedScenePackage`. It is bound to one frozen
request, exact accepted bytes/hash and receipt; retries with changed identity fail.
Its projection round-trips through the existing package converter without adding
claims or an independent audit. It makes no model call and applies no canon.
The legacy authoring default remains unchanged; independent extraction is opt-in.

`spindle_core::canon::events` and `spindle_core::canon::reducer` define the
append-only canon event schema (`CanonClaimRecordV1`, `CanonDecisionV1`,
`CanonEventV1`, `CanonSourceMapEntryV1`, `CanonSavepointCheckpointV1`) and pure
deterministic reducers (`state_at`, batch precondition/conservation validation,
parent-chain coverage/whole-record/delta-event masking, source invalidation,
checkpoint rollback, and 3-way branch merge). Migration V0051 and
`sqlite::repository::canon_events` / `sqlite::service::canon_events` persist the
append-only store and rebuildable projections (`canon_projection_state`,
`canon_projection_checkpoint`) without cutting over legacy state tables yet.

## Canon policy and per-project cutover

`spindle_core::canon::policy` is the pure, versioned v1 ratification policy. It
verifies every claim's UTF-8 evidence spans against the exact accepted revision
hash/bytes, rejects ambiguous entities and error-level contradictions against
ratified canon, and only auto-ratifies direct present narration with package and
independent-extractor agreement plus held-out calibration (minimum sample, ≥99%
point precision and a reported Wilson lower bound). Deaths/resurrections, secret
disclosures, promise payoffs/abandonments, invariant retcons, new world laws,
temporal interventions and ambiguous claims never auto-ratify in v1; they stage
as provisional for an author decision. Explicit operator declarations may
ratify ordinary claims without package agreement but remain subject to the same
safety route. Each evaluation records an audited `CanonDecisionV1` with policy
version, confidence inputs, precondition summary and idempotency key.

`sqlite::repository::canon_policy` persists policy decisions, events and claim
status atomically; a supplied `previous_decision_id` is compare-and-appended and
a stale tip fails without writing. Revocation appends `Withdraw` events for the
bad batch and its dependents, invalidates affected claim rows, and returns the
scene/chapter summary keys that later memory layers must recompute. Original
events and audit decisions are never deleted. Pending provisional work is
reported explicitly, and a checkpoint cannot claim continuity-clean while
eligible claims lack a final decision.

Migration V0054 adds per-project `canon_cutover_state`. `legacy` is authoritative
by default; forward transitions to `dual_write`, `shadow_read` and `canon_read`
require the compatibility parity/quarantine readiness gate, while rollback is
always allowed. In cutover modes the existing authoritative write paths
(`commit_scene_changes`, `commit_character_state`, `register_canonical_fact`,
`record_knowledge`, `update_relationship*`, `update_promise_status`) append
audited canon decisions in the same transaction and mark their exact legacy
snapshots in `canon_source_map`, so a backfill rerun is idempotent. Reads route
through canon projections in `canon_read`, compare but retain legacy reads in
`shadow_read`, and default to legacy. Legacy-unverified rows participate in
compatibility reads and lexical prefilters but remain warning/info-only until an
evidence-verified claim or operator ratification corroborates them.

## Shared claim-vs-canon validation

`spindle_core::canon::claim_validation` is the shared validator contract for
claims checked against ratified facts, knowledge edges, world rules and secret
circles. The lexical prefilter remains the first inexpensive retrieval hint and
is never treated as contradiction proof. Deterministic lanes then compare
normalized claims: unkeyed attribute prose drift (subject + attribute head +
explicit value antonym, with negation/irrealis/dialogue controls), paraphrased
knowledge references before the recorded learned position (distinctive-token
overlap + communication/recording verb + negation/time controls), and secret
values in narration attributed to the nearest named out-of-circle actor. A
`SemanticClaimAdjudicator` port covers prose rules that cannot be reduced to a
typed predicate; when no route is configured or the route fails, the report
carries an explicit `Degraded`/`Incomplete` status instead of reporting a clean
scan. Findings carry check type, severity, evidence span/quote, entity ids, lane
and a stable dedup key.

The deterministic lanes are wired into `check_consistency`'s
`canonical_fact_prose_drift`, `knowledge_timing` and `secret_leak` checks and
into the shared retcon scanner used by verify/commit. World-rule scanning
semantics v4 recognizes common inflections of plain verb patterns and promotes
an explicit cost-omission ("invoked ... with zero battery charge", "unbloodied
palms") to a likely violation. The original lexical-only findings and their
check types remain; no model-required lane is reported as zero findings.

## Layered derived memory and retrieval

`spindle_core::memory` defines the four derived memory layers (scene → chapter →
arc → book), coverage ranges, freshness/staleness, secret/reader visibility,
dependency edges, durable recompute jobs, budgeted context metadata, ordered
thread retrieval and direct-state entity retrieval. Summaries are derived
memory only; they never establish canon. Cursor filtering happens before layer
selection: a chapter/book/arc summary spanning the cursor is omitted and its
eligible children are used instead. Hidden secret facts and non-reader-visible
summaries are filtered before selection.

Migration V0055 adds `memory_summary`, `memory_summary_dependency` and
`memory_recompute_job`. `rebuild_layered_memory` deterministically derives scene
summaries from final prose + revision hashes, rolls chapters up from
`chapter_summary` (or child scenes), groups chapters under an explicit
`arc:unassigned:<book>` group until P3 supplies arcs, and rolls books up from
`book_digest`/chapters. Saving a scene marks its scene summary and every
transitive ancestor stale through the dependency DAG and queues deduplicated
lazy recompute jobs; stale rows stay visible as stale until a rebuild clears
them. `story_so_far` prefers fresh layered chapter summaries before the cursor
and falls back to the legacy recency path when memory has not been built.

`retrieve_open_threads_layered` reserves capacity per thread class, orders
overdue → due soon → blocking contradiction → scene/cast relevance → age →
stable id, labels planned deadlines separately, and reports overflow and the
next page offset. `retrieve_entity_state_layered` assembles state from the
existing stable-id snapshot (location/possessions/injuries/knowledge/neighbors)
with an explicit `direct_state` provider, so entity state is available without
an embedding provider.

## Staging & ratification (canon deltas + plan amendments)

While a drafter's structured continuity package (`character_states`,
`canonical_facts`, `relationship_updates`) is committed directly during
`commit_scene_changes`, post-commit mined canon deltas and post-chapter plan
amendments never reach the bible directly — they land in a staging table and
are applied only through an explicit operator decision. Two staging classes
share one lifecycle:

- **Canon deltas** (`canon_delta`, migration V0024; ADR 0001). Each committed
  scene can be mined (`mine_scene_canon`, or the run's `mine canon` step) into
  proposed, per-class, evidence-quoted deltas across fourteen classes. Each class
  maps to **exactly one existing write tool** on apply — mining opens no new write
  path. The operator lists the ratify queue (`list_canon_deltas`) and decides a
  batch (`decide_canon_deltas`: apply/reject, optional edit-before-apply, all-or-
  nothing pre-flight, apply dispatched per class to `register_canonical_fact` /
  `update_promise_status` / `update_relationship` / `commit_character_state` /
  `record_knowledge` / `annotate_scene_beats` / `commit_quantity_state` /
  `update_entity` / the `create_*` tools). Re-mining supersedes a scene's prior
  `staged` rows; decided rows are history.
- **Plan amendments** (`plan_amendment`, migration V0029; ADR 0003). After each
  chapter summary the replan differ compares realized reality against the
  not-yet-drafted chapters' plans and stages amendment proposals across eight
  classes, applied through the existing plan write paths
  (`plan_chapter` / `create_narrative_promise`). The lifecycle mirrors
  `canon_delta` exactly (`list_plan_amendments` / `decide_plan_amendments`); an
  **immutability guard** rejects any amendment whose target chapter already has a
  drafted scene at apply time (the outline chases the story, never the reverse),
  and applying snapshots the prior plan into `prior_state` and bumps
  `chapter_plan.plan_revision` so the outline keeps recoverable history.

Both staging classes are strictly **propose-only** — nothing auto-applies.
Authoring runs accept only `mining_policy = "stage_only" | "off"` (any
`auto_accept` value is rejected at input validation), and plan amendments
likewise have **no** auto-accept policy. Both class vocabularies are **one-way
doors** fixed by their ADRs (0001, 0003) — renames/reshapes strand staged rows
and break decided-audit replay. The `canon-steward` skill teaches both
ratification workflows.

## Circle-of-trust secrets

A canonical fact can be declared **secret** (`register_canonical_fact { secrecy:
{ holder_ids, concealment_note? } }`, migration V0023): `canonical_fact.secret`
plus one `knowledge_fact` row per holder linked back via
`knowledge_fact.secret_of_fact_id`. The circle of trust is thereafter **derived,
never duplicated** — `circle(fact)` is the set of characters with a linked
knowledge row — and is cursor-aware via each row's `learned_at`. Full design and
non-goals: `docs/secret-knowledge-gating-design.md`.

Three enforcement surfaces:

- **Context gate.** A single `SecretVisibility` resolver (`format.rs`, called by
  the assemblers in `service.rs`) enforces one rule per scene: if no circle member
  is present the fact is **withheld** from every context carrier (hard
  constraints, subject snapshots, knowledge briefing, semantic recall, digest
  open-threads); if a circle member is present it renders in a non-truncatable
  `[SECRETS IN PLAY]` hard-constraint block naming who knows, who is present and
  unaware, and (POV-only variant) that narration may carry private awareness while
  dialogue and others' behavior must not. A reveal is `record_knowledge` (or a
  `knowledge_learned` save-package entry / mined delta) with `secret_of_fact_id`
  set; it expands the circle from that placement forward and never leaks backward.
- **`secret_leak` audit** — the audience-direction complement to
  `knowledge_timing`. A deterministic dialogue-attribution tier always runs; a
  model-backed behavioral tier (`deep_secret_behavioral_leak_issues`) runs under
  `deep_check`, rating-gated through the chokepoint with honest-skip.
- **Reader-visibility rule for reader artifacts** (below) is deliberately stricter
  than the character-facing context gate.

## Search embedding index generations

Migration V0056 replaces the single hard-coded `FLOAT[64]` vec0 table with
provider/dimension-aware generations. A generation identity includes provider,
model, dimension, metric, normalization and version; each generation owns a
validated dynamic vec0 table (`vec_embedding_gen_<hash>`), while
`search_embedding` stores `generation_id`, `dimension` and a variable-length
`dimension * 4` BLOB. Existing token-hash-v1 rows are preserved as generation
`token-hash-v1:64`; no vector is padded, truncated or reinterpreted.

Every stored or queried vector is checked for emptiness, exact dimension,
non-finite values and f32 overflow before it reaches vec0. Queries load the
project's active generation and only search that generation's table, so a query
from a different provider/model/dimension is rejected rather than compared;
`search_bible`, semantic scene recall and fallback entity resolution all use
`embed_query_for_active_generation`.

Rebuilds are resumable and non-destructive: the target generation and its table
are created first, already-written rows are skipped on restart, and the active
generation keeps serving until completeness and an optional rank-based retrieval
quality probe pass. Generation build/active/retired/failed lifecycle is tracked per project in
`embedding_generation_project`, so the same shared spec/table can be migrated by
different projects independently. Activation atomically updates the project's
`embedding_index_state` and retires the previous generation with a rollback
window; rollback replays the previous pointer while its window is open. Active
document writes and source deletes/clears bump a per-project `write_epoch` in
the same transaction; activation requires the epoch to be unchanged and a
rebuild retries catch-up when it moves, so an edit, delete or new entity cannot
be lost between the source snapshot and the pointer switch. Source rows and the
epoch are read in one snapshot transaction. A provider
failure or failed quality gate marks the target failed and leaves the serving
index untouched. Per-project active state lets different projects migrate
independently. The legacy token-hash fallback remains available when no HTTP
embedding route is configured; an explicitly configured HTTP generation that
fails degrades to honest unavailable status rather than fabricating a vector.

## Run journal, SSE & operator console

Each authoring run appends an **append-only event journal** (`authoring_run_event`,
migration V0027; ADR 0002): one row per observable transition, written *after* the
state change commits (a journal-write error logs at `warn` and never fails the run
step). Payloads carry **ids, artifact paths, counts, and enums only — never prose,
fact text, evidence, or model output** — so the stream is safe to leave open on a
shared screen. The kind vocabulary and payload shapes are a **one-way door** fixed
by ADR 0002; consumers must ignore unknown kinds/keys. `authoring_status` (the run
tables) remains the source of truth; the journal is the timeline view. Host
`scene_drafted` rows may include an additive `anti_slop` summary (ids +
counts). The console timeline renders it when present. Chapter-scoped
shelf quotas roll across earlier scenes in the same chapter. Learned
false-positive suppressions (V0031 style-edit keep + V0045 store) apply
on later scans. Experimental structural observations stay off unless
`[anti_slop] experimental_structural` is set.

The journal streams over the existing HTTP surface at
`GET /events?topic=run:<authoring_run_id>` (SSE `id` = `seq`, `event` = kind,
`data` = payload JSON), replayed from `Last-Event-ID`+1 then followed live;
`/events` with no topic is the unchanged model-routes snapshot.

The **operator console** is served at `/console` when HTTP mode is on
(`SPINDLE_HTTP_ADDR`): a single embedded HTML page with no build step or external
assets. It reads run status, the compiled manuscript, and staged canon/plan
queues through localhost-only `GET /console/api/*` service reads, and follows a
run's journal live over the SSE topic above. Series, reader memory, editorial
decisions, and local release actions use the console's own MCP session at
`/console/mcp`, which exposes a fixed console tool set (`CONSOLE_TOOLS`)
independent of `SPINDLE_TOOL_PROFILE`, so a narrow model profile cannot break the
console; ratification still goes through the guarded `decide_*` tools. The GET
endpoints never mutate. Cross-site browser requests are rejected. When an Origin
header is present, it must match the loopback authority or an exact origin listed
in `SPINDLE_HTTP_ALLOWED_ORIGINS`; native MCP clients without one remain supported.

Console copy is bilingual: Traditional Chinese (`zh-TW`) is the fixed default and
English is an opt-in switch whose choice is remembered per browser. All
operator-facing copy lives in one per-language catalog embedded in the page, and a
key missing from the active language falls back to English rather than printing the
key. Each feature carries a help notice (`!`) that opens a short tutorial in the
active language. The catalog and the tutorials belong to the same embedded file,
so the console still needs no build step and makes no external requests.

Dynamic copy reads the same catalog: values are interpolated with `{placeholder}`
rather than concatenated, and the English data enums the server returns (item
status, promise state) are shown through catalog labels that fall back to the raw
value, so an unknown state is displayed rather than hidden. Switching language
re-renders the active pane, so already-rendered rows follow the switch.

All eight panes render from that catalog, as do the shared feedback line and the
generic load errors; no pane keeps hard-coded copy. The Settings pane reads
`GET /console/api/settings`, which returns the loaded config path and revision,
each agent's API-key variable name with a set/unset flag (never a key value), the
resolved routes, the features each route powers, and the editable config document;
routes on the built-in local stand-in are marked "no model connected". The editor
covers the whole document: agents (identity, provider, endpoint, model, key
variable, ratings, quality tier, context limit, capabilities, notes), per-route
rule details (fallback agent, purpose, max tokens, temperature, rating scope, stop
strings) for routes bound to an agent, and the global switches (health checks and
timeout, import-to-explicit routing, `anti_slop` lists and the experimental
structural switch).

`POST /console/api/settings` saves that document: the whole configuration is
validated with the startup parser (plus field-level errors for the editor), a
stale `expected_revision` is refused with 409, the previous file is copied to
`config.toml.bak` next to it, and the running server reloads the file — so a new
agent or a reassigned route takes effect on the next call with no restart. The
write is operator-only and stays on the console HTTP surface: no MCP tool exists
for it, so model transports cannot change agent configuration.

`POST /console/api/settings/test` probes one agent through the same route
machinery a real call uses: it reports latency, the answering adapter and model,
a short excerpt, or a readable reason. An unset API-key variable is reported
before any provider call is made, and the whole probe is bounded to twenty
seconds. Each feature's `!` tutorial is
attached by delegation, so rows, groups and checkpoints rendered later open their
own tutorial too.

## Craft evaluation layer

Two deep, checkpoint-scoped craft passes ride the same deep-check / honest-skip
pattern as `promise_payoff_detection`:

- **`scene_purpose_fulfillment`** (`check_consistency` deep tier) asks whether a
  drafted scene accomplishes its chapter-plan `purpose` field; a `fulfilled:false`
  verdict is one advisory `info` finding (purpose drift is often the story
  improving, not breaking), rating-gated per scene with honest-skip.
- **Cumulative reader simulation** runs under an auto checkpoint policy: a persona
  derived from the reader contract reads the checkpoint range's chapters in order
  with rolling memory (`reader-sim-notes.json` per run) and reports per-chapter
  engagement. It is **enrichment, not a gate** — concerns never fold into the
  approve/block verdict — and is fully rating-gated via the `reader_sim` route
  (falling back to `review`) with honest-skip.

**Style learning from operator edits** (migration V0031, opt-in per project via
`project.style_learning`) captures the before/after pair when an operator re-saves
an agent-drafted scene with changed prose as a *style-edit candidate* feeding the
existing `preview_refresh_style_profile` → `refresh_style_profile` review flow —
no new tools, nothing enters a profile without an explicit refresh, and
explicit-rated candidates are withheld unless the `style_analyze` route is
explicit-cleared. Provenance across the domain is modeled by the
`spindle_core::provenance::Provenance` vocabulary (scene / chapter / book / file /
asserted-by-author / imported / derived), which records where a value came from.

## Canonical fact model in practice

Canonical facts are active architecture work. Treat the canonical-fact
validator flow as transitional rather than final.

- Any schema/tool change that affects facts must update docs and tests in the
  same change.

## Reader-facing artifacts (spoiler-bounded)

`export_recap` and `export_series_bible` (evolution §3.8) are pure read models
over existing branch data — no model calls. Both assemble reader-facing Markdown
bounded at a story-index cursor (end of `through_chapter` / `through`, whole
book/project when absent) and both take an opt-in `write_to_workspace` that
reuses the same workspace `artifacts/` write as `compile_manuscript`
(canonicalize + `starts_with` confinement).

A single resolver (`reader_withheld_secret_values` in
`crates/spindle-adapters/src/sqlite/service.rs`) enforces the reader-secret rule
for both tools, and is deliberately stricter than the scene-context circle gate:
a `secret` canonical fact is only surfaced once a linked `knowledge_fact` reveal
row is `reader_visible` with a dated `learned_at` at or before the cursor. The
standing circle-of-trust holder row a secret declaration writes (learned_at
NULL) is dramatic irony, not a reader reveal, so it does not lift the veil.
Operator-facing lookup/recap flows live in the `bible-librarian` skill.

The read-only operator console v1 (evolution §3.7) is served at `/console` when
HTTP mode is on (`SPINDLE_HTTP_ADDR`): a single embedded HTML page (no build
step, no external requests) that reads run status, the compiled manuscript, and
the staged canon-delta / plan-amendment ratify queues through localhost-only
`GET /console/api/*` service reads, and follows a run's event journal live over
the existing `/events?topic=run:<id>` SSE. It never mutates (I5); ratification
stays in the `decide_*` tools. Actions and the thread board are v2.

## Quantity & economy continuity

Money, named prices, and progression systems (LitRPG/cultivation) are tracked as
typed canon, not free-text lore. Economies and their *price facts* (numeric
canonical facts with a unit) surface in scene context; a per-project **quantity
scheme** (`project_quantity_scheme`, migration V0020) plus append-only,
position-stamped **quantity state** (`quantity_state`) track wealth/progression as
ordered *bands* (band-primary; amount optional). Enforcement rides the existing
rails — the `quantity_drift`, `currency_consistency`, and `affordability` arms of
`check_consistency`, a `[WEALTH/STATE]` scene-context hard constraint, a WarnOnly
band-jump advisory on `commit_quantity_state`, and branch-merge carrying. Full
design and roadmap: `docs/continuity-quantity-design.md`.

## Temporal coherence (in-scene)

In-world *timing* is guarded on two axes. The **between-scene** axis is the
`chronology` arm of `check_consistency` (a later scene rewinding the story
clock). The **within-scene** axis is the `temporal_coherence` arm: a pure,
deterministic prose scan (`spindle_core::temporal::scan_temporal_coherence`,
infrastructure-free domain logic) that reads one scene's drafted prose and flags
an unsignaled time-of-day jump (*teleporting time*), prose that contradicts its
own established time of day (*drifting time*), or a scene whose clock declares a
multi-day `duration_days` but whose prose renders the span as one unbroken block
(*unrendered declared span*). It is prose-only and runs without a calendar; its
time lexicon covers parts of day, meal names, canonical hours, and meridian clock
times. High-precision/low-recall (conservative band-jump thresholds, word-boundary
matching, greeting/recollection guards, suppression on `temporal_mode` and coarse
`precision`), so findings are advisory `warning`s.

The same deterministic scan (via the shared `scan_temporal_findings` adapter
helper, mapping each hit to a `ConsistencyIssue`) surfaces at **three enforcement
points**, all advisory: the immediate `temporal_findings` field on
`save_scene_draft` / `revise_scene`; a `temporal_findings` field on
`commit_scene_changes` that is held separate from `blocking_continuity_findings`
and emitted at `warning` severity, so it can never block a commit under any
`continuity_gate` (including `BlockErrors`); and the branch-wide
`check_consistency` arm.

That deterministic scan is **Tier 1**. `check_consistency` with `deep_check:
true` adds **Tier 2** — `deep_temporal_coherence_issues`, a model-router-driven
semantic pass (one `review`-route call per scene) that recovers the idiomatic /
implied time jumps the fixed lexicon cannot, mirroring the existing
`deep_world_rule_compliance_issues`. It parses structured findings (tolerant of
code fences) and degrades to no extra findings when no review model is
configured, so Tier 1 always stands alone. Both tiers emit the same
`temporal_coherence` check_type at `warning` severity. The prevention half is the `[IN-WORLD TIME]` hard
constraint built by `SqliteSpindleService::temporal_anchor_constraint` and
surfaced in both `get_scene_context` and `get_chapter_briefing`: it feeds the
previous scene's end clock **and location** forward as the expected start, so the
drafting model anchors *where* and *when* a scene takes place — the temporal twin
of the spatial "White Room" grounding rule. A scene's location is persisted via
`scene.location_id` (migration V0021) from the `location_id` on
`SaveSceneDraftInput`. The deferred-NL-parsing scope note that motivated this
layer is in `docs/continuity-timing-design.md`.

## Voice drift and retcon checks

Voice-drift (`voice_drift`) and retcon/reachability (`retcon_reachability`) are
shipped Phase-4 validators (`crates/spindle-adapters/src/sqlite/validators.rs`),
cached per `scene_text_hash` + context hash like the other Phase-4 validators and
run by default in `check_consistency`. `voice_drift` is also in the scene-scoped
`SCENE_VERIFY_CHECKS` subset the in-run verify step uses. Retcon checks also run
the shared character-state scanner on save/revise, commit, scene verify and broad
consistency scans, resolving inherited scenes against the active branch. A named
action after a prior-scene deceased state yields one scene-attributed
`retcon_reachability` issue; mentions and explicitly marked memories, flashbacks,
recordings, dreams and hypothetical actions are excluded by a conservative lexical
prefilter scoped to the named action (including ordinary conditional clauses).
An object such as a recording-room door does not suppress an action. It is not
a semantic claim extractor. Same-scene final death states
are excluded because within-scene ordering is not yet available. Free-text death
statuses remain warnings; `status:deceased` remains an error. This state scan is
uncached so state changes cannot leave a stale text-only hit. The `continuity-editor`
skill documents both alongside the rest of the check catalog.

## Migration ledger

SQLite migrations are the source of truth for the schema
(`crates/spindle-adapters/migrations/`, applied in order; every column added by
the evolution work is nullable or defaulted so pre-upgrade databases and runs
resume unchanged). The continuity and editorial-loop migrations:

| Migration | Adds |
| --- | --- |
| V0020 | `project_quantity_scheme` + append-only `quantity_state` (money/progression bands) |
| V0021 | `scene.location_id` (spatial + temporal grounding) |
| V0022 | narrative-convergence audit fields (plot-line connected ids, conflict `escalation_demonstrated`, pacing-curve `intensity_points`) |
| V0023 | circle-of-trust secrets (`canonical_fact.secret` / `concealment_note`, `knowledge_fact.secret_of_fact_id`) |
| V0024 | `canon_delta` staging queue (ADR 0001) |
| V0025 | authoring-run `mining_policy` + per-scene `mine_status`/`mine_detail` |
| V0026 | authoring-run `max_revise_attempts` + per-scene `verify_status`/`revise_attempts` |
| V0027 | `authoring_run_event` append-only journal (ADR 0002) |
| V0028 | authoring-run `checkpoint_policy` + per-checkpoint auto outcome |
| V0029 | `plan_amendment` staging queue + `chapter_plan.plan_revision` (ADR 0003) |
| V0030 | authoring-run `replan_policy` + per-chapter `replan_status`/`replan_detail` |
| V0031 | style-learning-from-edits (`project.style_learning` + edit candidates) |
| V0041 | `model_call` telemetry ledger (`get_model_usage`) |
| V0047 | `model_call` attribution columns (`scene_id`, `run_id`, `route`, `prompt_template_id`, `prompt_version`, `context_packet_hash`, `temperature`, `top_p`, `max_tokens`) |
| V0048 | branch-scoped canon uniqueness, per-scene relationship delta ledger, and fact replacement links (renumbered from unshipped V0046 so V0047 databases upgrade in order) |
| V0049 | initial-planning sessions/proposals and operation receipt foundation (ADR 0004) |
| V0050 | planning apply attempts, operation audit/output fields and deferred semantic-index refresh queue |
| V0051 | append-only canon event store (`canon_claim`, `canon_decision`, `canon_event`, `canon_source_map`, rebuildable `canon_projection_state`, `canon_projection_checkpoint`) |
| V0052 | planning route-generation attempt ledger (`planning_generation_attempt`) with prompt/schema hashes, usage receipts, JSON-pointer findings and host-fallback envelopes |
| V0056 | provider/dimension-aware embedding index generations: variable-length `search_embedding`, generation registry, per-project active/previous state, legacy 64-dimension preservation |
| V0057 | McKee project marker (`project.mckee_structured`, set only by planning `CreateProject`) and project-level `story_spine` (one series spine, optional per-book sub-spines) |
| V0058 | McKee Main Plot / Subplot structure on `plot_line`: nullable `value_axis`, closed-set `subplot_job` (contradict / resonate / set_up / complicate) and the `main_plot` flag (additive: unmarked rows keep NULL / 0, but a McKee project's pre-existing plot lines must be completed through `update_entity` before the next Series-Arc/Book acceptance) |
| V0059 | McKee Character Design: one `character_design` row per character (2–5 contradictions, public / personal / private layers, conscious desire, pressure on the protagonist, behaviour cues) with `hidden_self_fact_id` / `unconscious_desire_fact_id` pointing at secret canonical facts rather than duplicating those texts. Additive: existing projects keep no rows |
| V0060 | McKee Narrative Promise payoff structure: `narrative_promise.planned_payoff_turning_beat` and `target_act` (both additive, nullable) |
| V0061 | McKee realized report on `scene.realized_report`: nullable JSON holding the realized Value Turns, Character Effects, realized promise events and dramatised planned Key Facts. Additive: existing rows keep NULL and every read skips an unset report |

## Where to make changes

Use this map when you need to patch behavior quickly.

- Add or modify tool DTOs: `crates/spindle-core/src/models.rs`.
- Add or modify Story Spine contracts: `crates/spindle-core/src/story_spine.rs`
  (records) and `crates/spindle-core/src/charge.rs` (typed Charge).
- Add or modify Character Design contracts: `crates/spindle-core/src/character_design.rs`
  (persisted record, full planning view, redacted non-planning view and the
  secret predicate namespace).
- Add or modify Main Plot / Subplot vocabulary: `crates/spindle-core/src/plot_line.rs`
  (closed `SubplotJob` and Value Axis comparison).
- Add or modify continuity and validator-facing DTOs:
  `crates/spindle-core/src/models.rs`.
- Add or modify MCP session/defaulting/schema behavior:
  `crates/spindle-mcp/src/tools/session.rs`, `serialization.rs`, or `schema.rs`.
- Change repository persistence/query behavior:
  `crates/spindle-adapters/src/sqlite/repository/` by aggregate (`projects.rs`,
  `scenes.rs`, `characters.rs`, `canon.rs`, `relationships.rs`, `branches.rs`,
  `plans.rs`, `authoring.rs`, `style.rs`, `imports.rs`, or `search.rs`). Shared
  parameter types, transaction primitives, and snapshot helpers remain in
  `crates/spindle-adapters/src/sqlite/repository.rs`; aggregate tests live in
  each family's `tests.rs`, with shared fixtures in `repository/tests.rs`.
- Change business logic or tool orchestration:
  `crates/spindle-adapters/src/sqlite/service/` (choose the corresponding
  family module; for example `context.rs`, `consistency.rs`,
  `canon_ratification.rs`, `plan_amendments.rs`, `export.rs`, or
  `import_hydration.rs`). Shared service state and helpers remain in
  `crates/spindle-adapters/src/sqlite/service.rs`.
- Change MCP tool/resource exposure:
  `crates/spindle-mcp/src/tools/catalog.rs` (registration), `profiles.rs`
  (profile membership), and `dispatch.rs` (service delegates). Authoring
  handlers live in `tools/authoring*.rs`; skills and journal helpers live in
  `tools/skills.rs` and `tools/journal.rs`. Router/shared state remains in
  `tools.rs`, and resources remain in `crates/spindle-mcp/src/resources.rs`.
  Family tests live under `tools/`; full pre-split `tools/list` snapshots
  for all profiles live in `tools/snapshots/`.

## Contributor guardrails

When you change architecture-sensitive code, keep these guardrails.

1. Do not move transport logic into `spindle-core`.
2. Do not bypass service-layer invariants from MCP handlers.
3. Put public tool/resource DTOs in `spindle-core`, not in `spindle-mcp`.
4. Keep validator findings structured and cache-aware.
5. Update docs and skill prompts when public behavior changes.
6. Run `cargo test -p spindle-core --test architecture_boundaries` after
   crate-boundary or DTO-placement changes.
7. Prefer branch-aware queries and avoid hidden project-wide scans unless
   explicitly required.

## First-hour onboarding checklist

Run this sequence when you start a new implementation task.

1. Read `README.md` for runtime and test commands.
2. Read this file end to end.
3. Locate affected DTOs in `spindle-core` and service paths in
   `spindle-adapters`.
4. Confirm any tool-surface impact in `spindle-mcp`.
5. Run targeted tests before and after edits.

## Related docs

- `docs/spindle-implementation-brief.md`: broader implementation snapshot.
- `docs/authoring-supervisor.md`: the current authoring-run flow, tool-by-tool
  (policies, staging hand-offs, journal, reader-sim).
- `docs/spindle-evolution-design.md`: the historical design record and rationale
  for the editorial loop (P0–P6, all landed) — this architecture doc supersedes
  it for current behavior.
- `docs/secret-knowledge-gating-design.md`: the circle-of-trust design (NOW +
  NEXT-5/6 landed; NEXT-7 reveal-aware briefing and LATER items still open).
- `docs/adr/0001-canon-delta-classes.md`, `0002-authoring-run-event-journal.md`,
  `0003-plan-amendment-classes.md`: the one-way-door contracts (delta/amendment
  class vocabularies, journal kinds/payloads) referenced above.
- `docs/continuity-quantity-design.md`, `docs/continuity-timing-design.md`: the
  quantity and temporal continuity spine the editorial loop builds on.
- `docs/fiction-anti-slop.md`: fiction anti-slop contract (shelf catalog,
  product locks, writing-packet digest, Phase 3 critic / revise loop).
  Scanner + rewrite-from-beats contracts: `spindle-core` `style/antislop/`.

## Initial planning foundation (Phases A–C)

Core `planning` contracts define closed operations with typed existing/proposal
references, `PlanningArtifactV1` envelopes, structured schedules (`promise_events`,
`pov_plan`, `intensity_targets`, `acts`), RFC 6901 JSON-pointer validation findings, and
session/call budget receipts. V0049 adds pre-project sessions, proposals and the
operation ledger; V0050 adds apply attempt history, operation audit/output fields
and a deferred search-index queue; V0052 adds `planning_generation_attempt` for
durable route-generation attempts and `retry_key` idempotency.
`sqlite/service/planning/apply.rs` translates the allowlist through existing
service methods, and `sqlite/service/planning/generate.rs` drives bounded
generation across `plan` (`contract`, `cast_world`, `series_arc`) and `outline`
(`book`, `chapter`, `scene`) routes using versioned prompt IDs
(`planning_contract_v1`, `planning_seed_v1`, `planning_series_arc_v1`,
`outline_book_v1`, `outline_chapters_v1`, `outline_manifest_v1`,
`planning_repair_v1`). McKee sessions resolve the parallel `_v2` family
(`planning_contract_v2`, `planning_seed_v2`, `planning_series_arc_v2`,
`outline_book_v2`, `outline_chapters_v2`, `outline_manifest_v2`,
`planning_repair_v2`); the v1 family stays for existing and non-McKee sessions
byte-identically, and the resolved ID plus prompt hash are recorded on the
generation receipt. The v2 prompt is the same hard-rules block plus a headed,
host-neutral McKee guidance block per level; the artifact schema is unchanged.
Built-in local stubs and `adapter_kind == "local"` are
rejected for creative planning; a real `review` agent is used only on explicit
`allow_review_backend` opt-in. Call, token and optional external USD ceilings are
checked before dispatch, with at most one repair attempt on validation failure.
Missing routes, stubs, rejected outputs and exhausted budgets return honest
outcomes (`needs_route`, `model_output_rejected`, `budget_exhausted`) with a
`PlanningHostRequest` fallback; host and labeled `fixture` submissions run
through the same staging validator and all output lands only as staged proposals.
The four MCP planning entry tools (`planning_begin`, `planning_advance`,
`planning_status`, `planning_decide`) expose this pipeline. Application success
is never drafting readiness.

Human acceptance preflights the complete selected dependency closure in a
rollback-only application transaction, then commits one domain operation and
its durable receipt at a time. Initial project/branch attachment shares its
creation receipt's commit; root aliases preserve the initial book/chapter IDs.
Interrupted retries reuse immutable receipts. Failed batches report their
committed prefix, failed operation and not-reached suffix. Rejection and
supersession cannot hide partially applied history. Snapshots protect reader
contract, refs, target hashes and revisions. Verified approved writes advance
later snapshots only when they match the transaction-local pre-write state;
unrelated external edits remain stale. Acceptance rechecks active branch,
visible IDs and persisted-prose guards. An active authoring run freezes roots
and requires amendments for existing plans (including inherited or cached run plans), while missing future chapters may
still be filled. Project-wide contract/allocation guards also respect runs on
other branches. Active-run projection refresh remains a later integration step.

`sqlite/pool/application.rs` pins reads to the writer inside this bounded async
unit of work. Planning-used repository transactions nest through savepoints:
`Repository::with_transaction` reads the pool's application-transaction signal,
opens a real transaction outside it and a SQLite **savepoint** inside it, so a
service such as `register_canonical_fact` can be reused by Key Fact apply
without a nested `BEGIN`. The outer application transaction owns the atomic
boundary; a nested failure rolls back only the savepoint, and the surrounding
operation still fails. Other writes wait on the application gate; cancellation
rolls back before that gate is released. Semantic embedding work runs after
commit, from a durable queue, and pending refresh counts remain visible if
indexing fails.

### McKee projects and the Story Spine

Projects created through the planning (`new-book`) root carry
`project.mckee_structured = 1`; every other project keeps 0, omits the field
from its payloads and cannot be switched. McKee behaviour is gated on this
marker; the contract prompt advertises `create_story_spine` only for a marked
attached project, and staging rejects a spine anywhere else. The contract level
stages a typed `CreateStorySpine` operation (`spindle_core::story_spine`);
accepted spines live in the project-level `story_spine` table (shared by all
branches, because the spine is series intent, not branch canon) and read back
through `planning_status` and the `story_spine` subject table in `get_entity`
(any profile that exposes `get_entity` can read one). `spindle_core::charge`
defines the closed **Charge** type (sign plus four-step depth) used by later
McKee structure work. See [GLOSSARY](../GLOSSARY.md),
[ADR 0006](adr/0006-acts-and-sequences-are-outline-intent.md) and
[ADR 0007](adr/0007-mckee-structure-swain-prose.md).

Every planning session starts with human gates at every level (ADR 0004). The
**McKee gate profile** (`policy_id = "mckee-default"`, `policy_version = 1`;
human at Contract, CastWorld, SeriesArc and Book, `delegate_if_valid` at Chapter
and Scene) is an **explicit author decision**: the author activates it with
`planning_advance(action = "set_policy")` (or `planning_begin`'s policy
override) and a human `policy_actor` plus note, after the contract/cast gate and
before the chapter/scene batches. Activation requires a McKee context — an
unattached new-book session or a session attached to a project carrying the
marker — and is rejected for an attached non-McKee project. A policy-actor
acceptance is audited as `policy:mckee-default:1` and requires the proposal's
policy snapshot to match the live policy; roots are refused for a policy actor
at apply time, and every explicit override rejects root-level
`delegate_if_valid` before staging. A delegated level that carries a stored or
revalidated blocking finding, an unresolved author question, a stale policy
snapshot or projection, an unaccepted upstream root or an over-cap advisory
count is refused for the policy actor and stays staged; one shared refusal
helper drives both the apply checks and `planning_status`'s existing
`next_action`, so its human-review reason cannot drift. Canon-delta and
plan-amendment ratification are untouched: both still reject a `policy:` actor
and remain human decisions (ADR 0001 / ADR 0003 / ADR 0004). Planning prompt
versions are pinned per session at creation (`prompt_version`: `2` for a McKee
context, `1` for an attached non-McKee project, absent legacy rows resolve
`_v1`), so prompt selection never flips on attach or advance; the resolved ID
and hash live in the existing generation receipt and model-call attribution
uses the matching `v1`/`v2` label. There is no new table, migration or tool: the
profile lives in the existing `planning_session.policy` JSON, and the pinned
prompt version is an additive field on the session JSON.

Plot lines carry the same one-way vocabulary through `spindle_core::plot_line`:
a Value Axis, a closed Subplot structural job, and the Main Plot flag
(`plot_line.value_axis`, `subplot_job`, `main_plot`; V0058). The create tool
(`create_plot_line`), the planning `CreatePlotLine` operation and the existing
`update_entity` allowlist all accept them; `get_entity(table="plot_line")`
reads them back through `PlotLineDetails`. The supported `update_entity` fields
for `entity_type: "plot_line"` are exactly `value_axis` (string or null),
`subplot_job` (`contradict` / `resonate` / `set_up` / `complicate`, or null)
and `is_main_plot` (boolean). `is_main_plot` is the only public spelling; it is
mapped internally to the `main_plot` column, and a caller passing the raw
column name is rejected with an error naming `is_main_plot`.

The closed job set and the boolean flag are validated on every write regardless
of the McKee marker — that is marker-independent data integrity. The Main Plot /
Subplot blocking gates below are McKee-only. For McKee projects, Series-Arc and
Book acceptance validate the resulting plot-line set: the non-archived rows
visible from the session branch (the branch plus its ancestor lineage, with a
child branch's row overriding a same-named parent row), plus the artifact's
plot-line operations and those of its staged ancestor proposals (an applied
parent is already in the row set and is not counted twice). Exactly one Main
Plot, its Value Axis equal to the accepted series Spine Axis (trim then exact
match), and exactly one structural job on every Subplot are required. Those
blocking findings are recorded on the staged proposal (so the author sees them)
and refuse acceptance; the acceptance error names a **descriptive locator** for
the repair site. Locators are not always pointers into the staged request: the
grammar is `/operations/{i}/operation/input/{field}` and `/schedule/...` for the
current artifact, `/existing_plot_lines/{record_id}/{field}` for an accepted
plot-line row, `/accepted_scenes/...` and `/staged_scenes/...` for planned
Scenes, `/merged_sequences/{sequence_key}/{field}` for an inherited Sequence,
`/accepted_acts/{order}/cause/...` for an accepted Act and
`/staged_acts/{order}/cause/...` for a staged ancestor's Act. `related_refs`
carries the record ids / output refs where the path alone does not identify the
row; it is empty when the path is enough. The `sequence_key` segment is
pointer-safe (non-empty, at most 128 bytes, only ASCII alphanumerics, `-`, `_`
and `:`), so it never contains `/` or `~`. A McKee project with no accepted
series spine yet skips the axis comparison. V0058 is
additive, so existing projects and unmarked rows keep their previous read and
export shape and keep NULL / 0. That is not an exemption: a McKee project's
pre-existing plot lines must be completed through `update_entity` (`value_axis`,
`subplot_job`, `is_main_plot`) before the next Series-Arc/Book acceptance, or
the gate blocks.

A Cast & World proposal on a McKee project may also carry a typed
`CreateCharacterDesign` operation (`spindle_core::character_design`): 2–5
contradictions, the four layers of self, conscious and unconscious desire, the
pressure the character puts on the protagonist, and planner-written behaviour
cues. Acceptance writes one `character_design` row per character (unique by
`character_id`, V0059). "Major character" is interpreted structurally: every
character the artifact introduces through `create_character` must have exactly
one design in that same artifact. Missing, fewer than 2 or more than 5
contradictions, and duplicate designs are blocking findings recorded on the
proposal (`character_design_*`) that refuse acceptance; characters added
outside planning are not retro-gated. Non-McKee projects are exempt, and
staging a design for one is rejected.

The Act list lives on the planning schedule (`PlanningSchedule.acts`) as
outline intent, never as a story record (ADR 0006). Each entry carries an
`order`, a climax scene placeholder, the Spine Axis Charge after its climax,
the `central_question_part` it answers, and `answers_central_question` for the
final Act. There is no new table or migration: accepted proposals keep their
schedule, and `planning_status` reads it back on the proposal summary. For
McKee projects, Series-Arc and Book acceptance merge applied Acts visible from
the session branch's lineage (farthest ancestor first, session branch last),
staged-ancestor Acts and the current artifact's Acts by order (later decisions
override) and record blocking findings for an unchanged adjacent Charge, an
unmarked final Act, a non-final Act marked `answers_central_question`, climax
chapters that do not rise with `order` inside one book, non-contiguous or
duplicate orders, empty question parts and missing/positive climax
coordinates; a non-increasing depth progression is advisory only. Acts are
Series-Arc/Book metadata: carrying a non-empty `schedule.acts` on any other
level is blocking (`act_schedule_wrong_level`) and such lists are ignored by
the accepted-Act merge. Chapter and Scene artifacts that plan a chapter
containing an Act climax must include a scene at the reserved `scene_order`
(`act_climax_scene_missing`), using the same evaluated Act set so a batch that
accepts the Act parent and the chapter plan together is already checked.
`act_breakpoints` are derived from the Act list for McKee projects (authored
maps are refused) and stay authored for unmarked projects.

Sequences are chapter-level schedule metadata (`PlanningSchedule.sequences`),
not story records (ADR 0006). Each Sequence has a stable `sequence_key`, exactly
one plot line reference, a `question`, an inciting Scene and a climax Scene that
must be one of its `member_scenes` **and the last member in story order**;
members are exact planned-Scene placements in strict story order and may cross
chapters and books. The key is a **global** identity for the merged set, not
scoped to one plot line: a later Chapter proposal reusing it **refines** that
Sequence, overriding the scalar fields and unioning `member_scenes` (deduped,
strict story order) so earlier members survive, but it may not change
`plot_line_id` (`sequence_plot_line_changed`). A key is non-empty, at most 128
bytes, and restricted to ASCII alphanumerics, `-`, `_` and `:` so it is safe in
a locator (`sequence_key_invalid` otherwise). Chapter/Scene acceptance records
blocking findings for a missing, duplicate or unsafe key, a refinement that
moves the plot line, a missing plot line or question, empty or unordered
members, a non-positive placement, a missing inciting Scene, a climax that is
not a member or not the last member, and an inciting Scene after the climax. For
a McKee
project the artifact's own Sequence list is checked at every level; a non-empty
`schedule.sequences` anywhere other than Chapter is `sequence_wrong_level` and
is never merged (unmarked projects keep ignoring Sequence metadata).

Planned scenes also carry the value planning the chain is checked against: a
primary Value Axis plus up to two secondary axes (each with an opening and
closing Charge), `opposition`, `gap`, `scene_question`, `inaction_cost`, `risk`
(1–5) and an optional `dilemma` naming the tested character contradiction. All
fields are additive and serde-defaulted, so existing chapter plans serialize
exactly as before; there is no new table or migration because chapter plans are
JSON on `chapter_plan` and Sequences live in the proposal schedule. For McKee
projects the Chapter/Scene value-chain pass builds the ordered planned-Scene
list (accepted chapter plans visible from the branch lineage, staged-ancestor
plans, then the current artifact; a later plan for one chapter replaces the
earlier one) and records stage-recordable blocking findings for a non-event, a
broken chain on any axis, a Scene in no Sequence, a missing Inaction Cost, a
missing dilemma at a Sequence or Act climax, an unknown character
contradiction, and invalid value-plan shapes. Advisory findings cover a
same-direction run of three or more Scenes on one axis, a risk drop inside a
Sequence, a chapter ending off a Value Turn (its last Scene is flat on the
primary axis, or closes at the Charge the chapter's first Scene opened with — a
net-zero chapter), and a Character Design contradiction no dilemma tests. The
findings use the existing structured shape and JSON-pointer paths:
`/operations/{i}/operation/input/scenes/{j}/...` for the current artifact,
`/accepted_scenes/{book}/{chapter}/{scene_order}/...` for accepted plans and
`/staged_scenes/...` for staged ancestors. Unmarked projects get no value-chain
findings and their payloads are unchanged.

Subplot structure is checked over the merged Sequences and Convergence Points in
the same Chapter/Scene pass (`spindle_core::subplot` owns the pure rules). A
Subplot Sequence climax placed **after every** Convergence Point of its Subplot
is blocking (`subplot_sequence_climax_after_convergence_point`); the finding
points at `/schedule/sequences/{i}/climax_scene` for a Sequence the artifact
declares or refines and at `/merged_sequences/{sequence_key}/climax_scene` for an
inherited one. Four advisories never block: a Subplot with no Convergence Point
(also reported over the merged plot-line set when a Series-Arc/Book artifact
authors the Subplot), a Subplot Value Axis that goes three consecutive scanned
Chapters without a turn (a Chapter gap, a turning Chapter or a book change
resets the run), a Subplot whose turns do not support its declared job
(`contradict` needs a negative-ending turn, `resonate` a positive-ending one;
`set_up` and `complicate` impose no direction), and a Subplot turning deeper than
the Main Plot inside one Act (Act N covers Scenes after Act N-1's climax up to
and including its own; the Main Plot axis is its `value_axis`, falling back to
the series Spine Axis). The depth comparison runs only when the merged Acts'
exact climaxes rise strictly in global story order, so a cross-book
out-of-order Act list cannot read one Act's turns as another's; a Subplot that
shares the Main Plot's Value Axis reads the same turns on both sides and cannot
false-positive. The job and depth heuristics and the dormancy/Act-range
definitions are documented in [planning engine](planning-engine-design.md).
Non-McKee projects get none of these findings.

An Act may name the Subplot Sequence climax that causes it:
`PlannedAct.cause` is an optional `{ plot_line_id, sequence_key }`, additive and
skipped when absent. The `sequence_key` is global, so a key collision is a data
error rather than an ambiguous lookup. Series-Arc/Book acceptance resolves every
merged Act with a cause against the merged plot lines and Sequences, and
Chapter/Scene staging re-validates the merged accepted Acts against refined
Sequences: the plot line must exist and be a Subplot, the Sequence must carry the
cited key and belong to that plot line, its climax must be at or before a
Convergence Point and at or before the Act's own climax. Semantic failures are
stage-recordable blocking findings (`act_cause_unresolved_subplot_sequence`,
`act_cause_not_a_subplot`, `act_cause_after_act_climax`,
`act_cause_after_convergence_point`); an unresolved `PlanningRef` itself is
already a hard staging error in the generic reference check. A cause authored
here is located at `/schedule/acts/{i}/cause/...`; a staged ancestor's at
`/staged_acts/{order}/cause/...`; an accepted Act's at
`/accepted_acts/{order}/cause/...`. A later Book or Series-Arc artifact can be
staged after the Chapter Sequences exist because the Sequences are read from
applied Chapter proposals, not from a parent; the forward pass cannot author the
cause, so the author re-stages that level after the Chapter Sequences are
accepted.

Planned Scenes also carry structured **Beats** (`spindle_core::beats`) for McKee
projects: each Beat has a closed `kind` (`action`, `reaction`, `turn`), an
optional `actor_character_id` (a character record id in the stored form, a
planning reference in the artifact), the `tactic`, what is `said`, what is
`wanted` (the subtext), a per-Beat `gap` (expected versus actual) and an
optional free-text `swain_label`. The key is spelled out rather than named
`actor` so planning's generic `*_id` reference resolution never treats an
unrelated operation field as a character. The Scene names its **Turning Beat**
with `turning_beat`, the 1-based position of that Beat in the ordered `beats`
list (the generated schema advertises `minimum: 1`), so a Scene numbers no Beat
of its own. The planning artifact carries the same fields with a planning
reference for `actor_character_id` (`spindle_core::planning::PlanningBeat`);
planning apply resolves it to a record id. The resolved form round-trips through
the direct `plan_chapter` write, the staged `plan_chapter` inputs exposed in
`planning_status` proposal payloads, the chapter briefing and plan-amendment
replay; the internal chapter-plan read (`Repository::get_chapter_plan`) serves
service paths and is not an MCP read surface. The legacy `beat_structure:
Vec<String>` is untouched: every project still deserializes it, but on a McKee
project structured Beats are authoritative and the strings are never a
substitute. Chapter/Scene acceptance records stage-recordable blocking
findings for a Scene with no Beats (`scene_beats_missing`), no Turning Beat
reference (`scene_missing_turning_beat`), a reference that is not the 1-based
position of an existing Beat (`scene_turning_beat_out_of_range`), a reference to
a Beat that is not of kind `turn` (`scene_turning_beat_not_turn`), and more than
one Turn Beat (`scene_multiple_turn_beats`). Swain categories remain optional
labels only (ADR 0007) and never gain validation or enum membership. There is no
migration: chapter plans are JSON on `chapter_plan`. Unmarked projects get no
Beat findings and their payloads are unchanged.

**Key Facts** are the turning canon decided during planning
(`PlanningOperation::CreateKeyFact`, `spindle_core::key_fact`). The operation is
in the closed allowlist at the Chapter and Scene levels only, and its `kind` is a
closed turning set: `death`,
`revealed_secret`, `relationship_turn`, `item_gained`, `item_lost`,
`power_gained`, `power_lost`. Any other fact type fails deserialization, so a
stat change, a pull result or an arbitrary note can never enter the Key Fact
path; those facts keep the existing mine-then-ratify canon flow unchanged. The
input names a subject from the existing canonical-fact subject model (or no
subject for a project-wide fact), a unique `key`, a `value`, optional `context`,
a story placement with a required `scene_order`, and an optional
`planned_scene_id` naming the planned chapter that owns the Scene. The fact is
**planned-and-pending**: the Scene it
names may still be unplanned, so a future placement accepts without a Scene row
and binding happens later. Acceptance applies the existing planned-and-pending
canonical-fact path (`scene_id: None`, placed by the Key Fact's book/chapter),
returns the canonical fact id, and leaves binding to the later
`bind_canonical_fact_to_scene` flow once the Scene row exists. For a McKee
project, Chapter/Scene acceptance records stage-recordable blocking findings
when the placement is not a positive book/chapter/scene, when an explicit
`planned_scene_id` cannot be resolved, or when the reference and the placement
name different Scenes. The **Key Fact marker** is the reserved canonical-fact
predicate namespace `key_fact.<kind>:<author key>` written by the apply path
(the same no-new-table pattern as the Character Design `character_design.*`
namespace). Writer-facing text strips the prefix via
`spindle_core::key_fact::canonical_fact_display_predicate`, and read paths
recognise a Key Fact by the prefix, so ordinary planned facts keep the
mine-then-ratify flow and are never treated as Key Facts. A `revealed_secret`
Key Fact needs a character subject
and is registered as a **secret** canonical fact (`secret = 1`) with that
character in the initial circle of trust (one `knowledge_fact` row linked by
`secret_of_fact_id`) and a concealment note explaining the planned reveal; the
circle expands later through `record_knowledge` at the dramatised reveal. Key
Facts are available only to projects created through the planning root; an
existing project gets a readable `key_fact_requires_mckee_project` staging
error. V0060 adds no Key Fact table: the fact is a `canonical_fact` row.

**Narrative promises** gain two additive fields on both the planning operation
and the direct `create_narrative_promise` input: `planned_payoff_turning_beat`
(the 1-based position of the payoff Beat in the planned payoff Scene, the same
numbering as `turning_beat`, requiring `planned_payoff`) and
`target_act` (the 1-based order of the Act the promise must pay off in). V0060
adds the two nullable columns to `narrative_promise`; row mapping, snapshots,
promise reads and the typed export emit them only when set, so unset rows and
existing projects stay byte-identical. For a McKee project a promise planned at
the Series-Arc or Book level is **long-range** and must name a `target_act` that
exists in the accepted/merged Act list (missing or unknown Act is a
stage-recordable blocking finding); a Chapter/Scene placement may omit it. The
pre-existing ordering rule is unchanged and still hard-blocking: a payoff
placement before its setup. A payoff Turning Beat that is not a Turn Beat or
falls outside the planned payoff Scene's Beat list is a stage-recordable
blocking finding instead: the proposal stages with the finding and only
acceptance is refused. The payoff Turning Beat may be omitted when the payoff
is not on the Turning Beat or the payoff Scene is outside the current horizon.
The direct `create_narrative_promise` write refuses a Beat reference with no
`planned_payoff` as a readable error.

The **deus-ex-machina guard** (McKee projects) ties the two together with
stage-recordable blocking findings. A `revealed_secret` Key Fact needs an
earlier-planted promise whose planned payoff lands in that Scene
(`key_fact_secret_reveal_without_promise`); a chapter-wide payoff placement does
not name a Scene and cannot satisfy it. A structured Beat gains
`information_dependent` (serde default `false`, never serialized when false) and
an optional `promise_id`; when the Scene's Turning Beat is information-dependent
it must name a promise planted at an earlier placement than its Scene
(`turning_beat_information_dependent_without_promise`,
`turning_beat_promise_not_planted_earlier`). Both guards read the merged promise
view — accepted `narrative_promise` rows visible from the branch lineage,
promises declared by staged ancestors and the current artifact, plus the planned
payoff/plant events of the merged promise schedule — with the same deterministic
precedence as the Act and Sequence gates. Non-McKee projects keep the additive
promise fields as plain optional data and their promises and plans serialize
exactly as before; they cannot use Key Facts at all (the staging error above).

On a McKee project a `scene_add` plan amendment is refused at pre-flight: the
class cannot add Sequence membership or a value plan, so it must restage the
Chapter planning artifact instead. `scene_replace` keeps the value-plan fields
untouched, while `scene_drop`/`scene_reorder`/`synopsis_update` remain available
and do not cascade into `schedule.sequences` (follow-up #34).

### McKee realized reports and deterministic scene verify

A written Scene reports what its prose actually did. The realized report
(`spindle_core::realized`, V0061 `scene.realized_report`) carries four additive,
serde-defaulted sections:

- `value_turns`: one realized **Value Turn** per planned Value Axis (primary and
  secondaries) with the realized opening and closing **Charge**, the realized
  **Gap** (expected versus actual) and the realized **Turning Beat** (a 1-based
  Beat position and/or a summary).
- `character_effects`: one **Character Effect** per present major character
  (`reinforce`, `reveal`, `contradict`, `change`, `none`).
- `promise_events`: the Narrative Promise events the Scene advanced
  (`setup`, `reinforcement`, `payoff`, `subversion`, `abandonment`).
- `key_fact_ids`: the planned **Key Facts** the Scene dramatised (canonical-fact
  ids).

The report rides all three continuity-carrying save entry points:
`commit_scene`'s `CommitSceneContinuityPackage.realized_report`,
`authoring_save_scene_draft`'s structured `realized_report` field (which also
counts as structured continuity content on a McKee project, so a McKee Scene that
only reports its turn still satisfies the mandatory package; it is inert on a
non-McKee project), and the raw `save_scene_draft` field used by the agent
drafting path and standalone saves. The raw path carries no continuity package
but does apply realized promise status. It is persisted on the Scene row
and read back additively: `get_entity(table="scene")` carries `realized_report`
when set, `get_scene_context`/`get_chapter_briefing` carry it on each earlier
Scene's `novel.realized_scenes` entry, and an unset report is skipped entirely —
so a Scene saved without a report, and every non-McKee project, serialize
byte-identically. A save replaces the stored report (the report describes the
text just saved); omitting it leaves the Scene with none.

Four deterministic checks join the fast scene-verify subset
(`SCENE_VERIFY_CHECKS`, no model calls) and run only for a McKee project when a
caller asks for them in `check_consistency` (the run's verify step does). They do
not run inside a bare `commit_scene`/`save_scene_draft` outside a run, so the
"mismatch cannot commit" guarantee is the run's bounded revise loop; parked
findings at commit are #33. All
four findings are `error`, use the existing consistency-finding shape and are
attributed to the Scene (`entity_ids` starts with the Scene id):

| `check_type` | Rule |
| --- | --- |
| `realized_value_turn` | A planned Value Axis with no realized turn, a realized opening/closing **Charge** that differs from the plan (exact comparison), a planned Gap the report never mentions, a realized Turning Beat position that differs from the plan, a report with no Value Turn at all for a Scene that has a planned one, or a realized axis the plan does not contain. |
| `realized_key_fact` | A planned Key Fact due in this Scene (the same due rule the drafting packet uses) that the report does not list in `key_fact_ids`. |
| `realized_promise_event` | A promise planned to be planted or paid off in this Scene (an exact Scene placement, not a chapter-wide one) that the report does not report as the matching event. |
| `character_effect` | `change` reported where no planned arc milestone of kind `change` is placed (a milestone pinned to the Scene, or placed chapter-wide in that chapter), or a planned Scene-level change milestone reported as anything other than `change` (including `reveal`, and including no effect at all). A chapter-level milestone is satisfied by `change` in any Scene of the chapter; the reportable mismatch is a Scene claiming only `reveal` while no Scene in the chapter reports `change`. |

Comparison tolerances are deliberate and documented on
`spindle_core::realized`: Charge comparison is exact (a closed typed pair);
Gap comparison is by presence, not wording (the Gap is prose the writer
legitimately paraphrases); the Turning Beat is position-compared only when the
report numbers one, and a summary-only reference is accepted. `get_entity` reads
a Key Fact by the reserved predicate namespace, so an ordinary planned fact is
never demanded.

**Promise status is system-written.** After a confirmed realized promise event
saves through any of the three entry points, the system itself advances that
promise's status through the existing promise status-history mechanism, with the
saved Scene as `source_scene_id`: `setup` → `planted`, `reinforcement` →
`reinforced`, `payoff` → `paid_off`, `subversion` → `subverted`, `abandonment` →
`abandoned`. The agent no longer calls `update_promise_status` for a McKee scene:
on a McKee project that tool (and an `update_entity` promise-status write) is
rejected with an error that points at the realized-events path, except for a
no-op status call. Validation and status application run in the same transaction
as the Scene write (the promise must exist, be on a branch the Scene inherits,
and not be planted after the Scene), so an unusable event rolls the Scene row
back with the status rows. A re-save of the same Scene with the same event is
idempotent; a re-save whose report no longer claims a previously reported event
appends a corrective status-history event that restores the pre-Scene status
(append-only: the original event stays as history). Non-McKee projects never
advance a status from a report, and their `update_promise_status` flow is
unchanged.

**Revise budget.** A McKee run must prepare with a non-zero
`max_revise_attempts` (`1` or `2`): `authoring_prepare_run` reports a missing
requirement and `ready_to_draft=false`, and `authoring_start_run` therefore
refuses to start. A zero/absent run budget means a Scene that realized a turn
different from its plan would have no bounded revise attempt before the
checkpoint. An absent budget means the run default of zero, so an omitted field
blocks a McKee run just as an explicit `0` does at both `authoring_prepare_run`
and `planning_readiness`: planning readiness passes no budget, so it now reports
the same missing requirement `authoring_start_run` would refuse on instead of
advertising a ready range. Non-McKee projects are unaffected.

The hidden layer and the unconscious desire are not plain columns: they are
registered as secret canonical facts (`secret = 1`, one `knowledge_fact` row
placing the character in the circle of trust, `subject_table = "character"`)
and the design row keeps only `hidden_self_fact_id` /
`unconscious_desire_fact_id`. The facts use the reserved
`character_design.hidden_self` / `character_design.unconscious_desire`
predicates. Planning reads are full reads: `planning_status.character_designs`
resolves those fact texts. Every non-planning read is redacted: subject
snapshots and scene context strip the design facts (they never get a
`[SECRETS IN PLAY]` envelope because the writer-facing rendering is the
behaviour cues), and `get_character_snapshot` exposes only the
`CharacterDesignRedactedView` (contradictions, public self, conscious desire,
pressure, behaviour cues; the personal and private selves are not part of the
writer-facing slice). Secret fact ids, values and concealment notes never appear
outside planning: `check_consistency`'s deterministic and deep secret-leak tiers
drop the author-only design facts through the same
`format::is_planning_only_secret_fact` helper as context assembly and subject
snapshots, so no finding echoes their text, predicate or fact id. Arc milestones
gain an optional `kind` (`change` / `reveal`); absent kinds and legacy milestone
JSON serialize exactly as before.

All gates remain human. Delegation activation, route generation, manifest/run
validation and agent entry arrive in subsequent phases. See
[ADR 0004](adr/0004-initial-planning-ratification.md); existing canon-delta and
plan-amendment classes and human ratification policies remain unchanged.
