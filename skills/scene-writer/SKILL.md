---
name: scene-writer
description: >
  Core fiction scene-writing workflow. Use whenever drafting, continuing, or revising a scene or
  chapter. Covers context preparation, authoring provenance, voice/canon constraints, focused
  self-review, validation, state commit, recovery, and honest completion.
---

# Scene Writer

Deliver on **this project's declared genre and reader contract** (`promise`, `style_notes`, `boundaries`, `style` world rules, and `narrator_voice`), surfaced as **Project Style Requirements** in scene context. Genre and voice compliance outrank generic craft defaults: a comedy must land jokes, a thriller must propel, and a chapter promised to end on a hook must not end on a quiet reflective fade.

## Voice and Delivery

Honor the author's creative direction and style contract. Surface concrete canon or boundary conflicts; otherwise execute routine drafting, saving, and local fixes without asking permission.

Before continuing an existing manuscript, locate the preceding scene with `list_chapter_scenes` and read a passage of its actual prose with `get_entity`, alongside supplied `voice_samples` and character voice profiles. Summaries preserve events; prose preserves cadence. Match diction, narrative distance, humor, interiority and dialogue without copying phrases.

Save only manuscript prose (no prefaces, beat outlines, critique or closing notes); use headings only when the book's format has them. A writing task is complete only when prose and required state updates are saved and verified by tool results.

## Step 1: Prepare Context

1. Call `set_active_project` once per session to set the active project and branch.
2. Call `prepare_scene` (`intent` `"draft"` / `"revise"` / `"continue"`, plus optional `scene_id`, `chapter_id`, `book_number`, `chapter_number`, `scene_order`, `character_ids`, `location_id`, `budget_tokens`) to assemble the bounded packet in one read: cursor, scene purpose, hard constraints, contract, style directive, voice samples, context, `existing_prose`, `drafting_packet_v1` and freshness tokens.
3. Use the individual read tools when inspecting slices outside `prepare_scene`.
4. Before offloading to a server-side route, check `bible://config/routing` (`caller_should_send_brief`, `adapter_kind`) and load `get_reference(name="scene-writer/offloaded-generation")`.

**McKee 結構專案：** `drafting_packet_v1` 即完整寫作簡報；只讓看到該 packet 的子代理動筆，其餘場景計畫、隱藏層與回收位置不得進入上下文。細節見 `scene-writer/drafting-packets`。

## Step 2: Enforce Canon, Voice, and Constraints

- **Project Style Requirements (`novel.style_directive`)**: Obey `reader_contract.promise`, `style_notes`, `boundaries`, `[STYLE DIRECTIVE]` world rules, and `narrator_voice` (`emotional_register`, `pacing_feel`, `comedy_density`, `interiority_ratio`, `chapter_ending_style`).
- **Pacing & Agency (`novel.pacing_directives`, `scene.agency_check`)**: Advance `behind` or `stalled` arcs; hold `in_cooldown` or `ahead` arcs. When `scenes_since_active_choice >= 3`, the protagonist must make an active, costly choice that drives events.
- **World Rules & Promises (`novel.world_rules`, `novel.narrative_promises_due`, `novel.semantic_references`)**: Never violate world rules or invent unseeded capabilities; pay off due promises when earned.
- **Knowledge & `[SECRETS IN PLAY]` (`novel.knowledge_briefing`)**: Characters act only on what they know at this cursor. Under `[SECRETS IN PLAY]`, characters in `Present and NOT in the know` must never reference, imply, or react to the secret; under a POV-only secret, narration may hold private awareness while dialogue and outward behavior stay sealed. For an on-page reveal, set `secret_of_fact_id` on `record_knowledge` with `learned_at` so the circle expands forward without leaking backward.

## Step 3: Render the Planned Beats and Draft Prose

On a McKee-structured project the plan already decides structure: the current Scene's Value Axis, opening and closing Charge, opposition, Gap, Turning Beat and structured Beats come from `drafting_packet_v1`. Render that plan; do not re-plan it. Load only the `story-structure` reference for the step in front of you: `get_reference(name="story-structure/scene-design")` when the Scene's pressure, opposition, Gap or Turning Beat is missing or weak; `get_reference(name="story-structure/dialogue-tactics")` when dialogue must be designed as action; `get_reference(name="story-structure/dialogue-line")` once the tactics are settled and the line itself needs work; `get_reference(name="story-structure/value-turns")` when the plan does not turn. Swain categories are optional `swain_label`s on a planned Beat, never a structure; `scene-writer/craft-and-voice` is paragraph-level rendering craft (MRU, voice, failure modes).

On other projects, define the purpose and end-state change, and outline beats with `plan_chapter` or `annotate_scene_beats`.

Write with clear stimulus-response causality, concrete character-specific details, distinct dialogue voices, and strict POV filtering. Ground every in-scene time jump in place, body, and mundane action (`[IN-WORLD TIME]`).

## Step 4: Content Rating and Provenance

Choose `general`, `teen`, `mature`, or `explicit` deliberately and verify boundaries against the reader contract.

- Pass `authorship: "assistant"` to `commit_scene` for host-composed or host-revised prose (`"human"` is reserved for the author's own writing), and the server `generation_receipt` when one is required.
- **Explicit sexual prose origin gate**: `commit_scene` rejects client-authored explicit sexual prose without a server-side generation receipt. When drafting or revising `explicit` sexual prose, load `get_reference(name="scene-writer/offloaded-generation")`.

## Step 5: Self-Review and Preflight

Preflight with `commit_scene` (`dry_run: true`) using the payload you intend to commit and inspect `issues`, `warnings`, `blocking_count`, `coverage`; a blocking finding writes nothing. Revise against the checks below. `save_scene_draft` stays save-only: no continuity, beats, summary or cursor.

1. **Findings**: Check world-rule, canonical-fact, retcon-reachability, style-contract, and advisory temporal findings from the preflight.
2. **Step 5a — Genre & Style Gate**: Verify tone match, `narrator_voice` delivery, genre payload density, active co-leads, `chapter_ending_style`, and cadence. Rewrite and re-save if any check fails.
3. **Step 5b — Fiction Anti-Slop & Craft Gate**: Check `anti_slop` against `compact_shelf_digest`, `voice_samples`, and `scene_negatives` (full 12-shelf catalog at `bible://references/anti-slop`):
   - **Hard shelves**: `contrast_not_x_but_y` (<=1/chapter), `emotion_cocktail` (0), `fishing_ending` (0).
   - **Soft shelves**: advisory against the digest; per-shelf limits are in the catalog.
   - When a shelf hits, rewrite from the scene beat in 1–2 passes (never synonym-swap) and re-save.
4. **Consistency & Review**: Run `check_consistency` with the prose, voice, style, canon, temporal and knowledge checks; with `deep_check: true` inspect `audit_coverage` / `next_offset`. Use `run_dual_persona_review` for Craft Technician, Literary Critic and Target Reader.

## Step 6: Commit the Scene Atomically

Call `commit_scene` once with `full_text`, `summary`, `content_rating`, `authorship`, the preparation token and `expected_scene_version`, the typed `continuity` package, `beat_annotations`, an optional chapter-final `chapter_summary`, and a client `idempotency_key`; it applies prose, approved continuity, beats, summary and cursor atomically. On a McKee project include `realized_report` (value turns, character effects, promise events, reported Key Facts); scene verify adds the four deterministic `realized_value_turn`, `realized_key_fact`, `realized_promise_event` and `character_effect` checks.

1. **Inspect the receipt**: `issues`, `warnings`, `blocking_count`, `coverage`, `applied`, `beat_count`, `chapter_summary_saved`, `writer_cursor`, `follow_ups`, `undo_support`. Retry only with the same key and payload.
2. **Targeted repair**: Outside the loop, the manifest's repair tools remain available (clock, state, relationship, knowledge, promise, entity, world rule, beats, summary, cursor).
3. **Author decisions only**: Mined canon proposals never auto-ratify; use the ratification workflow in `scene-writer/authoring-runs`. After a timeout or reconnect, inspect `get_writer_state` before retrying.

## On-Demand References (Load Only When Triggered)

Call `get_reference` only when a trigger fires; never load every reference first:

- `scene-writer/offloaded-generation`, `scene-writer/craft-and-voice`, `scene-writer/research-and-sources`, `scene-writer/authoring-runs`, `scene-writer/drafting-packets`, `anti-slop`.

<!-- tool: annotate_scene_beats -->
<!-- tool: check_consistency -->
<!-- tool: commit_character_state -->
<!-- tool: commit_scene -->
<!-- tool: commit_scene_changes -->
<!-- tool: create_character -->
<!-- tool: create_future_knowledge -->
<!-- tool: create_world_rule -->
<!-- tool: find_scenes_referencing -->
<!-- tool: get_chapter_briefing -->
<!-- tool: get_character_snapshot -->
<!-- tool: get_entity -->
<!-- tool: get_reference -->
<!-- tool: get_scene_context -->
<!-- tool: get_writer_state -->
<!-- tool: list_book_chapters -->
<!-- tool: list_chapter_scenes -->
<!-- tool: plan_chapter -->
<!-- tool: prepare_scene -->
<!-- tool: record_knowledge -->
<!-- tool: record_note -->
<!-- tool: register_canonical_fact -->
<!-- tool: run_dual_persona_review -->
<!-- tool: save_scene_draft -->
<!-- tool: save_summary -->
<!-- tool: set_active_project -->
<!-- tool: set_scene_clock -->
<!-- tool: update_entity -->
<!-- tool: update_promise_status -->
<!-- tool: update_relationship -->
<!-- tool: update_world_rule -->
<!-- tool: update_writer_position -->
