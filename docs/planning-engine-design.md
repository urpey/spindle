# Planning engine: staged intent to draftable manifests

Status: **proposed design only** (Wave 2 / P3). No runtime behavior changes in
this PR. Baseline: `origin/main` at
`419f40be7378d8919ac89d28139496d06b7db935` (2026-11-08).
All source citations below are `file:line` on that commit, with pinned GitHub
links so the concurrent adapter split cannot move the evidence underneath this
review. Statements about the proposed engine are requirements, not claims that
it already exists. Estimates and release thresholds are proposed budgets.

## 1. Problem and code evidence

Spindle has the pieces to store a plan, but lacks a model-backed **initial
planning orchestrator**. This is distinct from the shipped model-backed
post-summary replan differ. The missing product behavior is: supply a premise,
review a few creative decisions, and receive a complete, persisted plan that
passes `authoring_prepare_run` without manually coordinating entity and outline
writes.

| Current behavior | Evidence on baseline main |
| --- | --- |
| The entry skill is explicitly “Spindle writing in Codex”; planning delegates to `plot-architect`, worldbuilding and character skills. This is guidance, not an initial planning service. | [skills/spindle/SKILL.md:6–29][entry]; [skills/plot-architect/SKILL.md:20–48][plot] |
| `plot-architect` describes series/current-arc/next-episode horizons and asks the caller to invoke individual tools. It already teaches reader-contract, plot, arc, promise and pacing concepts; preserve that craft knowledge. | [skills/plot-architect/SKILL.md:50–99][horizons] |
| A project requires `ReaderContract { promise, style_notes, boundaries }`; creation returns project, branch, book and chapter IDs. Do not create book 1 / chapter 1 twice. | [crates/spindle-core/src/models.rs:86–92][contract]; [crates/spindle-core/src/models.rs:1068–1089][project] |
| Book/chapter creation and prose-form outlines exist; `plan_chapter` persists structured plans and returns style warnings. None of these methods generates an initial hierarchy from a premise. | [crates/spindle-core/src/models.rs:2953–2985][structure]; [crates/spindle-adapters/src/sqlite/service.rs:1996–2019][plan-write]; [crates/spindle-adapters/src/sqlite/service.rs:2591–2665][outlines] |
| A scene plan can carry cast, location, rating, beats, purpose and research metadata; the fields are more permissive than drafting readiness. Prepare explicitly rejects missing/unknown cast and locations, missing ratings, empty lists and missing chapter plans. | [crates/spindle-core/src/models.rs:3838–3887][manifest]; [crates/spindle-mcp/src/tools.rs:3020–3135][prepare] |
| Plot lines have convergence placements; promises have setup and planned-payoff placements; character arcs have milestones; pacing config/curves and arc constraints exist. Reuse these rather than inventing a parallel story bible. | [crates/spindle-core/src/models.rs:3583–3595][plot-model]; [crates/spindle-core/src/models.rs:3675–3683][promise]; [crates/spindle-core/src/models.rs:3749–3830][arc-pacing] |
| `generate_alternatives` is a deterministic scene/branch fan-out, explicitly “No LLM”, using heuristic synthesis. It is not a planner or a quality baseline for generated outlines. | [crates/spindle-adapters/src/sqlite/service.rs:14285–14380][alternatives] |
| There is no built-in `plan` or `outline` route in `default_routes`. Route assignment is configurable, so this does not mean those names cannot be configured by a user; there is no initial-planner dispatch consuming them. | [crates/spindle-adapters/src/ai.rs:2739–2853][routes]; initial write paths above |
| Plan amendments already have eight fixed classes, provenance, decision history and prior state. Their differ considers future undrafted chapters after a summary; apply checks persisted scenes again. | [crates/spindle-core/src/models.rs:7381–7453][amendment]; [crates/spindle-adapters/src/sqlite/service.rs:19070–19155][differ]; [crates/spindle-adapters/src/sqlite/service.rs:20855–20965][guard] |
| Accepted amendments bump plan revision and reseed active runs. Reseeding skips summarized chapters and any scene that has advanced beyond pending or has a saved scene ID. This Wave 1 fix is a required integration seam. | [crates/spindle-adapters/src/sqlite/service.rs:21230–21275][reseed-call]; [crates/spindle-adapters/src/sqlite/repository.rs:17385–17565][reseed] |
| Ratification preflights the batch before writing, then applies in order with per-item failure/not-reached results. This is not a transaction spanning all existing services. | [crates/spindle-adapters/src/sqlite/service.rs:20557–20825][decide] |
| The prose judge already supports blinded randomized/swapped judging, provider calls, usage accounting and human calibration. Its prose rubric does not measure plan quality. | [evals/prose_judge.py:1–68][judge]; [evals/prose_judge.py:863–960][double-judge] |

### Scope and ownership

P3 adds initial planning, staged ratification, structural planning validation,
and a host-neutral new-book entry flow. It does not implement prose critique or
revision (P1), canon extraction/memory (P2), tool-profile consolidation (P4), or
the behavior-preserving adapter split (S). Implement against the service seams,
not the current giant-file layout. Public DTOs belong in core, orchestration
and persistence in adapters, thin handlers in MCP, root skill assets in
`skills/`; these match the documented boundaries
([docs/spindle-architecture.md:43–78][architecture]). No new core dependency on
SQLite, transport, model runtimes or skill packaging.

## 2. Pipeline and persistence mapping (proposal)

One planning session owns a bounded dependency graph. Generate breadth at the
series horizon and detail only for the selected book/range (default proposal:
three chapters). Never expand a serial's entire future into scene manifests.
Each arrow below means **stage, validate, decide, then advance**; speculative
children may be staged against a parent's exact proposal hash, but cannot apply
until that parent is accepted.

| Level and output | Inputs and creative work | Accepted write path |
| --- | --- | --- |
| Premise / series intent | Author premise, standalone/series/serial intent, genre, length horizon, audience/rating ceiling, POV/tense preferences, fixed ideas and forbidden outcomes. Preserve unknowns explicitly. | New planning-session intent record only; no generated canon yet. |
| Reader contract | Emotional/genre promise, style notes, content boundaries; surface contradictions with the premise. | `create_project` with `ReaderContract` for a new project; explicit accepted changes through `update_entity` for an existing project. The partial-merge contract path exists at [service.rs:2139–2185][contract-write]. |
| Cast & world seed | Minimal protagonist/opposition/support, wants and constraints, locations needed by the near horizon, relationships and hard world rules. No encyclopedia expansion. | Existing `create_character`, `create_location`, `create_relationship`, `create_world_rule`; on McKee projects every introduced character also needs a `create_character_design`. Optional factions/terms/etc. only when needed. Resolve returned IDs before dependent writes. See [service.rs:693–780][seed-writes], [service.rs:1196–1220][world-rule] and [plot-architect:50–66][horizons]. |
| Series / current-arc outline | Central question, arc exit condition, book-level destinations, plot-line convergence, themes/conflicts, promises and character milestones. | Existing `create_plot_line`, `create_conflict`, `create_theme`, `create_motif`, `create_narrative_promise`, `create_character_arc`, `create_pacing_config`, `create_pacing_curve`, `set_arc_pacing_constraints`. Store series-intent prose in the new planning-session artifact; project contract and story records remain authoritative. No assumed existing series-outline table. |
| Book / chapter plans | Ordered causal turns, chapter POV, target threads, expected intensity and scene budget. Future arcs stay coarse. | Reuse initial IDs returned by `create_project`; `create_book` / `create_chapter` for additional structure; `set_book_outline`, `set_chapter_outline`, then `plan_chapter`. A current arc is an outline section plus existing story records, not a new competing arc entity. |
| Scene manifests | Concrete purpose, action/change, cast, location, rating, beats, promise movements and research flags for each scene. | Translate resolved references into complete `PlanChapterInput.scenes`; **planning does not call `save_scene_draft`**. Planned metadata stays intent, not realized events. |
| Handoff | Read back persisted plans; preflight exact range and intended run policies. | `authoring_prepare_run`; only `ready_to_draft=true` enables a suggested `authoring_start_run`. Starting a prose run remains a separate author instruction. |

Do not mark a promise paid or an arc milestone reached because an outline says
it will happen. `CharacterArcMilestone` separates planned placement from
`reached_at` ([models.rs:998–1010][milestone]); preserve the same planned/actual
distinction for promises. Long promises may intentionally remain outside the
three-chapter horizon, with an explicit future destination or deferred reason.

### Dependency resolution

Model outputs use proposal-local references (`cast:protagonist`, `place:harbor`,
`promise:missing-heir`), never invented database IDs. A typed reference union
separates `ExistingRecordId` from `ProposalRef`. Apply creates parents first,
records each returned ID, and rewrites child inputs through an allowlisted
translator to existing DTOs. Existing IDs must resolve to the selected project
and visible branch. An unresolved reference blocks acceptance; it is never
silently replaced by the POV character, a random location, or a lower rating.

## 3. Staging, gates and recoverability (proposal)

Add a **separate** `planning_session` / `planning_proposal` persistence family,
reusing the lifecycle and decision semantics of canon deltas and amendments.
Do not shoehorn initial cast, contract or project creation into ADR 0003's eight
classes or give initial planning a fictitious `source_chapter`. Those existing
classes are a persisted one-way door ([models.rs:7381–7405][amendment]).

A planning session may exist before a project does: `project_id` and `branch_id`
are nullable only until its contract proposal creates the project. This is how
the first generated artifact is staged even though `create_project` requires a
contract. Accepting that root attaches the returned project/branch/book/chapter
IDs atomically to the session's creation receipt; subsequent proposals are
bound to that branch. For an existing project, bind at session creation and
require an explicit branch match on every decision. A provisional project is
not needed just to display a generated contract.

Each proposal has: stable ID, schema version, level and typed operation kind,
session/project/branch, parent proposal IDs and hashes, dependency refs, target
range, base record hashes/revisions, prompt/model receipt, intent hash,
validated payload, rationale, findings, status, created/updated timestamps,
decision actor/time/note and policy snapshot. Reuse `staged | applied | rejected
| superseded`; track generation/apply attempts separately rather than inventing
an ambiguous success state. A level bundle groups proposals for one review.

All generated artifacts, including repairs, **first persist as staged**. Even a
delegated acceptance is a second, audited transition with actor
`policy:<id>:<version>`, never a direct model write. Per-level gate modes:

- `human`: display bundle, findings and changes; accept, edit-before-accept,
  reject or regenerate a selected scope.
- `delegate_if_valid`: only after a human has explicitly accepted the gate
  policy and upstream intent, accept when hard checks pass and no unresolved
  author decision remains. Optional warning thresholds are in the policy.
- `hold`: stage only; useful for later horizons and cost control.

**Open question for the author — default gates.** Recommendation: human gate
for reader contract, cast/world seed, and series/current-arc + book shape;
delegate chapter and scene detail after those approvals, with structural errors
always blocking and creative advisories visible. Until the author selects or
accepts this profile, use human gates at every level. Offer an all-human profile
and allow changing the policy for future proposals. This is a product decision,
not permission for implementation to silently enable automation.

ADR 0003 explicitly requires every amendment to be a human decision
([docs/adr/0003-plan-amendment-classes.md:64–77][adr-policy]). Initial-plan
policy delegation is a **new bounded permission**, requiring an additive ADR
clarifying the distinction; it does not enable auto-accept on `plan_amendment`
or `canon_delta`. During a run, amendments remain human-ratified. Any future
change to that rule needs a separate author decision and superseding ADR.

### Apply, stale proposals and failure

Preflight the entire selected dependency closure: scope, expected parent
versions, record hashes, referenced IDs, gate authority, structural checks and
absence of persisted prose at chapter targets. Recheck under the existing
mutation-serialization boundary before each write. Parent edits supersede
unapplied descendants; accepted descendants are history and require explicit
new proposals. A retry cannot regenerate or replace an already accepted root.

Reuse existing service methods for domain writes, with a durable operation
ledger `(proposal_id, operation_index, input_hash, returned_ids, outcome)`.
The proposed implementation must close the write/receipt crash window: expose
transaction-aware internal implementations of existing write paths where
possible, or give creation paths a persisted idempotency key and recoverable
lookup. Do not declare exactly-once behavior from an in-memory map. This is an
acceptance requirement, not an existing property of `create_*`.

Batch preflight guarantees zero writes on preflight errors. Apply guarantees
recorded sequential progress, not cross-service atomicity: stop on failure,
show applied/failed/not-reached operations, and resume from receipts after
revalidation. Domain mutation and its operation receipt must commit together
(or be recoverably keyed); a crash between mutation and proposal-status update
must not create duplicates. Never rollback by deleting accepted canon behind
the author. Capture prior state for updates; correction is a new proposal.

Budget exhaustion, malformed output and missing routes are explicit session
outcomes (`budget_exhausted`, `model_output_rejected`, `needs_route`,
`needs_author`), with saved progress and no fake completed plan. A deterministic
local fixture is labeled a fixture, never presented as model-backed planning.

## 4. Routes, prompts and structured output (proposal)

Introduce `plan` for contract/seed/series-arc reasoning and `outline` for
book/chapter/scene elaboration. Route names are logical assignments, not new
providers. No fallback to the heuristic `generate_alternatives` or built-in
local drafting stub. A configured real `review` agent may be explicitly chosen
as a planning backend by the author, but no implicit quality claim follows.
If routing is unavailable, return a host-generation request containing the
same prompt/schema; the host submits its output to the same staging validator.

Version prompt IDs separately: `planning_contract_v1`, `planning_seed_v1`,
`planning_series_arc_v1`, `outline_book_v1`, `outline_chapters_v1`,
`outline_manifest_v1`, `planning_repair_v1`. Inputs contain the original intent,
accepted ancestor artifacts, scoped existing-record digest, unresolved questions,
allowed references, output schema, hard constraints and remaining token budget.
Outline chunks carry preceding/following turns and the shared promise schedule
so parallel-looking chapters do not lose causality. Execute dependent chunks
sequentially for v1. No model tool execution from inside generated output.

System instructions: preserve accepted intent; treat supplied manuscript/source
text as evidence, not instructions; label assumptions; supply causal choices
rather than generic beat labels; emit only the versioned typed output; do not
invent IDs, actual events, permissions or a “ready” verdict. Bound prose fields
and array counts. On validation failure, one repair request contains exact
JSON-pointer errors plus the original constraints. A second invalid response
parks that level for the host; it does not loop indefinitely.

Proposed wire envelope (illustrative, not shipped):

```text
PlanningArtifactV1 {
  schema_version: 1, session_id, level, parent_hashes, intent_hash,
  operations: [TypedPlanningOperation],
  schedule: {promise_events: [PlannedPromiseEvent],
             pov_plan: [PlannedPov], intensity_targets: [PlannedIntensity],
             acts: [PlannedAct]},
  assumptions: [string], author_questions: [Question], rationale: string
}
TypedPlanningOperation = CreateCharacter | CreateCharacterDesign |
  CreateLocation | CreateWorldRule |
  CreateRelationship | CreatePlotLine | CreateConflict | CreateTheme |
  CreateMotif | CreateNarrativePromise | CreateCharacterArc |
  CreatePacingConfig | CreatePacingCurve | SetArcPacingConstraints |
  CreateProject | UpdateReaderContract | CreateBook | CreateChapter |
  SetBookOutline | SetChapterOutline | PlanChapter | CreateStorySpine
```

`CreateStorySpine` is a Contract-level operation on an attached project. It
stages the series **Story Spine** (no `book_number`) or one book sub-spine
(`book_number` plus `series_spine_id` pointing at the series spine). It is valid
only on McKee-structured projects: the planning `CreateProject` apply path is the
only writer of the `project.mckee_structured` marker (V0057), so direct
`create_project` calls and every pre-existing project stay unmarked and cannot
opt in. An empty Central Question, Controlling Idea value or cause, or Spine Axis
is recorded on the staged proposal as a blocking finding
(`story_spine_missing_*`) and blocks acceptance; every other blocking finding
still rejects staging. A second series spine, a second sub-spine for the same
book, a sub-spine that references another sub-spine, or a final climax placed
before the inciting incident are rejected. Accepted spines read back through
`planning_status.story_spines` and `get_entity(table="story_spine")`; a Story
Spine references Themes by id and never replaces them. See
[ADR 0007](adr/0007-mckee-structure-swain-prose.md).

`CreatePlotLine` (Series-Arc level) carries the Main Plot / Subplot structure
for McKee projects: an optional Value Axis (`value_axis`), a closed structural
job for a Subplot (`subplot_job`: `contradict` / `resonate` / `set_up` /
`complicate`) and the Main Plot flag (`is_main_plot`). The same fields are on
the direct `create_plot_line` tool and the `update_entity` plot-line allowlist
(`value_axis`, `subplot_job`, `is_main_plot`), and read back through
`get_entity(table="plot_line")`. `is_main_plot` is the only public spelling;
`update_entity` maps it to the `main_plot` column and rejects the raw column
name. The closed job set and the boolean flag are validated on every write for
every project (marker-independent data integrity); only the blocking gates
below are McKee-only.

At Series-Arc and Book acceptance the validator evaluates the resulting set —
non-archived plot lines visible from the session branch (the branch plus its
ancestor lineage, child rows overriding same-named parent rows), plus the
artifact's plot-line operations and those of its staged ancestor proposals — and
records blocking findings on the staged proposal: zero or more than one Main
Plot (`plot_line_missing_main_plot` / `plot_line_multiple_main_plots`), a Main
Plot whose Value Axis differs from the accepted series Story Spine's Spine Axis
(`plot_line_main_axis_mismatch`; trimmed, then exact), any Subplot without a
structural job (`plot_line_subplot_missing_job`), and a Main Plot that also
carries a job (`plot_line_subplot_job_on_main`; a Main Plot is not a Subplot).
Walking staged ancestors lets one approved batch accept a level whose Main Plot
is supplied by a sibling level; an applied ancestor is already in the row set
and is not counted twice. Findings record on the proposal but refuse
acceptance, and the acceptance error names the operation field or the existing
row to repair. If no series spine has been accepted yet the axis comparison is
skipped rather than relitigating the missing spine here. Because the Book gate
re-checks the accepted set, a project must accept its Series Arc (and therefore
its one Main Plot) before Book acceptance.

V0058 is additive: existing rows keep NULL / 0 and unmarked projects need no
action. They are not exempt, though — a McKee project's pre-existing plot lines
must be completed through `update_entity` (`value_axis`, `subplot_job`,
`is_main_plot`) before the next Series-Arc/Book acceptance, or the gate blocks.

`CreateCharacterDesign` (Cast & World level, McKee only) stages one Character
Design per character: 2–5 contradictions, `public_self`, `personal_self`,
`private_self`, `hidden_self`, `conscious_desire`, `unconscious_desire`,
`pressure_on_protagonist` and planner-written `behavior_cues`. The character is
referenced by `PlanningRef` (a `proposal_ref` to a `create_character` in the same
artifact, or an `existing_record_id`). Acceptance writes the `character_design`
row (V0059) and registers `hidden_self` and `unconscious_desire` as **secret
canonical facts** with the character as the sole initial holder; the row stores
their fact ids, so the facts remain the single source of truth for those texts.

At Cast & World acceptance the validator evaluates the characters the artifact
introduces and records blocking findings on the proposal: a character with no
design (`character_design_missing`), fewer than two or more than five
contradictions (`character_design_too_few_contradictions` /
`character_design_too_many_contradictions`), and two designs for one character
or a design for an already-designed character
(`character_design_duplicate`). A design whose target is neither an existing
character visible from the session branch nor an earlier `create_character`
(output) in the same artifact records `character_design_unresolved_character`,
including a design placed before its own `create_character`; the design write
is pinned to the session branch and rejects a character not visible from it.
Those findings stage but refuse acceptance (they are stage-recordable, like the
Story Spine and Main Plot gates). "Major character" is read structurally: every
character created by the Cast & World artifact needs a design; characters added
outside planning are not retro-gated.
Non-McKee projects get no Character Design gate findings, but staging a design
for one carries a blocking `character_design_requires_mckee_project` finding
that rejects staging.

Planning reads are full reads: `planning_status.character_designs` carries the
design plus the hidden layer and unconscious desire texts resolved from the
secret facts. Non-planning reads are redacted: subject snapshots and scene
context strip the two design facts instead of enveloping them (the
`[SECRETS IN PLAY]` block is not used for Character Design), and
`get_character_snapshot` returns only `CharacterDesignRedactedView`
(contradictions, public self, conscious desire, pressure, behaviour cues; the
personal and private selves are not part of the writer-facing slice). Secret
fact ids, values and concealment notes never leave the planning surface. Arc milestones gain an optional `kind` (`change` / `reveal`);
both are stored in the milestone JSON, and legacy milestone JSON without the
field keeps deserializing and serializing unchanged.

`PlanningSchedule.acts` (Series-Arc and Book) carries the ordered **Act list**
as schedule metadata, not as a story record. One entry is `order` (1-based,
unique), a `climax` scene placeholder (`book_number`, `chapter_number`,
`scene_order`), the `charge` the Spine Axis holds after that climax, the
`central_question_part` the Act answers, and `answers_central_question` (true
only on the final Act). The field is additive and serde-defaulted: existing
artifacts without `acts` serialize exactly as before, and there is no new table
or migration. Acts are outline intent (ADR 0006) and are never writable through
canon deltas.

Acceptance validation for McKee projects evaluates the Act list a decision
would leave behind: applied proposals visible from the session branch's
lineage (farthest ancestor first, the session branch last, decision order
within one branch), then staged ancestors in the same approved closure, then
the artifact's own list. A later decision overrides an Act with the same
`order`; an Act the later decision does not mention keeps the earlier entry. So
a child branch inherits its parent's accepted Acts and overrides them with its
own, and a Book-level proposal can refine one Act without restating the whole
list. That merge is also the deterministic accepted-Act read used by derived
breakpoints, and the same evaluated set feeds the climax placeholder gate, so a
Chapter/Scene batch accepted together with its Act-bearing Series-Arc/Book
parent in one `planning_decide` already enforces the reserved climax. An empty
Act list adds no finding: the fixed gates below are the Act validation rules,
and an empty list is not a new blocking rule. A non-empty `schedule.acts` on any
level other than Series-Arc/Book is the blocking `act_schedule_wrong_level`
finding instead: such a list is never Act-validated and must not leak into the
accepted Acts.

Blocking findings are recorded on the staged proposal (they stage, acceptance
refuses): an Act climax whose full Charge (sign and depth) equals the previous
Act's; the highest-order Act not marked `answers_central_question`; a
non-final Act marked `answers_central_question=true`; orders that are not
`1..=n`, duplicated, or non-positive; climax chapters that do not rise strictly
with `order` inside one book; an empty `central_question_part`; a missing or
non-positive climax placement; a missing `scene_order`; and a missing Charge.
The advisory `act_depth_not_progressing` compares depth ranks (positive = 0,
contrary = 1, contradictory = 2, negation of the negation = 3) and warns when
they are not strictly increasing; it never blocks acceptance.

A Chapter or Scene artifact that plans a chapter containing an Act climax must
include a planned scene at the reserved `scene_order` (using the same evaluated
Act set, so a staged ancestor's Acts already count). A missing scene records
the blocking `act_climax_scene_missing` finding and refuses acceptance, so the
placeholder an accepted Act reserved cannot be skipped when its chapter is
detailed.

For McKee projects `act_breakpoints` are derived, not authored. A
`create_pacing_curve` operation carrying a non-empty authored map records the
blocking `act_breakpoints_authored_for_mckee` finding (and the direct
`create_pacing_curve` tool refuses it with the same repair hint); an empty map
is filled before the row is written. Derivation is deterministic: keys are
`act_<order>`, and each value is the Act climax chapter divided by the highest
Act-climax chapter in that book. Spindle does not know a book's exact final
chapter count at planning time, so the highest Act climax is the fallback
denominator; a single Act therefore yields `1.0`, an Act with no climax in the
requested book contributes nothing (an empty map when none do), and every value
lies in `(0.0, 1.0]`. Because climax chapters must rise strictly with `order`
inside a book, the derived map is monotonic. Applied proposal schedules on the
session branch's lineage supply the Acts even when the curve is accepted in the
same batch as the Act list. Non-McKee projects keep authored breakpoints byte
for byte.

`PlanningSchedule.sequences` (Chapter level) carries the **Sequence** grouping
as schedule metadata. One entry is a stable `sequence_key`, exactly one
`plot_line_id` reference, the `question` the Sequence answers, the
`inciting_scene` and the `climax_scene`, and `member_scenes` — exact planned
Scene placements in strict `(book_number, chapter_number, scene_order)` order.
Members may span chapters and books; the climax must be one of them and the
**last member in story order**, and the inciting Scene is required and may not
come after the climax. The field is additive and serde-defaulted: existing
artifacts without `sequences` serialize exactly as before, and there is no new
table or migration because the schedule stays inside the planning proposal.

Chapter/Scene acceptance evaluates the Sequence set a decision would leave
behind: applied Chapter proposals visible from the branch lineage, then staged
ancestors in the approved closure, then the artifact's own list, merged by
`sequence_key` with a later entry **refining** the same key — it overrides the
scalar fields (plot line, question, inciting Scene, climax Scene) and **unions**
`member_scenes` (deduped by story position, kept in strict story order), so
members an earlier Chapter artifact listed survive. A Chapter artifact may
therefore extend a Sequence another chapter opened. A non-empty
`schedule.sequences` on any other level (Series-Arc, Book, CastWorld, Contract)
is the blocking `sequence_wrong_level` finding and is never merged; for a McKee
project the artifact's own Sequence list is checked at every level, and only
Chapter/Scene artifacts continue into the value-chain pass. Structural blocking
findings are: a missing or duplicate key, a missing plot line or question, empty
members, a non-positive placement, members not in strict story order, a missing
inciting Scene, a missing climax, a climax that is not a member, a climax that
is not the last member, and an inciting Scene after the climax. They stage with
the proposal and refuse acceptance.

Planned Scenes gain the value-planning fields in both the planning operation
input (`PlanningPlanChapterSceneInput`) and the stored planned Scene
(`PlanChapterSceneInput` / `PlannedScene`): `primary_value_axis` (one
`SceneValueAxis` with an `axis` name and opening/closing `Charge`),
`secondary_value_axes` (at most two), `opposition`, `gap` (expected versus
actual), `scene_question`, `inaction_cost`, `risk` (1–5) and `dilemma` (the
tested character and contradiction). Every field is serde-defaulted and skipped
when absent, so existing plans keep their exact output. For a McKee project the
direct `plan_chapter` write rejects a present-but-invalid value plan (more than
two secondary axes, blank or duplicate axis names, `risk` outside 1..=5, a blank
Gap or dilemma) before persisting; the planning validator records the same
shapes as findings at the operation path. Unmarked projects are not judged by
the McKee shape rules: their optional value fields are stored exactly as sent.

Structured Beats replace string beats for McKee projects. A planned Scene
carries `beats`, an ordered list of typed Beats — a closed `kind` (`action`,
`reaction`, `turn`), an optional `actor_character_id` (a planning reference in
the artifact, a record id in the stored plan), `tactic`, `said`, `wanted` and a
per-Beat `gap` (expected versus actual) — plus `swain_label`, a free-text label
that never becomes a structural kind (ADR 0007). The actor key is spelled out
so planning's generic `*_id` reference resolution does not mistake another
operation's `actor` field for a Beat character. `turning_beat` names the Scene's
Turning Beat as a 1-based position in `beats`; a Scene therefore numbers no Beat
of its own and the reference cannot drift from the array. The same fields are
additive and serde-defaulted on `PlanChapterSceneInput` / `PlannedScene` and on
the stored chapter-plan JSON, so existing plans keep their exact output, while
`beat_structure: Vec<String>` stays readable for every project. For a McKee
project the value-chain pass also records stage-recordable blocking findings for
a Scene with no Beats, no Turning Beat reference, a reference outside the array,
a referenced Beat that is not of kind turn, and more than one Turn Beat. These
findings use the same JSON-pointer paths as the value findings
(`.../scenes/{j}/beats`, `.../scenes/{j}/turning_beat`). Unmarked projects get
no Beat findings; there is no migration because chapter plans are JSON.

**Key Facts.** A new closed-allowlist operation, `create_key_fact`, wraps the
existing planned-and-pending canonical-fact registration and is allowed at the
Chapter and Scene levels only. Its closed `kind` set is `death`,
`revealed_secret`, `relationship_turn`, `item_gained`, `item_lost`,
`power_gained`, `power_lost`; any other fact type fails deserialization, and
every non-turning fact keeps the existing mine-then-ratify flow. The input
carries the subject (the existing canonical-fact subject model, or none for a
project-wide fact), a unique `key` (the predicate), a `value`, optional
`context`, a story placement with a required `scene_order`, and an optional
`planned_scene_id` naming the planned chapter plan that owns the Scene. The
fact is **planned-and-pending**, which is the point of the operation: the Scene
may still be unplanned, so a well-formed future placement accepts without an
existing Scene row and is bound later. Acceptance applies `scene_id: None` with
the Key Fact's book/chapter, returns the canonical fact id, and leaves scene
binding to `bind_canonical_fact_to_scene` once the row exists. Stage-recordable
blocking findings cover a placement that is not a positive
book/chapter/scene, an unresolvable explicit planned-Scene reference, and a
reference/placement pair naming different Scenes. A `revealed_secret` Key Fact
needs a character subject and is registered as a secret canonical fact
(`secret = 1`) with that character in the initial circle of trust (one
`knowledge_fact` row linked by `secret_of_fact_id`) and a concealment note
explaining the planned reveal; the circle expands later through
`record_knowledge` at the dramatised reveal. Key Facts are available only to
projects created through the planning root; an existing project gets a readable
`key_fact_requires_mckee_project` staging error. V0060 adds no table: a Key Fact
is a `canonical_fact` row.

**Narrative promises.** `PlanningCreateNarrativePromiseInput` and the direct
`CreateNarrativePromiseInput` gain `planned_payoff_turning_beat` (the 1-based
payoff Beat position in the planned payoff Scene, the same numbering as the
Scene's `turning_beat`, requiring `planned_payoff`) and `target_act`
(the 1-based order of the Act the promise pays off in). V0060 adds the two
nullable columns; row mapping, subject snapshots, promise reads and the typed
export emit them only when set, so unset rows and existing projects stay
byte-identical. A promise planned at the Series-Arc or Book level is long-range
and must name a `target_act` present in the accepted/merged Act list; missing or
unknown Acts are stage-recordable blocking findings. A Chapter/Scene placement
may omit it. The pre-existing ordering rule is unchanged and still a hard
blocking error: a payoff placement before its setup. The payoff-Beat checks are
stage-recordable Blocking findings instead: a payoff Turning Beat outside the
payoff Scene's Beat list (`promise_payoff_turning_beat_out_of_range`) or naming
a Beat that is not a Turn (`promise_payoff_turning_beat_not_turn`) stages with
the finding, and only acceptance is refused. The Beat reference is optional
when the payoff is not on the Turning Beat or its Scene is outside the current
horizon. The direct `create_narrative_promise` write refuses a Beat reference
with no `planned_payoff` as a readable error.

**Deus-ex-machina guard.** On a McKee project a `revealed_secret` Key Fact needs
an earlier-planted promise whose planned payoff lands in that Scene
(`key_fact_secret_reveal_without_promise`); a chapter-wide payoff placement does
not name a Scene and cannot satisfy it. A structured Beat gains
`information_dependent` (serde default `false`, never serialized when false) and
an optional `promise_id`; when the Scene's Turning Beat is information-dependent
it must name a promise planted at an earlier placement than the Scene, so a
missing promise (`turning_beat_information_dependent_without_promise`) or a
promise planted in the same or a later Scene
(`turning_beat_promise_not_planted_earlier`) is stage-recordable Blocking. Both
guards read the merged promise view (accepted branch-visible rows, staged
ancestors, the current artifact, plus the merged promise schedule's plant and
payoff events) with the same deterministic precedence as the Act and Sequence
gates. Unmarked projects keep the additive promise fields as plain optional
data and their payloads are unchanged; they cannot use Key Facts at all (the
staging error above).

For McKee projects the artifact's own `schedule.sequences` is validated at every
planning level (a non-empty list outside Chapter is `sequence_wrong_level`) and
the value-chain pass runs at Chapter and Scene acceptance. It builds the ordered
planned-Scene list the decision would leave behind: accepted `chapter_plan` rows
visible from the branch lineage (a nearer branch shadows a farther one), then
staged-ancestor `plan_chapter` operations in creation order, then the current
artifact's operations. A later plan for the same `(book_number,
chapter_number)` replaces the earlier one entirely, which matches the
`plan_chapter` write. The merged Sequence set supplies Scene membership and
climax placements; the evaluated Act list supplies Act climax placements.
Blocking findings (stage-recordable) are: a non-event (the primary axis opens
and closes at the same Charge), a broken chain (an axis opens at a Charge other
than its last closing Charge in the preceding planned Scenes, on the primary or
any secondary axis, across chapters and batches), a Scene in no Sequence, a
missing Inaction Cost, a Sequence or Act climax Scene without a dilemma, a
dilemma naming a contradiction that is not in that character's accepted
Character Design (trim-exact; a staged CastWorld ancestor's design counts so a
batch can reference the design it is about to create), and invalid value-plan
shapes (missing primary axis, more than two secondary axes, blank or duplicate
axis names, risk outside 1..=5, malformed Gap or dilemma). Advisory findings
are: one axis turning the same depth direction in three consecutive planned
Scenes that carry it (a missing occurrence or a flat turn breaks the run), risk
dropping between consecutive planned Scenes inside one Sequence, a chapter
ending off a Value Turn, and a Character Design contradiction no dilemma names
across the planned range. A chapter "lands on a Value Turn" when its last Scene
leaves the primary axis turned relative to the chapter's **first** Scene's
opening Charge; the advisory fires when the last Scene's primary axis opens and
closes at the same Charge (the literal flat-primary case, which also records the
blocking non-event) **or** when its closing Charge equals the first Scene's
opening Charge (a net-zero chapter that turns inside but returns to its start).
Findings use the existing structured shape and JSON-pointer paths: artifact
Scenes point at `/operations/{i}/operation/input/scenes/{j}/...`, accepted
Scenes at `/accepted_scenes/{book}/{chapter}/{scene_order}/...`, and
staged-ancestor Scenes at `/staged_scenes/{book}/{chapter}/{scene_order}/...`;
repair hints name the plan that must be restaged. Unmarked projects get no
value-chain findings.

**Subplot structure.** For McKee projects the Chapter/Scene pass also evaluates
every Subplot (a merged plot line with `is_main_plot=false`) against the merged
Sequences, Convergence Points and planned Value Axes. One rule is blocking and
stage-recordable: `subplot_sequence_climax_after_convergence_point`, a Sequence
belonging to a Subplot whose climax is placed **after every one of that
Subplot's Convergence Points**. The comparison is inclusive story order, so a
climax at or before a point satisfies it, and a Subplot with no Convergence
Point is not blocked by this rule (the missing point is advisory below). The
finding points at `/schedule/sequences/{i}/climax_scene` when the current
artifact declares or refines the Sequence, otherwise at
`/merged_sequences/{sequence_key}/climax_scene`, and lists the Sequence key and
the plot line record/output ref in `related_refs`.

The remaining Subplot findings are advisory and never refuse acceptance:

- `subplot_without_convergence_point` — the merged Subplot plans no Convergence
  Point. Path: the plot line operation's `/convergence_points` field
  (`/operations/{i}/operation/input/convergence_points`) or the existing row's
  `/existing_plot_lines/{id}/convergence_points`.
- `subplot_axis_dormant` — the Subplot's Value Axis goes three consecutive
  scanned Chapters without a turn. Exact run definition: scan the distinct
  `(book_number, chapter_number)` pairs the merged planned Scenes carry, in
  story order. A Chapter **turns** the axis when any planned Scene in it carries
  any axis entry (primary or secondary) whose name matches the Subplot's Value
  Axis trim-exact and whose opening and closing Charge differ. A run continues
  only while the next scanned Chapter is the immediately following Chapter in
  the same book (`n + 1`); a chapter gap or a turning Chapter resets it. The
  advisory fires once when a run reaches three Chapters and points at the last
  planned Scene of the Chapter that completed the run.
- `subplot_job_unsupported` — the Subplot turns its axis at least once but no
  turn supports the declared job. Documented heuristic: `contradict` requires at
  least one turn ending at a negative Charge, `resonate` requires at least one
  ending positive, and `set_up` / `complicate` impose no direction requirement.
  A Subplot with no turns on its axis at all is the dormancy advisory's concern,
  not this one. Path: the plot line's `subplot_job` field.
- `subplot_deeper_than_main_plot` — inside one Act the Subplot's deepest closing
  Charge rank is deeper than the deepest closing rank on the Main Plot's axis.
  Documented Act range rule: with merged Acts ordered by `order`, Act N covers
  the Scenes **after** Act N-1's bounded climax and **at or before** Act N's own
  climax (Act 1 has no lower bound); an Act with no climax, or with a
  chapter-wide climax, is skipped and creates no boundary; a Scene after the
  last bounded Act belongs to no Act and is never compared. Ranks are
  `positive = 0 < contrary = 1 < contradictory = 2 < negation of the
  negation = 3`. The Main Plot axis is the merged Main Plot's `value_axis`,
  falling back to the accepted series Spine Axis. The advisory fires only when
  both axes turn inside the Act, and points at the deepest Subplot turn's Scene
  and axis entry.

Non-McKee projects get no Subplot findings: the whole pass is gated on the
McKee marker, and their Sequences and plot-line fields stay plain optional
data.

**Act causes.** `PlannedAct` gains an optional `cause` — `{ plot_line_id,
sequence_key }`, additive and skipped when absent, so existing Act JSON is
byte-identical. The plot line is a `PlanningRef` (a visible record id or an
active parent proposal ref) spelled `plot_line_id` so the generic planning
reference validation treats it as a `plot_line` reference; the Sequence is named
by its schedule-local `sequence_key` because that is the merge identity, so a
later Chapter refinement is followed. At Series-Arc/Book acceptance every
merged Act with a cause is revalidated against the merged Sequences and plot
lines (so a later Sequence refinement cannot silently invalidate an accepted
cause), and every failure is stage-recordable Blocking:

- `act_cause_unresolved_subplot_sequence` — the plot line does not resolve, the
  key is empty or no merged Sequence carries it, the Sequence belongs to another
  plot line, or its climax has no exact placement;
- `act_cause_not_a_subplot` — the cited plot line is the Main Plot;
- `act_cause_after_act_climax` — the cited Sequence climax comes after the Act's
  own climax (a cause precedes its effect);
- `act_cause_after_convergence_point` — the cited climax is after every
  Convergence Point on the Subplot, including a Subplot that plans none.

A cause authored by the current artifact is reported at
`/schedule/acts/{i}/cause/...`; a cause carried by an inherited Act is reported
at `/merged_acts/{order}/cause/...`. The flow supports a later Book/Series-Arc
artifact after Chapter Sequences exist: Sequences are read from the applied
Chapter proposals (not from a parent proposal), so the reference resolves, and a
Book artifact that refines one Act need not restate the others.

Plan amendments do not carry Sequence membership. On a McKee project a
`scene_add` amendment is refused at pre-flight with an error that names the
missing value plan and Sequence membership and directs the author to restage the
Chapter planning artifact; otherwise the amendment would write a value-less
Scene that planning acceptance can never re-accept. `scene_replace` edits only
prose, purpose, cast, location and rating, so the Scene's value plan is carried
through untouched. `scene_drop`, `scene_reorder` and `synopsis_update` remain
available: they do not invent value fields, but a drop can leave a Sequence
member dangling and a reorder can break the axis chain. This pass does not
cascade into `schedule.sequences` (follow-up #34); the result is reported again
the next time a Chapter artifact is staged.

Each variant wraps the existing core input shape with typed proposal references
in ID slots. The dispatcher is a closed allowlist, not an arbitrary tool name
or free-form `update_entity` payload. `schedule` is new planning metadata:
setup/reinforce/payoff intent, expected intensity and POV constraints must be
stored with accepted artifacts, not squeezed into beat-label strings. Planned
schedule entries never become promise-status events or scene beat annotations
until prose demonstrates them. Every accepted structured plan is the source
for its prose outline rendering; edits to either view invalidate the other
until an explicit synchronization proposal is accepted.

Initially plan/outline accept premise, summaries and metadata only, matching
the existing replan boundary ([models.rs:7108–7115][run-contract]). They do not
fetch scene prose or copy it into receipts. Any future planning mode that
carries actual prose must join the rating-clearance chokepoint;
`PROSE_BEARING_ROUTES` is currently draft/mine/line_edit/reader_sim/review
([ai.rs:131–139][clearance]). Even without prose, preflight an author's desired
backend coverage for the planned content ceiling; never reduce scene ratings
to satisfy a route. Do not persist secrets or provider credentials in receipts.

## 5. Validation and readiness (proposal)

Return structured findings `{code, severity, path, related_refs, expected,
actual, repair_hint}` at generation, decision and final readback. Structural
errors block policy acceptance; subjective craft advisories do not become
universal narrative laws.

| Check | Proposed algorithm / policy |
| --- | --- |
| Manifest completeness | Positive book/chapter numbers; requested range exactly covered; nonempty synopsis and scene list; scene orders contiguous from 1; nonempty summary/purpose; nonempty resolved `character_ids`; real branch-visible location; typed per-scene rating within author boundaries. Preserve research fields. Never rely on legacy metadata inferred from summary strings. Compare existing harness invariants at [plan.rs:360–432][harness-checks]. |
| References and POV | Resolve all cast/location/theme/conflict/plot/arc refs; check chapter POV exists and appears in every scene where that POV is intended. V1 uses the existing chapter-level POV field; mixed-POV scenes require splitting chapters or a later explicit scene-POV contract extension. No imaginary existing scene-POV field. |
| Promise setup/payoff | Sort `StoryPlacement` lexicographically by book/chapter/scene. A payoff must follow a setup; reinforce events must lie between setup and payoff. Every in-horizon obligation has a scheduled beat; every deferred promise has a reason/destination. Mark cross-book destinations as coarse until refined, and warn on unresolved deadlines. Do not require paying every series promise in book 1. |
| POV rotation | Author selects single-POV or an allowed roster, desired shares and maximum absence/repetition window. Compute chapter counts and gaps; enforce explicit hard constraints, otherwise warn. Do not impose equal rotation on every novel. |
| Pacing curve | New plan-only metric: assign expected intensity in [0,1] per scene; normalized position `(i-1)/(N-1)` (single scene = 0). Interpolate accepted intensity points; report mean absolute deviation and longest sustained deviation. Start with advisory deviation >0.20 for three scenes; density error >0.15 per scene type is advisory. These are proposed thresholds, not existing validator behavior. Check act breakpoints ordered/in-range and densities normalized; arc milestones obey accepted spacing/budgets. Author can make specific constraints hard or waive advisories with a reason. |
| Causality/contract | Flag scenes without an intended state change, unsupported resolutions, boundary conflicts and unrelated thread targets. Deterministic coverage can prove links exist; only a human/model judge can assess whether the causal explanation is convincing. Preserve existing `plan_chapter` style warnings [service.rs:1996–2019][plan-write]. |
| Final readiness | After acceptance and ID resolution, read back the exact persisted range and call `authoring_prepare_run` with the intended mining/checkpoint/replan policies. Report its real missing requirements, including model-route coverage. Never equate passing planner checks with a successful prepare or a completed authoring run. |

## 6. Living outline and an active authoring run (proposal)

Initial planning owns **unwritten intent**; the existing replan differ owns
proposals responding to realized chapter summaries. After a run starts, freeze
the accepted contract, cast/world roots and coarse arc plan as its planning
baseline. New initial-planning sessions may fill missing future chapters but
must not overwrite existing plans in that run. Changes to already planned
future chapters go through `plan_amendment`, preserving ADR 0003 history and
guards. Contract/canon changes require their explicit existing write/ratify
workflows, not a planner backdoor.

At each summary, retain the shipped opt-in replan ladder and human queue
([service.rs:19190–19235][replan-dispatch]).
An accepted amendment updates the planning projection's hashes/schedule and
supersedes conflicting staged outline detail. If the amendment affects a
promise or intensity assumption that is not represented in its existing
payload, flag the projection stale and stage a reconciliation proposal; do not
claim it was synchronized automatically. This P3 work must not reshape ADR
0003 classes without a separate additive compatibility design.

Reuse the Wave 1 apply seam: revision bump, then
`reseed_authoring_runs_for_chapter_plan`, retaining IDs/metadata for mapped
scenes and refreshing pending scene artifacts. The baseline implementation
uses revisioned paths and removes stale artifacts
([repository.rs:17465–17565][reseed]). Do not implement a second run-manifest
cache. Re-read run status after apply; if drafting advanced between proposal
and decision, reject stale intent rather than reseeding a drafted chapter.
A pending host draft instruction can also become stale: carry the plan revision
in the proposed handoff receipt and reject a save against a superseded revision
before it can create prose. This additional concurrency check needs explicit
implementation tests; the current reseed guard alone is not that guarantee.

## 7. One new-book entry flow for agents (proposal)

Add a host-neutral embedded `new-book` skill and have `spindle` plus MCP
initialize instructions point to it for a new project/book. Keep Codex/Grok
installation adapters, but teach the same workflow through `get_skill` and
`bible://skills/new-book`; clients need no host-specific global skill install.
The baseline initialize text already exposes `get_skill` and resources but
emphasizes reconnecting and separate setup skills
([server.rs:109–115][initialize]).

Proposed minimal additional tool surface (coordinate names/profiles with P4):

1. `planning_begin`: new or existing project selection, premise/intent, horizon,
   budget and gate policy; returns session ID and next action. It stages the
   first level, never silently starts a drafting run.
2. `planning_advance`: resume/generate one level or chunk, or submit a host
   structured artifact when a route is unavailable. Returns stage status,
   findings, cost receipt and next action; retry key prevents duplicate work.
3. `planning_status`: current graph, review bundles, applied IDs, readiness,
   budget and stale nodes; paginated detail rather than the whole bible.
4. `planning_decide`: accept/edit/reject selected bundles or explicitly accept
   the gate policy; requires expected hashes and records human/policy actor.

Entry sequence: load `new-book`; look up existing projects before offering
creation; collect the minimum premise and author constraints; begin; advance
until a gate; show concise creative choices; decide; resume; report accepted
IDs and the real prepare report. To start drafting on request, load
`authoring-supervisor`, call `authoring_start_run`, then follow its existing
execute/save/checkpoint flow. Existing low-level `create_*` tools remain
available to expert agents; the new path orchestrates their service methods.
For “new book in an existing series”, require project/branch selection and
reuse its accepted contract/canon; never create a duplicate series.

## 8. Evaluation and disposable end-to-end smoke (proposal)

Add sibling `evals/plan_judge.py` rather than silently replacing the prose
rubric. Share reusable provider/response handling, randomized/swapped-order
judging, usage receipts and human calibration mechanics from
[evals/prose_judge.py:251–350][judge-schema] and
[evals/prose_judge.py:925–960][double-judge]. Keep fake judges labeled offline
fixtures. Offline tests prove evaluation plumbing, not literary quality.

Build 30 fixed intent briefs spanning standalone/serial/series, single and
rotating POV, mystery setup/payoff, romance boundaries, quiet fiction and
multi-arc progression. Include known cast/world constraints and deliberately
conflicting inputs. Compare human-curated/manual-tool baseline plans against
candidate engine plans with the same horizon, author constraints and token
budget. Blind model/version identity and normalize presentation; include
structured manifests and coarse plans in both packets.

Pairwise rubric: contract fidelity, causal progression, character agency and
arc credibility, promise setup/progress/payoff, world feasibility, POV
allocation, pacing fit, scene specificity/draftability and room for discovery.
Each dimension returns winner/tie, anchored 1–5 scores and evidence pointing
to chapter/scene/proposal refs. A high score requires concrete connected
choices; generic labels earn no specificity credit. Use two independent judge
families, swapped positions, bootstrap confidence intervals over **briefs**
(not individual judge calls), disagreement/position-bias reporting and blinded
author calibration on a recommended 20-pair pilot. Report judged quality and
hard structural pass rate separately; do not hide invalid plans by excluding
them without recording their generation failures.

Proposed release gates: 100% structural pass on valid-input fixture cases;
all invalid-input cases fail at the right boundary; no writes before staging
and decision; no duplicate writes after interrupted retries; no drafted-plan
overwrite. Quality pilot: candidate wins excluding ties >=60%, with the 95%
interval lower bound above 50%, no contract-fidelity regression accepted by the
author, and documented judge-human agreement. If sample size cannot establish
the interval, report inconclusive and expand it; do not relabel ties as wins.
These are targets to calibrate, not measured current performance.

Disposable smoke acceptance procedure:

1. Create a temporary workspace/database and isolated route config; never use
   an author's real project. Start real MCP through the existing harness client
   boundary. Supply a fixed premise, two named characters, one location, a
   planted mystery and a three-chapter horizon (two scenes/chapter).
2. Generate all levels with fixture router responses in CI. Assert staging
   leaves bible tables untouched until acceptance, including project creation.
   Human-accept roots; explicitly authorize delegated detail gates; verify
   their persisted decision receipts and stable ref-to-ID mapping.
3. Read back the book/chapter outlines and six complete scene manifests. Assert
   promise ordering, POV policy and pacing checks. Run
   `authoring_prepare_run` for chapters 1–3 and require ready=true.
4. Start a bounded authoring run with deterministic fixture drafting; follow
   execute/save/commit/summary/checkpoint protocol through completion. Assert
   six saved scenes, continuity packages and final status. Fixture completion
   proves wiring; it does not prove prose or plan quality.
5. Separate regression case: stage then accept a future-chapter amendment
   while that chapter is pending; check revisioned artifacts and refreshed run
   manifests. Advance a target before decision and require rejection. Repeat
   after a process interruption to verify receipt recovery and no duplicates.
6. Offer a separately labeled live smoke using configured real plan/outline
   routes and the same brief, then real authoring handoff. Record model IDs,
   prompt/schema hashes, usage, elapsed time, quality results and honest skips.
   Run live models through `ai run OLLAMA_API_KEY -- <evaluation command>` when
   that provider is selected; missing credentials stop that live experiment.
   No live model call is required or performed by this docs-only PR.
7. Remove the disposable workspace and config after capturing sanitized result
   summaries. No manuscript/canon content or secret values enter shared logs.

## 9. PR-sized delivery and cost estimates (proposal)

Engineering estimates assume one engineer familiar with the repo, exclude
provider outages/review wait, and include focused tests. Migration numbers are
allocated at implementation time to avoid Wave 2 collisions.

| Phase / PR | Scope and dependencies | Acceptance criteria | Estimate |
| --- | --- | --- | --- |
| A: contracts + staging ADR | Core typed planning operations/refs; additive ADR distinguishing initial delegation from amendments; session/proposal/operation-ledger migrations. No model dispatch. | Old DB upgrades; stage/list/decide round-trip; closed operation allowlist; branch/hash/dependency guards; existing amendment classes/policies unchanged. | 3–4 engineer-days |
| B: apply + recovery | Existing service-path translator, root project attachment, durable idempotency and ID receipts. Depends A. | All-human fixture can build a complete three-chapter plan; preflight error writes zero; injected crash resumes without duplicate project/entity creation; partial failure is reported accurately. | 4–5 days |
| C: route generation | `plan`/`outline`, versioned prompts/schemas, bounded generation/repair, host-output fallback, cost ceilings. Depends A; integrate B before release. | Real configured agent produces staged artifacts; missing route/stub/malformed/budget cases are honest; no model output writes directly; exactly one repair maximum. | 3–4 days |
| D: validator + run integration | Schedule checks, POV/pacing advisories, prepare readback, stale-revision handoff, amendment projection reconciliation. Depends B/C. | Manifest negative fixtures; future-amendment reseed regression; stale host draft cannot save; drafted chapter remains immutable; final readiness equals actual prepare result. | 3–5 days |
| E: agent entry | Four minimal tools, new-book skill, initialize links and reconnect path; coordinate P4/S. Depends B–D. | Host without global Codex skill completes premise-to-ready workflow; existing-series path reuses canon; interruption resumes; profile exposes all necessary tools without unrelated catalogs. | 2–3 days |
| F: evaluation + release | Pairwise plan rubric, calibration pilot, structural suite and disposable smoke; optional live report. Depends C–E. | Gates in §8 met or explicitly reported inconclusive; fixture/live results separated; sanitized costs and human disagreements recorded. | 3–4 days |

Total: **18–25 engineer-days**, with the durable creation/apply recovery work
as the largest uncertainty. Ship behind opt-in entry tools until phase F; no
existing client or run should change merely by upgrading.

### Model budget assumptions

These are planning estimates, not provider prices or measured token counts.
For a three-chapter / six-scene smoke, budget six generation calls (contract,
seed, series/arc, book, chapter batch, manifest batch), about 24k input + 12k
output tokens total. Reserve one repair per call and at most **2x** that token
budget; stop before dispatch if the ceiling cannot cover its maximum output.
At an illustrative $3/M input and $15/M output this is $0.252 nominal, $0.504
with full reserve. Provider-specific prices must be configured externally;
report actual tokens and unknown cost when usage/prices are unavailable.

For a 30-chapter / 90-scene book, use the same coarse roots then chapter and
manifest chunks of at most three chapters: roughly 24 calls, estimated 120k
input + 70k output ($1.41 under the same illustrative rates; $2.82 reserve).
Do not generate the entire series at that resolution. These ceilings exclude
authoring prose, checkpoints and replan calls, which must be reported separately.

Thirty pairwise briefs × two orders × two judge families = 120 judge calls;
assuming 8k input + 1k output each gives 960k input + 120k output, or $4.68 at
the illustrative rates, excluding candidate generation and calibration reruns.
Count model repair, retries and both competing candidates in the experiment
ledger. Existing prose-judge provider receipts already distinguish usage and
elapsed time ([prose_judge.py:514–648][judge-provider]); extend that pattern.

## 10. Open questions for the author

1. **Default gate configuration:** recommend human approval of contract,
   cast/world seed and series/arc + book shape, then explicitly delegated
   chapter/scene acceptance. Until selected, all-human. Never auto-accept
   in-run amendments under this decision.
2. **Discovery versus detail:** recommend a three-chapter detailed horizon,
   with later arcs/books coarse. A fixed full-book manifest is an explicit
   alternative for authors who prefer it.
3. **Creative control and POV/pacing:** recommend advisory rotation/curve
   thresholds except author-declared hard constraints; single POV remains a
   first-class choice. No mandatory equal POV share or cliffhanger quota.
4. **Quality release bar:** recommend the calibrated pairwise gate in §8 and
   an author-reviewed pilot before making new-book the default entry path.
   Inconclusive evaluation keeps the feature opt-in rather than claiming a win.

Proceed with these recommendations in follow-up implementation proposals;
this document does not enact the choices or change existing ratification rules.

## Source references (pinned to audited origin/main)

[entry]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/skills/spindle/SKILL.md#L6-L29

[plot]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/skills/plot-architect/SKILL.md#L20-L48

[horizons]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/skills/plot-architect/SKILL.md#L50-L99

[contract]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/crates/spindle-core/src/models.rs#L86-L92

[project]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/crates/spindle-core/src/models.rs#L1068-L1089

[structure]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/crates/spindle-core/src/models.rs#L2953-L2985

[plan-write]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/crates/spindle-adapters/src/sqlite/service.rs#L1996-L2019

[outlines]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/crates/spindle-adapters/src/sqlite/service.rs#L2591-L2665

[manifest]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/crates/spindle-core/src/models.rs#L3838-L3887

[prepare]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/crates/spindle-mcp/src/tools.rs#L3020-L3135

[plot-model]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/crates/spindle-core/src/models.rs#L3583-L3595

[promise]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/crates/spindle-core/src/models.rs#L3675-L3683

[arc-pacing]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/crates/spindle-core/src/models.rs#L3749-L3830

[alternatives]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/crates/spindle-adapters/src/sqlite/service.rs#L14285-L14380

[routes]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/crates/spindle-adapters/src/ai.rs#L2739-L2853

[amendment]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/crates/spindle-core/src/models.rs#L7381-L7453

[differ]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/crates/spindle-adapters/src/sqlite/service.rs#L19070-L19155

[guard]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/crates/spindle-adapters/src/sqlite/service.rs#L20855-L20965

[reseed-call]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/crates/spindle-adapters/src/sqlite/service.rs#L21230-L21275

[reseed]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/crates/spindle-adapters/src/sqlite/repository.rs#L17385-L17565

[decide]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/crates/spindle-adapters/src/sqlite/service.rs#L20557-L20825

[judge]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/evals/prose_judge.py#L1-L68

[double-judge]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/evals/prose_judge.py#L863-L960

[architecture]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/docs/spindle-architecture.md#L43-L78

[contract-write]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/crates/spindle-adapters/src/sqlite/service.rs#L2139-L2185

[seed-writes]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/crates/spindle-adapters/src/sqlite/service.rs#L693-L780

[milestone]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/crates/spindle-core/src/models.rs#L998-L1010

[adr-policy]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/docs/adr/0003-plan-amendment-classes.md#L64-L77

[run-contract]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/crates/spindle-core/src/models.rs#L7108-L7115

[clearance]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/crates/spindle-adapters/src/ai.rs#L131-L139

[harness-checks]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/crates/spindle-harness/src/plan.rs#L360-L432

[initialize]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/crates/spindle-mcp/src/server.rs#L109-L115

[judge-schema]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/evals/prose_judge.py#L251-L350

[judge-provider]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/evals/prose_judge.py#L514-L648

[world-rule]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/crates/spindle-adapters/src/sqlite/service.rs#L1196-L1220

[replan-dispatch]: https://github.com/VerifiedOrganic/spindle/blob/419f40be7378d8919ac89d28139496d06b7db935/crates/spindle-adapters/src/sqlite/service.rs#L19190-L19235
