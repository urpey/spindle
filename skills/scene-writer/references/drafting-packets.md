# Drafting packets

### The McKee writing brief

For a McKee-structured project, `prepare_scene` returns a filled
`drafting_packet_v1` and that packet is the whole writing brief. Its scene card
is filled from the current planned Scene: the primary Value Axis with opening
and closing Charge, opposition, Gap, the Turning Beat and the structured Beats
as required beats. `hard_constraints` carry every Key Fact due in this Scene
(kind `canon`, with the fact value). `promises_due` carries a promise's nature
(type and description) and never its planned payoff location, target Act or
payoff Turning Beat. `character_design_slice` carries only the tested
contradiction, conscious desire and behaviour cues.

Draft through a subagent that sees only that packet where the host allows it
(best effort, not enforced). Do not give it the chapter plan, other Scenes'
plans, `hidden_self`, `unconscious_desire`, their secret fact ids or any payoff
location. Render the planned turn and the due facts exactly; never invent,
omit or alter them. A Scene with no planned value plan yields an unavailable
packet ("drafting_packet_v1" in `unavailable_passes`), never placeholder copy.

The packet prose task contains its own compact rubric; do not append this skill,
standards manuals or tool instructions to a stateless packet completion.

PR 2 adds internal opt-in Rust packet-draft APIs and a P2 extractor consumer
contract. It does not add an authoring tool or enable a run policy. A verified
model tokenizer is required; unknown framing stops before dispatch. Plain-text
artifacts preserve exact bytes and receipt provenance. Truncated drafts cannot
be accepted. Accepted-text extraction failures keep prose staged for a retry;
they never request another draft. Continue using the public legacy package
workflow until host/agent integration ships.

Unsaved scenes retain immutable book/chapter/scene placement for extraction.
Once a packet dispatch starts, cancellation or an unresolved failure leaves a
recovery marker; resume must not silently dispatch another draft. Inspect the
artifact/receipt before explicit recovery. Selected extraction canon is preceding
ratified evidence, never future plans or drafter assertions.

PR 3 supplies configured, pinned local tokenizers and an operator-only
`spindle-tokenizer` calibration/count utility. It adds no authoring tool or run
policy. Assembly counts offline; optional final-message provider verification is
on by default and may use a billed one-token Ollama Cloud probe. Probe usage is
visible in the model ledger, and uncertain attempts require explicit recovery.
See `docs/spindle-agent-config.md` for setup; never substitute an estimated token
count or append skill content to the packet prompt.

<!-- tool: prepare_scene -->
