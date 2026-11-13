# ADR 0008: `scene_replace` value-plan payload extension

Status: accepted (issue #34)

ADR 0003 D1 treats plan-amendment payload shapes as a one-way door and lists
`scene_replace` as summary/purpose/cast/location only. The McKee amendment cascade
needs that class to repair a planned Scene's opening Charge after an upstream
closing Charge changed, so `scene_replace` gains two **optional** keys:

```json
{
  "scene_order": 1,
  "summary": "...",
  "purpose": "...",
  "character_ids": ["..."],
  "primary_value_axis": {"axis": "...", "opening": {...}, "closing": {...}},
  "secondary_value_axes": [{"axis": "...", "opening": {...}, "closing": {...}}]
}
```

They extend the existing payload; they do not replace it. Absent (or `null`)
keys keep the pre-#34 contract exactly: the stored value plan is carried through
untouched. Present keys replace the whole axis object. The extension is additive,
so existing staged rows and existing rejected/applied history are unchanged.

Two compatibility rules make the asymmetry explicit:

- Only a McKee-structured project stores a value plan. A `scene_replace` payload
  that carries either key on any other project is **rejected at pre-flight with a
  readable error**, never silently discarded.
- The cascade applies the keys through the same `plan_chapter` replay all other
  classes use, so plan validation (value shapes, non-events, the value chain)
  still runs.

A companion additive key on `promise_followup`, `planned_payoff_turning_beat`
(1-based, requires `planned_payoff`), lets a cascade reconciliation promise record
the payoff Beat the same way the direct promise write does. It is absent by
default and keeps the old payload shape when absent.

This ADR clarifies and extends ADR 0003 D1's `scene_replace` payload core without
editing ADR 0003 (supersede, never edit). The cascade depends on the extension:
without it, a broken value-chain link has no expressible `scene_replace` repair
and every downstream Scene would stay stale with no reconciliation.

Reversal cost is low for the keys themselves (they are additive and absent-key
behaviour is unchanged) but high for removing them once reconciliation rows are
staged: those rows would become unappliable payloads, so removal requires a
superseding ADR plus terminalizing the staged rows.
