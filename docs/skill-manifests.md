# Skill manifest alignment

`skills/<name>/manifest.toml` is the versioned dependency inventory for each of
the 15 embedded skill cores. A manifest declares its direct tool requirements,
required skill cores, and currently supported profiles. Dependencies contribute
their transitive tool requirements. Routing to a specialist skill in prose does
not automatically enable that specialist or union its tools into the entry skill.

`[[references]]` declares an independently loadable unit with a name, Markdown
path under `references/`, trigger, direct tools, and supported profiles. Loading a
reference requires both its tools and its owning core's dependency closure.
The nested units are `spindle/setup`, `editor/style`,
`story-structure/story-spine`, `story-structure/character-design`,
`story-structure/value-turns`, `story-structure/scene-design`,
`story-structure/dialogue-tactics`, `story-structure/dialogue-line`, and the five
`scene-writer` references
(`scene-writer/authoring-runs`, `scene-writer/craft-and-voice`,
`scene-writer/drafting-packets`, `scene-writer/offloaded-generation`, and
`scene-writer/research-and-sources`); load them with
`get_reference(name="spindle/setup")` or the equivalent
`bible://references/spindle/setup` URI. Existing craft names and skill names remain
valid. Lookups match exact compiled names; the text behind a name is read at
runtime when a content directory is configured (see **Runtime markdown** below).

The build checks TOML shape/version, names, duplicates, dependency cycles and
reference confinement (including canonicalized paths). The skills crate retains
no runtime dependencies. MCP tests validate tool names against the actual
registry, forbid operator dependencies, and check both discovery and dispatch
availability for every declared profile. Every named tool mention in core or
reference prose needs a declaration (possibly inherited) and an HTML
`<!-- tool: name -->` marker. Unknown markers fail too. Markers are inventory
annotations, never alternate callable names. This deliberately conservative
scanner covers prose, code spans, and fenced examples; remove obsolete tool
mentions instead of concealing them behind negative examples.

`skills/profiles.toml` now composes generated workflow profiles from core and
reference units. The build includes transitive skill dependencies and appends
generated profile membership to embedded manifests. Authored `profiles` arrays
remain coverage assertions for compatibility profiles. The unset `default` label
means no startup setting. See [profile selection and measurements](tool-profiles.md)
for new names, enabled units, static compiler snapshots and transitional limits.
The `spindle` entry core is host-neutral (`<= 1,000` estimated tokens), `scene-writer`
core is split into core (`<= 2,500` estimated tokens) plus five on-demand references,
and every other skill core and reference is capped at `<= 3,000` estimated tokens
(`skill_load_budgets`). `get_skill` and `get_reference` report structured profile
coverage metadata (`covered`, `required_profile`, `available_units`,
`missing_capabilities`) and return a profile-selection message when the active
session profile does not cover the requested unit. Reading a skill never grants
new capabilities or mutates the active profile.

`tools/tool-dispositions.toml` freezes every currently shipped registry name.
Retained entries need a skill/reference owner; compatibility entries have an
explicit owner and review release; operator entries cannot become skill
dependencies. Operator entries are now CLI-only: their commands are shipped in
`spindle-mcp`, hidden from every model profile, and denied by name on model
transports. See [release R migration](tool-migration-r.md). The ledger still does
not claim replacement aliases; alias input/result adapters and R/R+1 removal
metadata are added with their actual replacements, and no alias is a mere
string rename.

Run `cargo test -p spindle-skills`, `cargo test -p spindle-mcp alignment`, and
`cargo test -p spindle-mcp nested_skill_references_load`. Metadata is generated
from embedded manifests and compared to
`crates/spindle-mcp/src/tools/alignment/metadata.json`. After an intentional
manifest change, regenerate it with
`SPINDLE_UPDATE_SKILL_SNAPSHOT=1 cargo test -p spindle-mcp skill_manifest_metadata_matches_snapshot`,
inspect the diff, and run the test without the update setting. Never hand edit
generated metadata. Normal workspace CI runs all these tests; negative fixtures
cover unavailable selection, missing offload dependency, typo, operator dependency,
missing marker, and a newly added unowned tool.

## Runtime markdown (edit without rebuilding)

Skill **text** is read from disk on every call, so optimizing a skill needs no
rebuild and no restart. The compiled manifests still decide what exists:
a file on disk can change the text of a known unit but cannot add a skill, a
reference or a tool.

| Read at runtime | Still compiled |
| --- | --- |
| `skills/<name>/SKILL.md` | `manifest.toml` (tools, profiles, dependencies) |
| `skills/<name>/references/*.md` | `profiles.toml` composition |
| `references/*.md` (craft references) | the unit inventory and every tool name |
| | the compiled fallback copy |

Point a running server at a checkout with two environment variables, both
relative to the server's working directory:

```bash
# defaults are exactly the repo layout, so a dev run from the repo needs neither
SPINDLE_SKILLS_DIR=/path/to/spindle/skills \
SPINDLE_REFERENCES_DIR=/path/to/spindle/references \
  ./target/release/spindle-mcp
```

A directory that is not there means the compiled copy; that is silent for the
default and a `/health` warning when the variable named a path that does not
exist. Everything reads through the same source, so an edit shows up on the
next `get_skill`, `get_reference`, `bible://skills/<name>` or
`bible://references/<name>`.

If a file is missing or unreadable, Spindle serves the compiled copy, logs a
`skill content:` warning, and lists it in `GET /health`:

```json
"skills": { "reads_from_disk": true, "skills_dir": "skills", "references_dir": "references",
            "warnings": ["skills/scene-writer/SKILL.md: No such file or directory …"] }
```

The warning clears on the next successful read. Paths are confined to the
configured directory: `..`, absolute paths and symlinks that leave the root are
refused and reported the same way.

The edit loop:

1. edit `skills/<name>/SKILL.md` or a reference;
2. `spindle-mcp skills check` — seconds, no cargo: reports unknown tool markers,
   markers for tools the unit's manifest does not declare, prose mentions with no
   marker, and compiled skills or references whose file is missing. Problems
   carry `file:line: code: message`, and the exit code is 1 when anything is
   wrong, so it fits a pre-commit hook;
3. call `get_skill` / `get_reference` (or read the `bible://` resource) again;
4. `curl -s localhost:4321/health` shows the content mode and any fallback.

```bash
spindle-mcp skills check                       # skills check: 28 files OK
spindle-mcp skills check --json                # machine-readable problems
spindle-mcp skills check --skills-dir /tmp/skills --references-dir references
```

The command reads the same directories the server reads (`SPINDLE_SKILLS_DIR`,
`SPINDLE_REFERENCES_DIR`, defaults `skills` and `references`) and validates the
compiled unit inventory, so it can report a unit whose file is gone but can never
invent a unit that the build did not compile.

Rebuild when a `manifest.toml`, `profiles.toml`, reference declaration or tool
list changes, and keep the compiled fallback copy current: it is what ships when
no directory is configured.
