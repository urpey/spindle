---
name: new-book
description: Host-neutral new-book entry flow. Take an author from a premise, through human-approved planning gates, to a plan that is genuinely ready to draft.
---

# New Book: Premise to Ready

Use this unit when the author wants to start a new book or plan a new book in an existing series. This workflow only creates and ratifies unwritten intent. It never writes prose, saves a scene draft, approves an in-run amendment, or starts a drafting run without a separate author instruction.

## Intake

Ask at most five questions and only for what the author has not already supplied:

1. What is the premise in one or two sentences?
2. What content rating ceiling should the plan respect?
3. Is this standalone, a series opener, or a new book in an existing series? How long is the near horizon?
4. Single point of view, or a named POV roster? If a roster, any hard rotation rule?
5. How deep should planning go right now: a three-chapter detailed horizon (recommended) or a different chapter range?

Record unknowns as assumptions. Do not invent canon to fill a missing answer.

## Step 1: Select or Create the Project

1. Call `list_projects` before offering creation. If the author names an existing series, ask which project and reuse its accepted contract and canon; never duplicate the series.
2. For an existing project, call `set_active_project`, then call `get_writer_state` to confirm the branch and current position.
3. For a brand-new book, keep the planning session unattached. The Contract level creates the project only after the author accepts it. Do not create a placeholder project.

## Step 2: Begin the Planning Session

Call `planning_begin` with the premise, the agreed horizon, the content-rating ceiling, and the budget when the author set one. For a new book in an existing series, pass `project_id` (and `branch_id` when the author chose a non-default branch) so the session binds to that canon.

Every session starts with human gates at every level. Do not pass a gate policy here unless the author explicitly asks to delegate Chapter or Scene detail. Contract, CastWorld, SeriesArc, and Book always stay human-approved, and in-run amendments always stay human-ratified.

The next action in the result names the safe next step. Follow it.

## Step 3: Premise and Cast Gate

1. Call `planning_advance` to stage the Contract level. It returns a staged proposal, findings, and the next action.
2. Call `planning_status` to show the author the compact proposal: reader promise, core cast and world needs, assumptions, author questions, and any blocking findings.
3. Discuss the creative choices. If the author wants changes, reject or supersede the staged proposal with `planning_decide`, adjust the premise or direction, then advance again. Repairs are replacement proposals, never edits under an existing hash.
4. Call `planning_decide` with the proposal's exact payload hash only after the author explicitly approves. Then advance the CastWorld level and repeat the same review and decision.

Never treat model output, passing checks, or silence as approval.

## Step 4: Arc and Book Gate, then the Optional McKee Profile

Continue the same staged, review, decide loop for SeriesArc and Book. Both are root levels and always need an explicit author decision:

- Show the central question, arc exit condition, promise setup and payoff schedule, POV plan, and the book turns.
- Keep later books coarse. Detail only the agreed near horizon.
- Surface author questions before asking for a decision.
- Record the author as the decision actor for each root.

If a level returns a host generation request instead of a proposal, generate the artifact with the host model, then resubmit it through `planning_advance` with the host artifact action. The same validator and human gates apply to host output.

### The McKee gate profile is opt-in

After every root level (Contract, CastWorld, SeriesArc, Book) is accepted and before you stage the Chapter batch, ask the author whether to activate the McKee gate profile. Explain the trade-off in plain terms:

- Roots stay human: Contract, CastWorld, SeriesArc and Book always need an author decision.
- Chapter and Scene batches are then accepted automatically by the audited policy actor `policy:mckee-default:1`, but only when the bundle is clean: no blocking finding and no unresolved author question. A flagged batch still comes back to the author with the reason.
- Canon deltas and plan amendments are never auto-accepted, with or without the profile.

Only on an explicit yes, call `planning_advance` with:

```json
{
  "session_id": "<session>",
  "action": "set_policy",
  "policy": {
    "gates": {
      "contract": "human",
      "cast_world": "human",
      "series_arc": "human",
      "book": "human",
      "chapter": "delegate_if_valid",
      "scene": "delegate_if_valid"
    },
    "policy_id": "mckee-default",
    "policy_version": 1
  },
  "policy_actor": "author:<name>",
  "policy_note": "Activate the McKee gate profile: roots human, Chapter and Scene delegate."
}
```

Never activate it silently or from a vague answer, and never claim that every level is human-gated once it is active. If the author declines, keep the all-human default and continue level by level.

## Step 5: Chapter and Scene Batches

1. Advance the Chapter level. With the profile active, a clean batch is already applied by `policy:mckee-default:1`; report that audited acceptance and its operation receipts instead of asking the author to rubber-stamp it. Without the profile, stage it and require an explicit author decision as before.
2. Advance the Scene level for every chapter in the horizon: purpose, cast, location, rating, beats, and promise movements.

## Step 6: Scene Packets and Handoff

1. Call `planning_status` and read the real readiness report. It distinguishes planner checks from the authoring prepare result.
3. Call `planning_decide` with the prepare action to get the real prepare report for the exact range. Do not claim readiness from planner checks alone.
4. When readiness passes, summarize accepted IDs, the chapter and scene packets, and any advisory warnings. Then load the drafting workflow: `scene-writer` for one scene, or `authoring-supervisor` for a supervised multi-chapter run. Starting prose remains a separate author instruction.
5. If readiness fails, show the missing requirements and advance or repair the relevant level. `authoring_prepare_run` remains available for an independent low-level recheck.

## Reconnect

Planning is durable. If a session is interrupted, call `planning_status` without a session ID to list active sessions, then call it with the chosen session ID. The status result reports the active gate, staged proposals, findings, readiness, and the next safe action. Continue from there; do not restart an accepted level.

<!-- tool: authoring_prepare_run -->
<!-- tool: create_book -->
<!-- tool: create_chapter -->
<!-- tool: create_project -->
<!-- tool: get_writer_state -->
<!-- tool: list_projects -->
<!-- tool: planning_advance -->
<!-- tool: planning_begin -->
<!-- tool: planning_decide -->
<!-- tool: planning_status -->
<!-- tool: set_active_project -->
