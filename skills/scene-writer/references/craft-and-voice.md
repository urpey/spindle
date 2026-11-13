# Rendering Craft, Voice, and Failure Modes

Load this reference for **paragraph-level craft while rendering an already-planned Scene**: MRU causality, scene/sequel rhythm as prose pacing, voice and POV discipline, and failure modes. It never decides story structure.

## 0. Structure Is Decided Before This Reference

Under ADR 0007 McKee owns structure: the planned Scene's Value Axis, opening and closing Charge, Gap, Turning Beat and structured Beats (`kind`: `action` / `reaction` / `turn`) are fixed by the plan. Scene and Sequel, and motivation-reaction units, are **paragraph-level rendering habits for a planned Beat** — never planning units, never a second structural framework. If no plan exists or the plan does not turn, load `story-structure/scene-design` (or `story-structure/value-turns`) instead of inventing structure here. The plan's `turning_beat` is the only structural turn; Swain categories survive only as an optional `swain_label` on a planned Beat.

## 1. Scene/Sequel Rhythm as Paragraph Craft

Render the planned Beats in the order the plan gives them. Scene/sequel names may describe how a paragraph sequence feels, but they add no units:

- **Rendered scene pressure**: a planned `action` Beat can open on a concrete, immediate want and escalating resistance. The reader should be able to measure progress inside the paragraph, not learn the want from a later summary.
- **Rendered aftermath**: a planned `reaction` Beat can hold feeling, a costly choice between options, and the commitment that carries into the next Beat. This is where a `reaction` Beat earns its space.
- **End on the planned turn**: close the sequence on the turn the plan places at `turning_beat`. Do not add a second disaster so the prose feels busier; if the plan's turn is weak, that is a planning repair, not a prose invention.
- **Pacing proportion**: compress aftermath to speed pacing and extend it to deepen a turn's cost. The book's genre and reader contract decide which; a thriller compresses, a quiet chapter may dwell.
- **Beat labels when annotating a realized Scene**: `annotate_scene_beats` takes one label per realized Beat — `goal`, `conflict`, `disaster`, `reaction`, `dilemma`, `decision`, `revelation`, `reversal`, `escalation`, `calm`, `transition`. They describe what the prose did; they add no planning units and never change the plan's structured `kind` (`action` / `reaction` / `turn`).

## 2. Causality, Voice, and POV Discipline

- **MRU causality**: Keep stimulus -> response clear without forcing a bodily reaction, action beat, and speech line into every paragraph. Do not re-explain an emotion after action or dialogue has already shown it.
- **Dramatization vs. summary**: Dramatize pivotal choices, conflict, and intimacy; use concise summary for transitions and orientation. Avoid replacing named emotions with stock organ clichés (clenched jaws, dropped stomachs, held breath).
- **Character voice**: Anchor each speaker in `get_character_snapshot` and prior scenes (`list_chapter_scenes`, `get_entity`). Differentiate vocabulary, sentence length, verbal tics, forbidden words, formality range, and interruption habits. Keep dialogue tags unobtrusive (`said`, `asked`) unless a colored tag earns its place.
- **POV filter**: Restrict internal thoughts and knowledge strictly to the POV character. Filter sensory details and emotional coloring through what *this* POV character notices and values.

## 3. Common Failure Modes and Fixes

- **White Room Syndrome**: Dialogue in a sensory void. *Fix*: Open with a concrete detail tying the POV character's body or task to the physical room.
- **Teleporting / Drifting Time**: Unsignaled morning-to-night jumps or prose contradicting its own clock (`temporal_coherence` in `check_consistency`). *Fix*: Anchor scene openings to `[IN-WORLD TIME]`, render multi-hour or multi-day spans (`duration_days >= 1` on `set_scene_clock`) with grounded place + body + mundane-task transitions (avoiding bare `"Hours passed."` fades that trip `solitary_fade`), and mark deliberate rewinds with `temporal_mode: "flashback"`.
- **Talking Heads**: Pages of ungrounded back-and-forth speech. *Fix*: Give speakers a physical task that can go wrong or reveal subtext.
- **Info Dump**: Narrator lecturing on lore or backstory. *Fix*: Surface facts only through active conflict, discovery, or immediate stakes.
- **Passive Protagonist**: Events happen *to* the lead (`scenes_since_active_choice >= 3`). *Fix*: Force a costly, plot-driving choice in this scene.
- **Emotional Whiplash**: Instant mood flips from grief to banter. *Fix*: Earn transitions through the planned `reaction` Beat, not a bolt-on sequel.
- **Homogeneous Voices**: Interchangeable dialogue across cast members. *Fix*: Re-read each character's voice profile and prior lines; strip dialogue tags to test whether each speaker remains identifiable.
- **Stakes Deflation**: Stated threats fail to bite. *Fix*: Enforce `world_rules` and deliver real costs when a plan goes wrong.

## 4. Targeted Repair Tools After the Receipt

Repair is separate from the drafting loop; use the narrowest tool and never reopen the plan to hide a prose problem:

- **Clock and state**: `set_scene_clock`, `commit_character_state`, `update_relationship`.
- **Knowledge**: `record_knowledge` (an on-page reveal also sets `secret_of_fact_id`), or `create_future_knowledge` when the knowledge belongs to a later placement.
- **Canon and promises**: `update_entity`, `update_world_rule`, `update_promise_status`.
- **Beats, summary and cursor**: `annotate_scene_beats`, `commit_scene_changes`, `save_summary`, `update_writer_position`.

These stay available outside the ordinary loop; a mined canon proposal still needs the ratification workflow in `scene-writer/authoring-runs`.

## 5. Anti-Slop Self-Review

- **Hard shelves**: `contrast_not_x_but_y` (<=1/chapter), `emotion_cocktail` (0), `fishing_ending` (0).
- **Soft shelves**: advisory against `compact_shelf_digest`; the full 12-shelf catalog and per-shelf limits are at `bible://references/anti-slop`.
- When a shelf hits, rewrite from the scene's planned Beat in 1–2 passes, never synonym-swap a flagged line.

<!-- tool: annotate_scene_beats -->
<!-- tool: check_consistency -->
<!-- tool: commit_character_state -->
<!-- tool: commit_scene_changes -->
<!-- tool: create_future_knowledge -->
<!-- tool: get_character_snapshot -->
<!-- tool: get_entity -->
<!-- tool: list_chapter_scenes -->
<!-- tool: plan_chapter -->
<!-- tool: record_knowledge -->
<!-- tool: save_summary -->
<!-- tool: set_scene_clock -->
<!-- tool: update_entity -->
<!-- tool: update_promise_status -->
<!-- tool: update_relationship -->
<!-- tool: update_world_rule -->
<!-- tool: update_writer_position -->
