# ADR 0006: Acts and Sequences are outline intent, not canon

Status: accepted

Acts and Sequences are stored as planning metadata on accepted planning
artifacts (an ordered Act list with each climax scene, Spine Axis Charge and
depth; Sequences as groups of planned Scenes), not as story records beside plot
lines. They describe how the story is meant to be shaped, not events that
happened, so they change through planning decisions and plan amendments rather
than through canon deltas. `act_breakpoints` is derived from the Act list
instead of being authored separately.

## Considered options

- **First-class story records** (like plot lines), editable through the canon
  workflow. Rejected: it would let an outline claim sit beside realized canon
  and invite treating a planned climax as something that already happened,
  the same planned-versus-actual confusion that `reached_at` and promise status
  exist to prevent.
