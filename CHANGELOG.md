# Changelog

## Unreleased

- Add the **three advisory McKee checkpoint answers** (issue #35). On a
  McKee-structured project the checkpoint report now carries an additive
  `mckee_advisory` section computed deterministically (no model call) from the
  accepted Story Spine, the accepted Act list, the plot lines and the written
  Scenes' realized reports: (1) **Central Question progress** — which Acts'
  `central_question_part`s the reader has actually reached because the reserved
  climax **Scene is written** at or before the range (planned-only position is
  reported separately as `acts_planned_reached`, never as answered), the Act
  totals, whether the spine's final climax Scene is written inside the range, and
  the lowest `order` among the Acts answering the whole question; (2) the
  **Controlling Idea / Counter-Idea side** the last written Scene's realized
  Spine Axis turn at or before the checkpoint proves, **across the series in
  story order** (positive = Controlling Idea, negative = Counter-Idea, no
  realized turn = `undecided`), with the axis and a book/chapter/scene basis;
  (3) likely-forgotten **Subplots** — every non-Main-Plot plot line whose Value
  Axis has not turned for `DORMANT_CHAPTER_RUN` (3) **read Chapters** since its
  last written turn (series-scoped, no adjacency requirement — this is not the
  planning validator's planned-consecutive-Chapter rule). The host checkpoint
  path writes the section at creation, the auto-checkpoint writes it before its
  verdict, and `authoring_review_checkpoint` refreshes it (so a CLI-harness
  report carries it by review time) and persists it even when the review is
  refused, so the operator can read the answers while acting on the block; an
  I/O failure on that advisory write is logged and never blocks the gate. The
  answers are **advisory only**: the approve/block gate never reads
  `mckee_advisory`, so a `counter_idea` side or three forgotten Subplots can
  never block an otherwise clean checkpoint and a flattering answer can never
  rescue a blocked one. The field is `Option` + `skip_serializing_if`, so a
  non-McKee checkpoint report keeps its exact previous bytes (golden serialized
  report test). The service returns no advisory for a non-McKee project, a
  project with no active branch, or an invalid placement (`book_number <= 0` /
  `through_chapter <= 0`); those three meanings are documented and are not a
  single "not McKee" signal. No new tool, table or migration.
- **Skill wiring for the McKee structure references** (issue #35). `new-book`,
  `plot-architect`, `character-creator` and `scene-writer` now point at the
  correct `story-structure` reference at the correct step (story-spine at the
  contract/spine gate, character-design at cast & world, value-turns at the
  series-arc/book Act and value-chain work, scene-design + dialogue-tactics for
  scene/beat planning, dialogue-line for line work) and the `new-book` skill
  states plainly that every project it creates is McKee-structured while only
  the gate delegation is opt-in. **A reference unit can now be composed without
  its owning core**: the build adds the reference's own tools and the core's
  `requires_skills` closure but not the core's unit or tools, so the
  `new-book`, `planning`, `worldbuilding`, `workflow-write` and
  `workflow-authoring` profiles load every reference their entry skills name
  without gaining `planning_*`/`get_entity` capabilities (a test loads each
  pointer under the profile the skill actually runs under). The pointers resolve
  through `get_reference(name="story-structure/…")` and the
  `bible://references/…` resource. `scene-writer/references/craft-and-voice.md`
  is reframed per ADR 0007 as **paragraph-level rendering craft for an
  already-planned McKee Beat** (MRU causality, scene/sequel rhythm as prose
  pacing, voice and failure modes), with no scene/sequel or MRU planning units;
  Swain categories survive only as an optional `swain_label` on a structured
  Beat. The beat-label vocabulary, the named targeted-repair tools and the
  anti-slop catalog/rewrite guidance that the core rewrite had dropped now live
  in that reference, so `scene-writer` stays under its 10,000-byte cap without a
  usability regression. Every unit stays inside its token budget.
- **Docs pass for the shipped McKee behaviour** (issue #35). The architecture,
  planning-engine and authoring-supervisor docs now describe the delivered
  planning levels/gates, writer visibility, realized reports and deterministic
  verify, independent critic and park-for-checkpoint, amendment cascade, skills
  and the checkpoint advisory end to end, and the stale "authoring runs do not
  read or enforce it yet" / "new-book planning entry tools are not shipped yet"
  claims are gone. The honest limitations stay documented: supervisor-side
  value-chain enforcement is not implemented, automatic amendment→resume
  detection for a parked Scene remains future work, and no amendment class can
  directly edit a Key Fact row, an existing promise or a planned Beat's promise
  reference.
- Add a **service-layer end-to-end McKee fixture** (issue #35):
  contract → Story Spine → cast & world → series arc (Acts, Sequences, Main
  Plot) → book → chapter → scene manifests, each accepted by an explicit human
  decision, then a Scene committed through `commit_scene` with a typed realized
  report and a clean deterministic McKee verify, followed by the checkpoint
  advisory read-back. The local-only model router makes any hidden model call
  fail the test.
- Add the **McKee amendment cascade over unwritten Scenes** (issue #34). On a
  McKee project, accepting a plan amendment re-validates the later planned,
  undrafted Scenes when the applied effect changed (a) a planned Scene's
  **opening or closing Charge** on any Value Axis, (b) a Key Fact's planned-Scene
  binding, or (c) a Narrative Promise payoff's planned-Scene/Turning-Beat binding.
  The trigger is detected by comparing the amendment's recorded `prior_state` with
  the plan the apply actually wrote — never inferred from the amendment class —
  and the **first changed position is a positional diff of the prior/post
  planned-scene arrays** (summary/value plan/cast/location/beats as identity), so
  a length-only change can no longer anchor at index 0 and sweep earlier Scenes.
  For a drop or reorder the renumbering puts everything from the first differing
  index onward in scope; a `scene_replace` that rewrites only an opening Charge
  still triggers and includes the amended Scene. Scope is the amended Scene plus
  later Scenes in the amended chapter and all later chapters — never earlier
  Scenes.
  The pass re-runs the SAME shared rule functions the planning gate uses — the
  #26 value-chain walk, the #28 information-dependent Turning Beat decision and
  secret-reveal promise predicate, and one merged promise-gate assembly built
  from accepted rows plus the staged-ancestor/all-proposal promise events the gate
  can see (via the branch-wide applied receipt map) — so the two cannot drift. A
  failing Scene is persisted as stale (`scene_plan_stale`, additive migration
  V0062; one row per Scene, repeat cascades update it) with the finding codes, a
  reason and the triggering amendment id, exposed additively through
  `list_plan_amendments` / `decide_plan_amendments` as `stale_scenes`. Drafting is
  decided **per Scene placement**, so an undrafted Scene in a partly drafted
  chapter is still validated; a drafted Scene is never marked or rewritten.
  A broken value-chain link stages one `scene_replace` reconciliation whose
  optional `primary_value_axis` / `secondary_value_axes` payload sets the opening
  Charge the amended chain requires (ADR
  [0008](docs/adr/0008-plan-amendment-value-plan-payload.md)); a
  `key_fact_secret_reveal_without_promise` stages a `promise_followup` at the
  latest earlier valid planned placement with its payoff at the failing Scene.
  A repeat cascade reuses a still-staged repair only while it is still correct and
  otherwise supersedes it (recording `decided_at`/`decided_by` and the reason,
  additive V0063 `plan_amendment.decision_note`), so a Scene never accumulates
  duplicate proposals. A failure with no expressible repair (a planned Beat's
  promise reference, a Key Fact row, a non-event Charge repair) is marked stale
  proposal-less with an explicit reason, never a fabricated empty repair.
  Applying a reconciliation clears the mark only when the Scene passes; a mark
  whose Scene became drafted is kept as honest stale/drafted history, never
  silently "cleared", and mark + stage + supersede + clear commit in one writer
  transaction. A `promise_followup` apply is not a no-op: its executed promise is
  part of the shared source set and later Scenes are re-validated so the mark it
  repairs clears (or is re-marked with the remaining codes). `scene_replace`'s
  value-plan keys are refused with a readable error on a non-McKee project instead
  of being silently discarded. A non-McKee project's other plan-amendment
  behaviour and tool outputs are unchanged (the new outputs are skipped when
  empty). Honest limitation: no current amendment class can edit a Key Fact row,
  an existing Narrative Promise, or a planned Beat's promise reference, so a
  direct change to any of those is not expressible; the detection hook covers the
  plan-shape binding effect a class can produce. Public discovery surface: the
  additive output fields grow full `tools/list` bytes for the profiles that carry
  `decide_plan_amendments` — default and legacy-all 876,530 → 877,618, write
  577,224 → 578,312, authoring 444,744 → 445,832, workflow-authoring 213,349 →
  214,437 (input bytes unchanged: 192,969 / 136,832 / 109,601 / 106,703) — so
  `tools/profile-budgets.json` raises exactly those measured compatibility
  ceilings; the `tools/list` snapshots and the generated legacy schema inventory
  are regenerated in the same PR, and no canonical ceiling or lint waiver changes.
- Add the **independent McKee critic and park-for-checkpoint rule**, with
  provenance-gated independence (ADR 0005). A McKee project now selects
  `scene-critic-mckee-v1` (`spindle_core::drafting::critic`) instead of
  `scene-critic-v1`; it keeps the base rubric and adds four dimensions — Value
  Turn realized, Gap lands, exposition as ammunition rather than explanation,
  and over-obvious foreshadowing. The dual-persona review round persists the
  critic's structured verdict (`agree`/`mismatch` plus notes) on the realized
  Value Turn and on the realized Narrative Promise events, together with the
  four dimension scores. The critic prompt carries the plan (axes and Charges,
  Gap, Turning Beat, planned promise events) and **not** the writer's realized
  report, so the critic cannot grade the writer's own claim. A verdict produced
  by the `review` route may gate; a verdict supplied through
  `run_dual_persona_review`'s additive `critic_output` field is persisted with
  `critic_verdict_source: "host"` (route rounds omit the marker) and is ignored
  by gating, so the Scene's own writer can never submit the verdict that gates
  its commit. The deterministic `critic_report_disagreement` scene-verify check
  fires only in the direction the realized-vs-plan rules cannot see: the
  writer's report matches the plan while the critic independently says the prose
  does not deliver it (the inverse case is already `realized_value_turn` /
  `realized_promise_event`, so one root cause never produces two errors). A
  missing, stale, host-produced or malformed verdict is an unavailable pass,
  never a finding. On a McKee run a Scene whose verify ends `parked_findings` —
  any actionable deterministic finding after the revise budget — moves to the
  new `parked_for_checkpoint` phase and the scheduler returns a fresh checkpoint
  for its chapter, never `commit scene changes`. A checkpoint that is still
  pending review is awaited; a reviewed checkpoint is history, so a second park
  in the same range creates a new checkpoint instead of deadlocking on the old
  save point. At checkpoint review the critic gates approval: a sampled Scene
  whose persisted **route** verdict disagrees with the writer's report cannot be
  approved manually or by an auto-checkpoint, the report lists it in
  `critic_disagreement_scene_ids`, and the operator must redraft it through
  `authoring_review_checkpoint`'s additive `redraft_scenes` to continue. The
  live drafting park itself is driven by deterministic findings, because
  verdicts only exist after the checkpoint-time review. A parked Scene is
  cleared only by that review: `authoring_resolve_block` now refuses a parked
  Scene in every target phase. `authoring_status` reports parked Scenes as
  `parked_scene_refs` and `phase: "parked_for_checkpoint"` (ids only). A plan
  amendment accepted before the review is honoured by the redraft; automatic
  amendment detection for a parked Scene is **not implemented** (the harness
  does not persist the parked plan revision), so the operator still requests the
  redraft after applying the amendment. The `CritiqueV1` critic path also
  selects the rubric by project marker now (`critique::critic_rubric_version`),
  so its prompt and shadow artifact carry `scene-critic-mckee-v1` for a McKee
  project and `scene-critic-v1` byte-identically otherwise. Non-McKee runs keep
  `scene-critic-v1`, get no verdict and no check, keep the existing
  commit-on-parked behaviour, and keep their previous redraft handling (no
  deliberate-redraft marker, positional checkpoint sampling). Add the
  `story-structure/dialogue-line` reference (Traditional Chinese) for line-level
  dialogue work — subtext, exposition as ammunition, over-obvious foreshadowing
  and compression — registered in the `story-structure` manifest and the
  `mckee` profile.
- Raise the **measured compatibility discovery ceilings** for the additive
  critic/status/checkpoint fields (the provenance marker on the review round,
  the verdict on the review round, the checkpoint review's `redraft_scenes`, and
  `authoring_status`'s `parked_scene_refs`). Regenerating the schema inventory
  does not itself authorize a raise. Measured UTF-8 tools/list bytes before →
  after this ticket: `default` and `legacy-all` 863,617 → 869,303 (input 191,793
  → 192,969); `authoring` 431,831 → 437,517 (input 108,425 → 109,601); `write`
  564,311 → 569,997 (input 135,656 → 136,832); `workflow-authoring` 200,856 →
  206,122 (input 105,527 → 106,703); `workflow-write` 127,695 → 132,275 (input
  72,581 → 73,071); `revision` 58,512 → 63,092 (input 23,079 → 23,569); `style`
  94,402 → 98,982 (input 34,159 → 34,649); `release` 92,241 → 96,821 (input
  46,791 → 47,281). Tool names and tool counts are untouched, and the unchanged
  profiles' ceilings stay byte-identical.
- Add **McKee realized reports and deterministic scene verify**. A written Scene
  now reports what its prose actually did, and the plan is still the law. The
  realized report (`spindle_core::realized`) carries the realized Value Turn per
  planned Value Axis (opening/closing Charge, the realized Gap and the realized
  Turning Beat), the Character Effect per present major character (`reinforce`,
  `reveal`, `contradict`, `change`, `none`), the Narrative Promise events the
  Scene advanced (`setup`, `reinforcement`, `payoff`, `subversion`,
  `abandonment`) and the planned Key Facts it dramatised. It rides all three
  continuity-carrying save entry points — `commit_scene`'s
  `CommitSceneContinuityPackage.realized_report`,
  `authoring_save_scene_draft`'s structured `realized_report` field, and the raw
  `save_scene_draft` field used by agent-mode drafting; the structured field also
  counts as structured continuity content on a McKee project only — persists in
  the new nullable
  `scene.realized_report` column (V0061) and reads back through
  `get_entity(table="scene")` and each earlier Scene's
  `novel.realized_scenes` entry. A save replaces the stored report; an unset
  report is skipped entirely, so a report-less Scene and every non-McKee project
  serialize byte-identically. Four deterministic checks join the fast
  scene-verify subset with no model calls and run only on a McKee project:
  `realized_value_turn` (planned versus realized Charge — exact comparison —,
  Gap presence, Turning Beat position, a missing report, or an unplanned axis),
  `realized_key_fact` (a planned Key Fact due in the Scene that the report does
  not list), `realized_promise_event` (a planned plant or payoff the report does
  not report) and `character_effect` (`change` only at a planned change
  milestone — pinned to the Scene or placed chapter-wide, where `change` in any
  Scene of the chapter satisfies it — and a planned change never downgraded to
  `reveal`). Findings are `error`-severity in the existing consistency shape,
  attributed to the Scene, and run only when `check_consistency` is asked for the
  scene-verify set; a bare save outside a run does not run them, and parked
  findings at commit remain #33. On a McKee project the system writes Narrative
  Promise status inside the Scene's own transaction after a confirmed realized
  event, with the saved Scene recorded as the source scene (`setup` -> `planted`,
  `reinforcement` -> `reinforced`, `payoff` -> `paid_off`, `subversion` ->
  `subverted`, `abandonment` -> `abandoned`): an unusable event rolls the Scene
  row and status rows back together, an unchanged re-save is a no-op, a re-save
  that drops a previously reported event appends an append-only correction
  restoring the pre-Scene status, and a hand-written `update_promise_status` on a
  McKee project is rejected in favour of the realized-events path. A McKee run
  must prepare with a non-zero `max_revise_attempts` (absent means the run
  default of zero): `authoring_prepare_run`, `planning_readiness` and therefore
  `planning_decide(action=prepare_authoring_run)` report it as a missing
  requirement and `authoring_start_run` refuses to start, while every unmarked
  project is unaffected. Agent-mode drafting returns the same report through the
  generated-scene package and dispatch prompt, and the `CharacterEffect` /
  `RealizedPromiseEventKind` input schemas stay closed string enums after the
  Gemini schema sanitiser.
- Raise the **measured compatibility discovery ceilings** for the additive
  realized-report DTOs; regenerating the legacy schema inventory does not itself
  authorize a raise. Measured UTF-8 tools/list bytes before → after this ticket:
  `default` and `legacy-all` 833,541 → 863,617 (input 180,181 → 191,793);
  `authoring` 406,371 → 431,831 (input 96,813 → 108,425); `write` 538,851 →
  564,311 (input 124,044 → 135,656); `minimal` 251,890 → 268,983 (input 30,863 →
  38,724); `import` 248,146 → 252,762 (input unchanged); `workflow-authoring`
  189,244 → 200,856 (input 93,915 → 105,527); `workflow-write` 119,834 →
  127,695 (input 64,720 → 72,581). The delta includes replacing the degraded
  `oneOf`-object schemas with the closed `CharacterEffect` /
  `RealizedPromiseEventKind` string enums, so the closed sets are now
  advertised. Tool names, tool counts and the canonical acceptance ceilings are
  untouched.
- Add **McKee writer visibility and the filled drafting packet**. For a
  McKee-structured project, `get_chapter_briefing` returns only the requested
  Scene's plan (`chapter_plan.scenes` is filtered to that Scene); the bundled
  scene context carries `novel.planned_scene`, earlier written Scenes of the
  chapter as summaries (`novel.realized_scenes`, a dedicated record so a
  realized Value Turn can be added additively later) and the current Scene's
  Character Design slice (`novel.character_design_slice`: tested contradiction,
  conscious desire, behaviour cues). `get_scene_context` returns the same three
  fields. Later Scenes' plans are omitted and the narrowing is marked additively
  with `chapter_plan.scene_scope` (`current_scene_only`, resolved Scene, planned
  and exposed counts; skipped for non-McKee projects). A placement-only briefing
  resolves the writer cursor's next Scene, so a later-Scene briefing no longer
  falls back to Scene 1. Promise reads now share one redaction rule
  (`spindle_core::promise_visibility`) across `narrative_promises_due`,
  continuity sheets, subject snapshots (`get_entity` /
  `get_character_snapshot`), `get_writer_state` and the
  `bible://projects/{id}/narrative-promises` resource: the planned payoff
  placement, payoff Turning Beat, target Act and payoff-encoding notes never
  appear, and a promise not yet planted at the cursor is not listed; the
  promise's nature stays visible. `prepare_scene`'s `drafting_packet_v1` is
  filled from the current planned Scene instead of the legacy placeholders —
  primary Value Axis with opening/closing Charge (readable `positive` /
  `negative (contrary)` / `negative (contradictory)` /
  `negative (negation of the negation)`), opposition, additive typed `gap`,
  Turning Beat (`turning_beat` plus the `turn` line), structured Beats as
  `required_beats`, and an honest `emotion.exit` of
  `unchanged (no planned emotion shift)` (a McKee plan has no emotion field).
  Only due **Key Facts** become `canon` hard constraints: the reserved
  `key_fact.<kind>:<author key>` predicate namespace marks them, the unique fact
  id is the constraint id, and a chapter-level (`scene_order: None`) placement is
  due in every Scene of the chapter. An ordinary planned fact is never a packet
  constraint. A due secret Key Fact ships only inside the `[SECRETS IN PLAY]`
  envelope when a circle member is present; otherwise its value is withheld and
  an explicit `due_key_facts` unavailable-pass entry records the omission.
  A Scene with no planned value plan reports an unavailable packet with a reason
  instead of placeholder text. Both the default read-tool path and the critique
  path see the same data, and the `scene-writer` skill tells the host to draft
  through a packet-only subagent where the host allows it. Every new field is
  additive, serde-defaulted and skipped when unset; jobs on non-McKee projects
  keep byte-identical briefing, scene context and packet payloads.
- Raise the **measured compatibility discovery ceilings** for the additive McKee
  DTOs and record them here; regenerating the legacy schema inventory does not
  itself authorize a raise. Measured UTF-8 tools/list bytes (outputs included)
  before → after this ticket: `default` and `legacy-all` 793,333 → 833,541;
  `minimal` 228,232 → 251,890; `write` 510,111 → 538,851; `import`
  243,064 → 248,146; `authoring` 377,631 → 406,371. The growth is the additive
  `GetChapterBriefingOutput.chapter_plan.scene_scope` and promise/Key-Fact DTO
  documentation; tool names, inputs and the canonical acceptance ceilings are
  untouched. The scene-writer core was also restored to its pre-ticket English
  text (the compression is undone) and the detailed McKee brief moved to the
  Traditional-Chinese `scene-writer/drafting-packets` reference (the already
  duplicated anti-slop shelf list and on-demand reference descriptions now point
  at the reference and manifest triggers that own them), keeping the core
  comfortably under its 10,000-byte cap.
- Add the **McKee gate profile** for planning sessions. Every session still
  starts all-human (ADR 0004); the profile is an **explicit author decision**
  made with `planning_advance(action = "set_policy")` (or `planning_begin`'s
  policy override) and a human `policy_actor` plus note: human at Contract,
  CastWorld, SeriesArc and Book, `delegate_if_valid` at Chapter and Scene, under
  the stable identity `policy_id = "mckee-default"`,
  `policy_version = 1`. A delegated decision is audited as
  `policy:mckee-default:1` on the proposal and its operation receipts.
  Activation requires a McKee context (an unattached new-book session or a
  session attached to a project carrying the marker) and is rejected for an
  attached non-McKee project. Root levels can never be delegated: every
  explicit override (including `planning_begin`'s) rejects root-level
  `delegate_if_valid` before staging, in addition to the apply-time
  policy-actor check. Chapter/Scene `delegate_if_valid` remains explicitly
  overridable back to human, and the profile identity survives the override. A
  delegated level that carries a stored or revalidated blocking finding, an
  unresolved author question, a stale policy snapshot or projection, an
  unaccepted upstream root or an over-cap advisory count is refused for the
  policy actor and stays staged for a human; one shared refusal helper drives
  both the apply checks and `planning_status`'s existing `next_action`, so the
  human-review reason cannot drift. All-human sessions keep their previous
  `next_action`/`message` wording. Canon-delta and plan-amendment ratification
  are unchanged: both still require a human operator and reject `policy:`
  actors (ADR 0001 / 0003 / 0004).
- Add **versioned `_v2` planning prompts** for McKee sessions:
  `planning_contract_v2`, `planning_seed_v2`, `planning_series_arc_v2`,
  `outline_book_v2`, `outline_chapters_v2`, `outline_manifest_v2` and
  `planning_repair_v2`. The prompt version is **pinned per session at creation**
  (`PlanningSession.prompt_version`: `2` for a McKee context, `1` for an
  attached non-McKee project; legacy rows without the field keep `_v1`
  byte-identically), so selection never re-derives from the current scope and a
  session cannot flip version on attach or advance. McKee sessions resolve the
  `_v2` family at every level (including the single repair pass) and record the
  resolved ID and prompt hash in the existing generation receipt; model-call
  usage attribution records the matching `v1`/`v2` label instead of a
  hard-coded version, and the v1 system prompts are pinned by hash in tests. The
  v2 text is the same hard-rules block plus a clearly headed, host-neutral McKee
  guidance block per level, restricted to operations each level can actually
  stage (Story Spine; Character Designs; Acts/Subplots/long-range promises; book
  outline and Acts, with promises at Series-Arc and a sub-spine at Contract;
  Sequences/value-chain plans; value plans/structured Beats/Key Facts). The
  artifact schema is unchanged. There is no new tool, table or migration: the
  profile lives in the existing session policy JSON and prompt selection in the
  existing receipts.

- Add McKee **Subplot structural checks** over planned Sequences and
  Convergence Points. A Subplot Sequence whose climax lands after every one of
  that Subplot's Convergence Points is a stage-recordable Blocking finding
  (`subplot_sequence_climax_after_convergence_point`); a climax at or before a
  point satisfies the rule, and a Subplot with no Convergence Point is advisory
  instead. Four advisories never block: a Subplot with no Convergence Point
  (also reported when a Series-Arc/Book artifact authors the Subplot, over the
  merged plot-line set), a Subplot Value Axis that goes three consecutive
  scanned Chapters without a turn (a chapter gap, a turning Chapter or a book
  change resets the run), a Subplot whose turns do not support its declared job
  (`contradict` needs a turn ending negative, `resonate` one ending positive;
  `set_up` and `complicate` impose no direction), and a Subplot turning deeper
  than the Main Plot inside one Act (Act N covers the Scenes after Act N-1's
  climax up to and including its own; the Main Plot axis is its `value_axis`,
  falling back to the series Spine Axis). The depth comparison runs only when
  the merged Acts' exact climaxes rise strictly in global story order (book,
  chapter, scene), so a cross-book out-of-order Act list skips it instead of
  reading one Act's turns as another's; a Subplot that legally shares the Main
  Plot's Value Axis reads the same turns on both sides and cannot false-positive.
  Findings use descriptive locators (`/schedule/...`,
  `/merged_sequences/{sequence_key}/...`, `/existing_plot_lines/{id}/...`) and
  are exempt for unmarked projects. The heuristics and exact run/range
  definitions are documented in `docs/planning-engine-design.md`.
- Let an **Act climax cite a Subplot Sequence climax as its cause**:
  `PlannedAct` gains an optional `cause` (`{ plot_line_id, sequence_key }`,
  additive and skipped when absent, so existing Act JSON is unchanged). The
  `sequence_key` is a **global** merge identity: a refinement must keep its plot
  line (`sequence_plot_line_changed` otherwise), and an unsafe key is rejected
  (`sequence_key_invalid`) so it stays usable in locators. At Series-Arc/Book
  acceptance the merged Act's cause must resolve to a Subplot plot line and a
  Sequence of that Subplot whose climax is at or before a Convergence Point and
  at or before the Act's own climax; Chapter/Scene staging re-checks the merged
  accepted Acts against refined Sequences, so a later refinement cannot
  silently invalidate an accepted cause. Failures are stage-recordable Blocking
  findings (`act_cause_unresolved_subplot_sequence`, `act_cause_not_a_subplot`,
  `act_cause_after_act_climax`, `act_cause_after_convergence_point`); an
  accepted Act's cause is reported at `/accepted_acts/{order}/cause/...`, a
  staged ancestor's at `/staged_acts/{order}/cause/...`. A later Book/Series-Arc
  artifact can be staged after Chapter Sequences exist, so the cause resolves in
  practice, but the forward pass usually cannot author it: the author re-stages
  that level after the Chapter Sequences are accepted.
  `planning_status` exposes the cause in the proposal schedule; there is no new
  tool, table or migration.
- Add the **Key Fact** planning operation (`create_key_fact`) to the closed
  allowlist at the Chapter and Scene levels. Its closed turning set is `death`,
  `revealed_secret`, `relationship_turn`, `item_gained`, `item_lost`,
  `power_gained`, `power_lost`; any other fact type fails deserialization, and
  every non-turning fact keeps the existing mine-then-ratify canon flow. The
  input carries the canonical-fact subject (or a project-wide fact), a unique
  key, a value, optional context, a story placement with a required
  `scene_order`, and an optional planned-Scene reference. The placement is
  **planned-and-pending**: the Scene may still be unplanned, so a well-formed
  future placement stages and applies. Acceptance writes the existing
  planned-and-pending canonical fact (`scene_id: None`, placed by the Key
  Fact's book/chapter), returns its id, and leaves binding to
  `bind_canonical_fact_to_scene` once the scene row exists. Only an explicitly
  supplied planned-Scene reference must resolve and agree with the placement;
  a placement/reference disagreement is stage-recordable Blocking. A
  `revealed_secret` Key Fact is registered as a secret canonical fact
  (`secret = 1`) with its subject character as the initial holder (one
  `knowledge_fact` row linked by `secret_of_fact_id`) and a concealment note
  explaining the planned reveal; the circle expands later through
  `record_knowledge` at the dramatised reveal. Key Facts are available only to
  projects created through the planning root: an existing project gets a
  readable `key_fact_requires_mckee_project` staging error. There is no new
  table.
- Narrative promises gain a planned payoff **Turning Beat** and a long-range
  **target Act**: `planned_payoff_turning_beat` is the 1-based payoff Beat
  position in the planned payoff Scene (same numbering as the Scene's
  `turning_beat`) and `target_act` is the 1-based Act the promise pays off in.
  V0060 adds both as nullable `narrative_promise` columns; row mapping, subject
  snapshots, promise reads and the typed export emit them only when set, so
  unset rows and existing projects stay byte-identical. For McKee projects a
  promise planned at the Series-Arc or Book level must name a `target_act` that
  exists in the accepted/merged Act list (missing or unknown Act is
  stage-recordable Blocking); Chapter/Scene placements may omit it. The
  existing payoff-before-setup ordering rule is unchanged and still blocking.
  The payoff-Beat checks are stage-recordable Blocking findings: a payoff
  Turning Beat that falls outside the planned payoff Scene's Beat list or names
  a Beat that is not a Turn stages with the finding, and only acceptance is
  refused. The direct `create_narrative_promise` write refuses a payoff Turning
  Beat with no `planned_payoff` as a readable error.
- Add the McKee **deus-ex-machina guard**: a `revealed_secret` Key Fact with no
  earlier-planted promise whose planned payoff lands in that Scene, and an
  information-dependent Turning Beat (`information_dependent` plus an optional
  `promise_id` on a structured Beat) with no promise planted at an earlier
  placement, are stage-recordable blocking findings. Both read the merged
  promise view (accepted branch-visible rows, staged ancestors, the current
  artifact, and the merged promise schedule), and both fields are skipped when
  false/unset so existing plan JSON is unchanged. Non-McKee projects keep the
  additive promise fields as plain optional data and cannot use Key Facts at
  all.
- Replace beat strings with structured **Beats** for McKee-structured projects.
  A planned Scene now carries `beats` — a closed `kind` (`action` / `reaction` /
  `turn`), an optional `actor_character_id`, the `tactic`, what is `said`, what
  is `wanted` (the subtext) and a per-Beat `gap` (expected versus actual) — plus
  an optional free-text `swain_label`. Swain categories stay optional labels and
  never become structural kinds or validation (ADR 0007). The same fields are
  additive and serde-defaulted on the planning operation input (with a planning
  reference for `actor_character_id`), the stored planned Scene, the chapter-plan
  JSON and the briefing read path, so existing plans serialize exactly as before
  and there is no migration (chapter plans are JSON).
- Add the Turning Beat: `turning_beat` names the Beat whose Gap turns the
  Scene's primary Value Axis, as the 1-based position of that Beat in `beats`;
  the generated schema advertises that `minimum: 1`. For McKee projects a Scene
  with no Beats, no Turning Beat reference, a reference outside the Beat list, a
  referenced Beat that is not of kind `turn`, or more than one Turn Beat is
  blocked at Chapter/Scene acceptance with
  stage-recordable findings on JSON-pointer paths. Unmarked projects get no Beat
  findings.
- Keep legacy `beat_structure` strings readable for every project, McKee and
  unmarked alike. On a McKee project the structured Beats are authoritative and
  the strings are not rendered as a substitute for them.
- Add the `story-structure/scene-design` and `story-structure/dialogue-tactics`
  skill references and include them in the `mckee` profile: Scene design
  (pressure, opposition, Scene Question, Gap, Turning Beat, Inaction Cost,
  structured Beats) and dialogue as action (tactic, said versus wanted, Beats
  inside dialogue, the Turning Beat's line, Swain labels as labels only). Both
  are written in Traditional Chinese.
- Add Sequence planning to the chapter-level `PlanningSchedule.sequences`
  metadata (ADR 0006): a stable key, one plot line reference, a question, a
  required inciting Scene, a climax Scene and member Scenes in strict story
  order. The climax must be the last member, and a later same-key entry refines
  the Sequence (overrides the scalar fields, unions the members). Sequences may
  cross chapters and books and merge by key across accepted and staged plans;
  they live in the proposal schedule, so there is no new table or migration.
  A non-empty Sequence list outside the Chapter level is blocked for McKee
  projects instead of being silently ignored.
- Add additive, serde-defaulted value-planning fields to planned Scenes: a
  primary Value Axis plus up to two secondary axes (axis name and opening /
  closing Charge), opposition, Gap, scene question, Inaction Cost, risk (1–5)
  and a dilemma naming the tested Character Design contradiction. Existing plans
  keep their exact output; a direct `plan_chapter` write on a McKee project
  rejects invalid shapes, while unmarked projects store the optional fields
  exactly as before.
- Add the McKee value-chain validator across the planned range: blocking
  findings for a non-event, a broken chain on any axis (across chapters and
  batches), a Scene in no Sequence, a missing Inaction Cost, a Sequence or Act
  climax without a dilemma, an unknown character contradiction and malformed
  value plans; advisory findings for same-direction runs, risk drops inside a
  Sequence, chapter endings that do not land on a Value Turn (including a
  net-zero chapter that returns its primary axis to the first Scene's opening
  Charge) and untested contradictions. Findings stage with the proposal and
  refuse acceptance, and non-McKee projects are exempt.
- Refuse `scene_add` plan amendments on McKee projects instead of writing a
  value-less Scene that no Sequence lists; `scene_replace` carries the existing
  value plan through untouched, and the other chapter classes stay available.
  Sequence-aware amendment cascades are follow-up #34.
- Mark projects created through the `new-book` planning root as
  McKee-structured. Every other project and every existing project stays
  unmarked and its payloads are unchanged.
- Add the typed Story Spine planning operation at the contract level, read back
  through `planning_status.story_spines` and `get_entity(table: "story_spine")`.
  One series spine per project, plus an optional sub-spine per book.
- Add the typed Charge (sign plus four-step depth) used by McKee structure work.
- Add the `story-structure` skill core, the `story-spine` reference and the
  `mckee` profile.
- Add migration V0057 for the McKee marker and the project-level `story_spine`
  table.
- Add McKee Main Plot / Subplot structure to plot lines: an optional Value
  Axis, one structural job per Subplot and the Main Plot flag, decided during
  Series-Arc planning and gated at Series-Arc and Book acceptance on McKee
  projects. A Main Plot is not a Subplot, so `create_plot_line` and
  `update_entity` reject a Main Plot that also carries a Subplot job.
- Add migration V0058 for the plot-line `value_axis`, `subplot_job` and
  `main_plot` columns; the fields are additive and existing rows keep NULL / 0
  until they are completed through `update_entity`.
- Add the typed Character Design planning operation at the Cast & World level
  for McKee projects: 2–5 contradictions, the four layers of self, conscious
  and unconscious desire, pressure on the protagonist and planner-written
  behaviour cues. Missing designs, out-of-range contradiction counts and
  duplicate designs are blocking findings that refuse acceptance.
- Store the hidden layer and unconscious desire as secret canonical facts
  (circle-of-trust holders) and keep only their fact ids on the design row.
  Planning tools read those texts in full through
  `planning_status.character_designs`; every non-planning read is redacted and
  never exposes the hidden layer, the unconscious desire, their secret fact ids
  or concealment notes. `get_character_snapshot` gains a redacted design slice.
- Add the optional `change` / `reveal` kind to character-arc milestones;
  legacy milestone JSON without the kind still deserializes and serializes
  unchanged.
- Add migration V0059 for `character_design`; it is additive and existing
  projects keep no rows.
- Add the `character-design` reference to the `story-structure` skill and the
  `mckee` profile.
- Add the ordered Act list to the planning schedule for McKee projects
  (`PlanningSchedule.acts`): order, climax scene placeholder, the Spine Axis
  Charge after the climax, the part of the Central Question the Act answers and
  the final-Act flag. Acts are schedule metadata, never story records and never
  editable through canon deltas (ADR 0006); there is no new table or migration.
- Validate Acts at Series-Arc and Book acceptance: an unchanged adjacent
  Charge, an unmarked final Act, a non-final Act claiming
  `answers_central_question`, climax chapters that do not rise with `order`
  inside one book, non-contiguous/duplicate/non-positive orders, an empty
  Central-Question part and missing or non-positive climax coordinates are
  blocking findings recorded on the staged proposal; a non-increasing depth
  progression is advisory only. Book acceptance re-checks the merged accepted
  Act list, so a later decision can refine one Act but cannot clash with its
  accepted neighbours. Accepted Acts are read across the branch lineage, so a
  child branch inherits its parent's Acts and can override them.
- Reject `schedule.acts` on any level other than Series-Arc/Book
  (`act_schedule_wrong_level`); such a list is never Act-validated and no longer
  leaks into the accepted-Act merge.
- Enforce the reserved Act climax placeholder in Chapter and Scene batches: a
  plan for a chapter containing an Act climax must include a scene at that
  `scene_order`, or acceptance is refused (`act_climax_scene_missing`). The
  gate evaluates the same Act set as the Act-list gate, so a chapter batch
  accepted together with its staged Act parent is already checked.
- Derive `act_breakpoints` from the Act list for McKee projects instead of
  accepting an authored map; keys are `act_<order>` and values normalize
  against the highest Act climax chapter in the book (single Act = 1.0).
  Non-McKee projects keep authored breakpoints unchanged.
- Add the `value-turns` reference to the `story-structure` skill and the
  `mckee` profile: Value Axis, Charge spectrum, Value Turn, Gap, the three
  structural levels, and how to check Act climax Charges and depth progression.

## 0.3.0 — 2026-11-09

Spindle 0.3.0 is the compatibility release for the consolidated tool surface.
It keeps the release-R aliases working while recommending the generated
workflow profiles for new sessions.

### Highlights for authors

**Plan before drafting**

- Durable new-book and planning sessions: `planning_begin`, `planning_advance`,
  `planning_status`, and `planning_decide` stage proposals level by level and
  keep human decisions explicit. The `new-book` skill takes a premise through
  intake, staged planning, review, and a ready-to-draft handoff.
- Plan review and release smoke checks catch malformed or incomplete plans
  before they reach the drafting loop.

**Canon, memory, and retrieval**

- Shared claim-vs-canon validation lanes give miners, importers, and the
  authoring loop one place to distinguish explicit canon, inferred claims, and
  contradictions.
- Layered derived memory keeps compact summaries and retrieval context fresh as
  scenes, chapters, and books grow.
- Configurable embedding providers and dimensions are tracked by index
  generation, so changing the embedding route no longer silently mixes vector
  spaces.

**Drafting packets, critic, and revision (off by default)**

- `prepare_scene` assembles a bounded scene packet with cast, setting, canon,
  continuity, promises, and voice guidance; `commit_scene` persists the prose
  and approved continuity effects atomically.
- The authoring supervisor can run deterministic verify/revise, dual-persona
  critic review, canon mining, checkpoint review, and replanning as opt-in run
  phases. These advanced loop features remain off by default pending
  measurement.
- Optional line edits and bounded chapter passes help revise without turning a
  single instruction into an unbounded rewrite.

**Tool surface**

- Generated workflow and specialist profiles are the recommended startup
  surface: `workflow-write`, `workflow-authoring`, `workflow-import`, and
  focused profiles for revision, worldbuilding, planning, new-book, research,
  style, release, and setup.
- `prepare_scene` and `commit_scene` provide the ordinary scene read/write
  boundary, with dry runs, stale-token checks, idempotency, and run-phase
  handoff.
- Typed `update_character`, `update_location`, `update_conflict`, and
  `update_character_arc` replace the supported fields of the open
  `update_entity` path. Atomic batch creates cover terms, motifs, narrative
  promises, and character voice profiles.
- `list_changes` gives one paginated history vocabulary; `undo_change` resolves
  an operation to its explicit revision scope and refuses automatic mutation
  where compensation is not atomic.
- Operator commands (`configure-agents`, `test-agent`, `rebuild-search-index`,
  `backfill-scene-source-offsets`, `operator-delete-scene`) are CLI-only and
  denied on model transports. The authoring harness keeps an internal,
  token-scoped `test_agent` dispatch.

**Security and reliability**

- Operator tools are hidden from every model profile and denied by name over
  stdio and HTTP.
- A panic in a tool handler returns a named error instead of hanging the
  caller; project serialization and internal-token dispatch are shared across
  transports.
- Scene addressing is unambiguous with `scene_id`, stale scene versions and
  preparation tokens are rejected, and commit paths avoid partial writes.
- Import, pull/push, review dispatch, and checkpoint paths no longer lose or
  duplicate work on retries, timeouts, or transport failures.

### Compatibility

0.3.0 is a **compatibility release**. The following release-R names still work
but are **deprecated** and will be removed in the next release:

- Legacy profiles: `authoring`, `write`, `minimal`, `import`, and `legacy-all`.
- Style preview twins: `preview_refresh_style_profile`,
  `preview_apply_style_profile`, and `preview_style_revision_patch`.
- Batch aliases: `batch_create_terms`, `batch_create_motifs`,
  `batch_create_narrative_promises`, and
  `batch_set_character_voice_profiles`.
- The generic `update_entity` path for entity types that now have typed
  updates; `save_scene_draft`, `revise_scene`, `authoring_save_scene_draft`,
  and `commit_scene_changes` retain their existing save-only and branch
  semantics.

Use the generated profiles and canonical tools for new work. See
[`docs/tool-profiles.md`](docs/tool-profiles.md) and the
[release-R migration guide](docs/tool-migration-r.md) for the mapping and
removal plan.

### Known limitations

- Advanced drafting-loop phases (verify/revise, dual-persona critic, canon
  mining, self-clearing checkpoints) remain off by default pending measurement.
- Semantic contradiction detection is degraded; deterministic claim-vs-canon
  lanes are the reliable path.
- There is no operator CLI for rebuilding the embedding index; index generation
  changes are tracked, but rebuild tooling is not shipped.
- Unified undo is checkpoint-only for scene continuity and multi-step style
  rollback. Scene prose restores use `restore_scene_version`; style effects use
  the explicit rollback tools.

### Added

- Unified history and undo scopes: `list_changes` paginates scene-version,
  checkpoint, and style receipts with explicit undo support; `undo_change`
  resolves an operation id and refuses automatic mutation for scopes whose
  compensation is not atomic, directing callers to the explicit revision tool.

- Canonical atomic batch creates: `create_terms`, `create_motifs`,
  `create_narrative_promises`, and `set_character_voice_profiles` accept 1-50
  items, support `dry_run`, preflight every item, and commit all-or-nothing in
  one branch transaction. The four `batch_*` names remain one-release aliases.

- Typed entity updates: `update_character`, `update_location`, `update_conflict`,
  and `update_character_arc` replace the supported fields of the open
  `update_entity` path with unknown-field-rejecting DTOs. `update_entity`
  remains a compatibility path for entity types without a typed replacement.

- Operator migration (release R): `configure-agents`, `test-agent`,
  `rebuild-search-index`, `backfill-scene-source-offsets`, and
  `operator-delete-scene` are now `spindle-mcp` CLI commands; the existing
  `init-grok-skills` CLI covers skill installation. Their old MCP names are
  hidden from every model profile (including unset and `legacy-all`) and denied
  by name on stdio/HTTP model transports. DB-mutating commands acquire the
  workspace primary lock, and `--data-dir`/`--config` override the matching
  environment variables. The authoring harness retains its internal
  `test_agent` dispatch through a server-issued token. Added
  `docs/tool-migration-r.md` with the CLI mapping, compatibility aliases,
  profile guidance, and the R+1 removal plan.
- `refresh_style_profile` and `apply_style_profile` accept `dry_run`. A dry run
  reports the proposed candidate profile or prose diff (narrator voice, style
  notes, world-rule action, metric deltas, and quality/apply-safety notes)
  without inserting a profile version, consuming or dismissing candidates,
  activating the profile, invalidating caches, or writing an application audit
  record. `preview_refresh_style_profile` and `preview_apply_style_profile`
  remain available through release R and delegate to the dry-run paths.
  `create_style_revision_patch` is the canonical non-mutating patch creation
  step; `preview_style_revision_patch` remains a release-R alias, and only
  `apply_style_revision_patch` writes prose.
- New `commit_scene` MCP tool is the single atomic write for ordinary scene
  work: it persists prose, the author-declared continuity package, beat
  annotations, an optional chapter-completion summary, and the writer cursor in
  one SQLite transaction. Deterministic validation gates run before the write,
  so blocking findings return no partial writes. `dry_run` previews the same
  effects without writing, stale preparation tokens and scene versions are
  rejected, idempotency keys make retries safe, and an authoring-run caller can
  advance the harness scene phase only after a live success. `save_scene_draft`,
  `revise_scene`, `authoring_save_scene_draft`, `commit_scene_changes`, and
  `annotate_scene_beats` keep their existing save-only and branch semantics.
- New `list_skills`, `get_skill`, and `get_reference` MCP tools expose the
  embedded workflow skills and craft references to clients without MCP resource
  support (e.g. Kimi): they return the same content as the `bible://skills/*`
  and `bible://references/*` resources and are available in every tool profile.
- A panic in any MCP tool handler now returns an error naming the panic instead
  of unwinding past the response writer, which left the caller waiting on a
  response that never came (a ~60s hang with the panic visible only on stderr).
- `delete_scene`, `operator_delete_scene`, and `move_scene` accept an optional
  `scene_id` that identifies the row unambiguously. With `scene_id` the
  position fields become optional (and are validated against the row's actual
  placement when supplied); the scene must be on the project's active branch.
  Position-only addressing is unchanged, but it could not disambiguate rows
  sharing a position and silently acted on whichever row the resolver picked —
  the root hazard behind a live-run deletion of a real scene.
- `list_chapter_scenes` returns `unresolved_alternatives`: scenes on
  `alternative`-type branches that were never promoted to the active branch
  (the unselected output of `generate_alternatives`). They are reported
  beside the spine — never inside it — so they can no longer be mistaken for
  spine scenes or deleted by position.
- `compile_manuscript` returns `stub_scenes` (`chapter.scene` refs) alongside
  `missing_scenes`: drafted scenes whose body matches obvious placeholder
  text or sits below the minimum scene word count. They still render into the
  Markdown but are named so they are never read as finished prose.
- `preflight_book_export` flags stub scenes with a Blocking `scene_stub_text`
  issue, and `check_consistency` gained a `scene_stub_text` check
  (severity `info` — stubbiness is a manuscript-readiness observation, not an
  in-story consistency violation, and the authoring loop's checkpoint
  policies approve/block on errors+warnings). A scene is a stub when its
  entire body is an obvious placeholder marker (`placeholder`, `TODO`,
  `TBD`, …) or its word count is below the floor. The floor is per-project:
  `project.min_scene_word_count` via `update_entity` (migration V0036),
  defaulting to 20 words.
- New `update_arc_milestone` tool updates a single character-arc milestone by
  label — move its `placement` and/or stamp `reached_at` — while preserving
  the fields the caller didn't pass (`description`, `unlocks`). Previously the
  only write path was resending the whole milestones array through
  `update_entity`, but `get_entity(table="character_arc")` and
  `get_character_snapshot` never projected milestones, so the array was
  effectively write-once and a placement edit guaranteed data loss. Both
  snapshots now project the full milestone array, and the
  `arc_milestone_audit` advice names the new tool.
- Characters can be renamed. `update_entity` on a character with
  `changes: {"name": ...}` previously failed with "column 'name' is not in
  the update allowlist", so a co-lead created before its in-world naming
  scene could never receive its name — authors carried the real name in the
  summary and hoped every drafting model read it. Renaming is a controlled
  operation: pass `allow_rename: true` and the call moves the name's
  project-wide uniqueness key, keeps the old name as an alias, refreshes the
  search index (FTS + embeddings), and returns a `rename_report` listing the
  scenes, canonical facts, knowledge facts, and arcs still referencing the
  old name so the author can fix them — scene prose is never rewritten
  automatically. Without the flag the rename is rejected with guidance, and
  a colliding name (any branch) names the character that holds it.
- Characters have an `aliases` field (migration V0037): settable at
  `create_character` time and amendable through
  `update_entity { changes: {"aliases": [...]} }`. Writes are shape-validated
  as an array of strings before the row is touched (`json_valid` is not
  enough; a wrong-shaped value would persist and then break every later
  `get_character` / snapshot read). Aliases resolve through `find_entity`
  exactly like the primary name and render in subject snapshots, so a record
  can keep its stable working label and carry the in-world name once it is
  decided — no rename needed. Rename preserves the old name here
  automatically. The `fts_character` index was rebuilt to include aliases.
- `register_canonical_fact` no longer requires a scene: omit `scene_id` to
  register a fact decided during planning but not yet dramatised (e.g. a
  name locked by author decision before its scene exists). The fact is
  planned-and-pending — placed by `book_number`/`chapter_number` on the
  active branch, applying in consistency checks from that placement — and
  the new `bind_canonical_fact_to_scene` tool attaches it to its scene once
  that scene is drafted (migration V0038 makes the column nullable).

### Fixed

- `reader_contract` is no longer immutable. It was set once at `create_project`
  and could never be changed — `update_entity` rejected `reader_contract`,
  `promise`, `style_notes`, and `boundaries` as "not in the update allowlist"
  because the contract lives in a single JSON column the per-column allowlist
  cannot address. This corrupted a manuscript: a stale `style_notes` chapter
  length ("1,200–2,000 words") silently overrode an explicit author directive
  ("3,500–5,000") because narrator voice and the structure world rule could be
  updated but the contract could not, and four of five drafted chapters landed
  in the superseded band. Now `update_entity(entity_type="project", ...)`
  accepts `reader_contract` (a nested object) and the sub-fields `promise`,
  `style_notes`, `boundaries`, applied as a validated partial merge — unset
  fields are preserved, wrong-shaped values are rejected before any write, and
  the StyleCompliance cache is invalidated so drafting reads the new figure.
  `apply_style_profile` is unchanged: it still only touches profile-generated
  notes, so an author's original notes are edited deliberately through this
  path.
- New contradiction guard: when `reader_contract.style_notes`, the narrator
  voice, or a style world rule is updated, Spindle cross-checks the word-count
  targets across all three surfaces and returns a `warnings` entry on
  `update_entity` / `set_narrator_voice` / `update_world_rule` when they
  disagree (e.g. the contract still says 1,200–2,000 while the narrator voice
  says 3,500–5,000). Advisory, never blocking — so a stale figure is surfaced
  at update time instead of being noticed in a word count after export.
- `create_style_profile_from_markdown` no longer produces permanently
  unappliable profiles. Guidance synthesis is model-populated, but the prompt
  only referenced "the StyleProfileGuidance schema" by name and never included
  it, so real models could not conform: the response failed to parse and the
  call silently degraded to a `NeedsReview` profile with every guidance field
  empty — which `apply_style_profile` then rejected with no escape hatch. A
  profile could be created but never applied. Now:
  - the synthesis prompt injects the actual `StyleProfileGuidance` JSON Schema
    (generated from the type, so it cannot drift), telling the model the exact
    shape to emit;
  - parsing is lenient (`#[serde(default)]` on every guidance field) so a
    partial response degrades to empty fields instead of failing wholesale,
    and an all-empty result is detected explicitly;
  - `create_style_profile_from_markdown` fails loudly — before persisting —
    when guidance comes back empty, naming the route and the `metrics_only`
    opt-out, instead of reporting success and creating an unusable record
    (metrics-only profiles are exempt by design);
  - `apply_style_profile` gains `force: Option<bool>`: forcing activates a
    `NeedsReview` or metrics-only profile deliberately, and when there is no
    prose guidance it activates without touching the project's narrator voice,
    style notes, or style world rule — so metrics-only profiles can back drift
    detection without clobbering existing prose style. `force_apply` on create
    now also carries through to the apply step.
- `update_entity` allowlist audit: records are no longer near-immutable after
  creation. Authoring scaffolds a record first and deepens it as decisions
  land, but most planning fields were frozen at create time, forcing canon
  into free-text summary fields and defeating the structured records they
  belong in. Now updatable after creation:
  - `character_arc`: `ending_state`, `starting_state`, `thematic_purpose`,
    `arc_type`, `connected_theme_ids`, `status` (an arc's destination is
    among the likeliest fields to change during planning; a live record was
    stuck reading "OPEN, deliberately so" after the decision was made).
    `progress` stays locked (computed), as do the id links.
  - `character_emotional_profile`: `base_emotions`, `suppressed`, `triggers`,
    `defense_mechanisms`, `flex_range`. Profiles were write-once at
    `create_character`, so characters scaffolded before their psychology was
    worked out — most supporting cast — were permanently stuck with empty
    profiles. Writes are shape-validated against the stored types before any
    column is touched (the schema only checks `json_valid`, and a
    wrong-shaped value would poison every later read of the profile, which
    briefings embed).
  - `world_rule`: `rule_type`, `scan_pattern`, `established_in`,
    `relevance_tags` via `update_world_rule` (which routes through the same
    allowlist and whose cache invalidation already anticipated rule_type
    edits); `faction`: `faction_type`, `realm`, `tags`; `religion`: `tags`;
    `economy`: `realm`, `scarce_resources`, `trade_goods`; `term`:
    `usage_context`, `origin`; `plot_line`: `plot_type`; `theme`:
    `introduction_point`, `resolution_point`; `motif`:
    `max_uses_per_chapter`, `connected_theme_ids`; `narrative_promise`:
    `planted_at` (symmetric with `planned_payoff` after a renumber);
    `timeline_event`: `title`, `event_type`, `placement`, `summary`,
    `related_entity_ids` (previously entirely locked, so a renumber stranded
    event placements with no fix path); `temporal_intervention`:
    `intervention_type`, `summary`, `consequences`, `status`;
    `system_overlay`: `system_type`, `rules`, `visibility`,
    `progression_currency`, `stats`, `advancement_tiers`; `future_knowledge`:
    `knowledge_summary`, `source`, `learned_at`, `expires_at`.
  Still locked by design: name-family identity columns (they back uniqueness
  indexes and the search index — renames stay controlled operations), FKs,
  and columns with dedicated typed write paths (voice profiles, scene text,
  canonical facts).
- `set_character_birth` accepts negative `day_index`. Day 1 is the story's
  opening, so every adult character was born before it; the old `>= 0` bound
  made the tool unusable for any project not set at the literal dawn of its
  own timeline, forcing ages into prose summaries where nothing can derive or
  validate them. The clock is a signed offset from the epoch everywhere it is
  consumed (ordering, elapsed-time math, rendering), and births/backstory
  timeline events now anchor pre-epoch as expected. `duration_days` remains
  non-negative (it is a magnitude), and the intra-day `time_of_day` bounds
  are unchanged.
- Verified: mixed-type `update_entity` payloads — one string field plus one
  array field in the same `changes` object — apply every field at both the
  MCP parse boundary and the service write path. An earlier failure rejected
  the combination with "changes must be a JSON object" despite `changes`
  being a valid object (the message pointed at the wrong thing entirely);
  regression tests now pin the behavior at both layers, including the
  stringified-object form some clients send.
- `scan_pattern` no longer matches inside longer words. The patterns were
  compiled as bare substrings, so a Stat Growth Rate rule with pattern
  `stat` fired on the four bytes inside "statement" (and "station",
  "status", …). Plain patterns now anchor to word boundaries on each end
  whose adjacent character is a word character, with the pattern wrapped in
  a non-capturing group so alternations stay bounded. **Behavior change:**
  patterns that relied on prefix matching (for example `resurrect` catching
  "resurrected"/"resurrection") must opt in explicitly, for example
  `resurrect\w*`. The scanner semantics version bumped to v3, so cached
  `world_rule_semantic_drift` findings are recomputed on the next
  `check_consistency` run.
- `pull_chapter_from_file` round-trips real edits. Pull used to re-derive
  scene boundaries with the import slicer's structural heuristics
  (blank-line transitions, separator lines), so any edit that changed byte
  lengths — or added a paragraph pattern the slicer reads as a scene
  break — failed with "could not map N scene range(s)" or, worse, shifted
  later scenes onto the wrong text. Files written by `push_chapter_to_file`
  are now mapped by splitting on a dedicated scene marker
  (`<!-- spindle-scene -->`), not a markdown thematic break: length-changing
  edits pull cleanly, an editor's trailing newline is trimmed rather than
  adopted, and adding/removing a scene in the file fails with a clear
  count-mismatch error naming the recovery path. Files previously pushed
  with the old `\n\n---\n\n` delimiter still split that way when the piece
  count matches the chapter; otherwise (and for external manuscripts) the
  import slicer is used. The `scene_divergence` remediation now names
  `pull_chapter_from_file` as the way to adopt file edits (re-pushing
  discards them).
- `update_entity` now allowlists `narrative_promise.planned_payoff` and
  `scene.summary`. After a chapter renumber every promise kept pointing at
  its old payoff chapter and `narrative_promise_tracking` emitted permanent
  "past planned payoff" warnings with no API path to retarget them
  (`update_promise_status` takes only status/note); the tracking advice now
  names the `update_entity` path. Scene summaries were likewise frozen at
  `save_scene_draft` time, so renumbers left stale cross-references that
  `get_chapter_briefing` kept feeding into drafting context.
- `world_rule_semantic_drift` no longer fires secrecy-class rules on
  narration or HUD text. A secrecy rule ("Nate must never disclose the
  N.A.I.P.") constrains disclosure to other characters, but the scanner
  matched the literal `scan_pattern` anywhere in scene text — interior
  narration and bracketed interface readouts included — so every correct
  usage fired and real violations were buried. Secrecy-class rules
  (classified from the rule type/name, or non-disclosure phrasing in the
  description, so existing rows need no migration) now only flag hits inside
  quoted dialogue spans; the commit gate and `check_consistency` share the
  same classifier. The validator cache context hash now also mixes in a
  scanner semantics version plus the classification inputs (`rule_type`,
  `description`): previously it keyed on rule metadata alone, so findings
  computed by the old scanner kept being served from cache after the
  scanner changed, making the fix invisible until every scene or rule was
  touched. Stale rows are retired automatically on the next
  `check_consistency` run.
- `create_chapter`'s description claimed `chapter_number` is validated
  against the next sequential slot; it isn't — arbitrary numbers with gaps
  are accepted, which is what makes restructures possible. The doc now
  describes the real behavior, and the success response echoes the
  chapter's effective `title`.
- `push_chapter_to_file`'s outside-`data_dir` error named `source_path`;
  the tool's parameter is `target_path` and the message now says so.
- The stdio server no longer dies during startup against clients that probe with
  a vendor-specific request before `initialize`. Antigravity (`agy`) opens the
  pipe with a proprietary `server/discover` request, and rmcp treats any
  pre-`initialize` message except `ping` as fatal, so Spindle exited with
  `expect initialized request, but received: ... "server/discover"` and never
  advertised a tool — the client showed the server as errored. Client traffic
  now passes through a handshake shim that answers unknown pre-`initialize`
  requests with JSON-RPC `-32601` (what standard SDK servers do), drops unknown
  pre-`initialize` notifications, and hands everything else to rmcp untouched.
  After the `notifications/initialized` handshake the shim is a pass-through.
- `run_dual_persona_review` no longer loses everything to a client timeout.
  Rounds ran inline with a single persist at the very end, so when the MCP
  client gave up (~60s) the whole run was discarded — the `dual_persona_review`
  table held zero rows despite every model call having been paid for, and the
  retry restarted from nothing. On a reasoning model a two-round review is
  minutes of wall time, so this was the normal outcome, not an edge case. The
  call now starts (or joins) a detached job, waits 25s, and returns either the
  finished review or `status: "in_progress"` with the rounds banked so far; the
  job keeps running and persists EACH round as it lands. A retry joins the same
  job rather than starting a duplicate, and collects the finished result instead
  of stacking another `rounds` on top of it. An in-flight job whose scene was
  edited does not overwrite the newer fingerprint's banked rounds: persist is
  refused unless the live prose still matches the job.
- The three review personas now dispatch concurrently instead of in sequence, so
  a round costs one model call of wall time rather than three. (A two-round
  review with a style contract configured was six sequential calls.)
- `check_consistency` with `deep_check: true` now makes forward progress across
  retries over a chapter range. The per-scene `temporal_coherence` and
  `world_rule_compliance` tiers cache each scene's raw model output keyed on the
  scene's prose fingerprint (migration V0039) and fan out with bounded
  concurrency, so a retry re-parses analyzed scenes for free and only calls the
  model for the remainder. Editing a scene misses the cache and re-analyzes. The
  "a dead or uncleared route costs exactly one attempt" guarantee is preserved:
  the first scene is probed alone before the rest fan out.
- `authoring_status` no longer times out while the run it reports on is mid-step.
  Tool calls are serialized per project, and a drafting or review step holds that
  lock for as long as its model call takes — minutes on a reasoning model — so
  the one tool that could say whether the slow step was still alive queued behind
  it. Status now degrades to the existing non-persisting reconcile when the
  project lock is busy, instead of blocking.
- `authoring_execute_next` no longer hangs until the MCP client times out when
  `.spindle/runtime/spindle.addr` names a dead listener. Reclaiming a stale addr
  file ran its liveness probe on a nested current-thread runtime via
  `block_on`, which panics ("Cannot start a runtime from within a runtime") on
  the async dispatch path every real caller uses — so the stale addr was never
  reclaimed and the whole authoring loop (verify, commit, beats, summary, draft
  route resolution) stalled with no state advance. The probe now runs on the
  caller's runtime. The unit test that covered reclamation ran as a plain
  `#[test]`, off any runtime, and passed against the broken path; it and the new
  regression test now exercise it from async.
- `list_chapter_scenes` (and `list_book_chapters`) no longer report scenes
  from OTHER branches as if they were the chapter's spine. The listing query
  filtered by `chapter_id` only, but books and chapters are branch-shared —
  so the never-selected alternatives `generate_alternatives` stores on their
  own branches leaked into the listing, producing live-run chapters with five
  listed scenes and three of them at one `scene_order`. The listing now
  shares the ONE branch-scoped spine predicate with `compile_manuscript` and
  the position resolver (`Repository::list_scenes_by_chapter_and_branch`), so
  the listing, the compiled manuscript, and position addressing can never
  disagree about which scenes are in the spine. A contract test pins the
  invariant (listing ≡ compiled spine ≡ position-addressable).
- A FAILED `save_scene_draft` no longer consumes the explicit generation
  receipt. The receipt claim ran as its own committed write before placement
  validation, so a save that then failed (`book 0 not found for project`)
  left the receipt bound to the bogus placement and burned it — forcing a
  full regeneration of explicit prose that had already been produced and paid
  for. The claim now commits inside the scene's save transaction (migration
  V0034 semantics, enforced atomically): a save that fails at any point
  leaves the receipt unbound and reusable.
- `save_scene_draft` now resolves book/chapter placement from `chapter_id` as
  its schema documents, and rejects calls that name neither `chapter_id` nor
  positive `book_number`/`chapter_number` with a validation error. Previously
  the numbers silently defaulted to 0 and the save failed deep in persistence
  with `book 0 not found for project` — which is also what corrupted the
  receipt claim above. Contradictory `chapter_id` + numbers are rejected
  loudly, mirroring `get_scene_context`.
- `create_chapter` persists the `title` it accepts. Previously the field was
  silently dropped and a later `preflight_book_export` reported
  `chapter_missing_title` for a chapter created with a title. An existing
  chapter's title is never clobbered — the ensure path only fills a missing
  one.

### Security

- An explicit generation receipt now authorizes exactly one scene. The first
  explicit `save_scene_draft` presenting a `generation_id` binds it to that
  scene's `(book, chapter, scene_order)` placement (migration V0034); a later
  explicit save on a *different* scene is rejected before any prose is
  persisted. Re-saving the same scene with the same receipt stays legal, so
  retries and operator re-saves still work. Previously one trip through the
  explicit-capable agent blanket-authorized explicit saves across every other
  scene, each stamped `agent:{id}` as though that agent had produced it.
- `get_scene_context` no longer hands an explicit scene's prose to a
  non-explicit drafting agent. When the preceding scene is Explicit and the
  scene being drafted is not, `previous_scene_tail.excerpt` carries the
  neighbour's summary instead of its closing prose and the new
  `elided_reason` field explains the substitution. Elision is a rating
  boundary, not a blanket filter — an Explicit scene following an Explicit one
  still receives the real prose — and it fails closed when the target scene's
  rating cannot be established. See "Mixed-rating chapters" in
  `docs/spindle-agent-config.md`.

- `docs/spindle-agent-config.md` no longer claims `save_scene_draft` persists
  the server-held generation output for explicit saves. The caller's
  `full_text` has been authoritative since the receipt-stub data-loss fix; the
  receipt is provenance only.
- `create_pacing_config` and `create_pacing_curve` no longer fail with
  "pacing_config not found" / "pacing_curve not found" when a row already
  exists for the project+branch (the upsert conflict path re-read by the
  freshly minted id, which the kept row never had; the read-back now uses the
  upsert key, and recreating returns the original row id).
- Authoring runs no longer deadlock over pre-existing content: a chapter with
  persisted prose and a persisted summary from outside any run is adopted on
  reconcile (scenes `beats_annotated`, `summary_saved` set) instead of blocking
  on the summary residue rule; runs already stuck at `draft_saved` self-heal on
  the next execute. `authoring_resolve_block` now also advances scenes in a
  run blocked at RUN level (and no longer requires an artifact for
  pre-existing-prose scenes); its refusal message distinguishes scene-level
  from run-level blocks.
- The save-summary step no longer honors a stale summary artifact as
  idempotency proof: the referenced `chapter_summary` row must still exist on
  the run's branch, and artifacts are stamped with (and checked against) the
  producing `run_id`; otherwise the summary is regenerated from the current
  scene packages and persisted.
- Re-saving a scene through `authoring_save_scene_draft` in a chapter whose
  summary was already persisted (the revise-during-checkpoint flow) deletes
  the now-stale summary row and artifact and resets `summary_saved`, so the
  run resumes and regenerates the summary post-approval instead of blocking.
  Completion-order validation now tolerates such reopened chapters (only a
  chapter with zero progress before a complete one is a `completion_gap`).
- `decide_canon_deltas` normalizes miner payload shapes before pre-flight and
  apply, so previously staged rows ratify without manual `edit`
  reconstruction: bare ULIDs gain their class table prefix
  (`narrative_promise:`/`conflict:`/`character_arc:`/`character:`), plural
  table/kind names singularize (`characters` → `character`),
  `description` maps to `summary`/`definition`, `role` defaults to `minor`,
  null profile structs are dropped, and a free-string `planted_at` coerces to
  the mined scene's placement (the string survives as the placement note).
- `create_character` accepts missing or null `voice_profile` and
  `emotional_profile` (they default to empty profiles).
- Deterministic `temporal_coherence` scan: "before lunch"-style deadline idioms,
  gnomic present-copula statements ("second breakfast is …"), and sentences
  with contracted past perfect ("I'd … at midnight") no longer anchor scene
  time; "two in the morning" parses as a clock hour (night) instead of the
  word "morning"; `by breakfast/lunch/supper/dinner/midnight` join the
  transition markers. The world-rule scan no longer promotes hits to Likely on
  the bare function words "without"/"despite".
- The model-backed world-rule compliance prompt now carries the scene's
  declared `content_rating` and instructs rating-scoped mandates to be judged
  against the declared tier, not on-page intensity.
- `run_dual_persona_review` rounds accumulate across reviews of the same prose
  version (fingerprint-matched top-up), so the checkpoint review-currency gate
  can be satisfied incrementally; a prose change still resets the count.
- `get_chapter_briefing` keeps recent chapter summaries under token-budget
  pressure: the section's trim priority rose above outlines/threads/plan, and
  a budget trim keeps the immediately preceding chapter's summary instead of
  clearing the list and falsely reporting "None recorded before this chapter".

## 0.2.0 — initial public release

Spindle ships as a local-first MCP server for fiction planning, drafting,
branching, revision, and story-bible search.

Highlights of the initial public surface:

- Five-crate Rust workspace: `spindle-core`, `spindle-adapters`,
  `spindle-skills`, `spindle-mcp`, `spindle-harness`.
- Embedded SQLite persistence with compiled migrations under
  `crates/spindle-adapters/migrations/`.
- Public MCP tool surface covering project structure, branching, revision,
  world and entities, plot and arc tracking, pacing, planning, consistency
  validation, semantic and full-text search, canonical-fact registration,
  model-agent routing, EPUB and bible export, the full drafting loop,
  writer-state and lookup, scene-source bridging, manuscript import, and
  knowledge recording.
- MCP resource surface for skills, project-scoped entity reads, branch and
  timeline graphs, paginated history, and direct entity reads through
  `bible://{table}:{id}` templates.
- Optional streamable HTTP MCP transport (`SPINDLE_HTTP_ADDR`) alongside the
  default stdio transport.
- Operator-driven `spindle-harness` CLI for batch drafting with editorial
  checkpoints and resumable artifacts.
- Embedded skill prompts in the binary, sourced from the root `skills/`
  directory at build time.
