# Startup tool profiles

Select a profile before starting Spindle, then reconnect the MCP client:

```sh
SPINDLE_TOOL_PROFILE=research spindle-mcp
spindle-mcp profiles
spindle-mcp profiles --json
```

CLI help lists every startup name. The profile catalog includes enabled units
and their complete required tool names without opening a project database.
Unknown names now fail before workspace/database access in both stdio and HTTP
modes. Directly constructed unknown-profile routers list nothing and deny calls.
The internal manifest label `default` means an unset environment variable; it is
not a startup profile name.

Unset, `minimal`, `write`, `import`, and `authoring` keep their compatibility
catalogs while the canonical `commit_scene` facade is added to the scene-writing
profiles. Explicit `legacy-all` gives the same complete catalog as unset. New
generated profiles are opt-in:

| Profile | Enabled work | Tools | Input bytes | Full bytes |
| --- | --- | ---: | ---: | ---: |
| workflow-write | Scene-writer core + craft/packet/research refs + setup | 44 | 59,354 | 115,504 |
| workflow-authoring | Scene-writer (all refs) + authoring-supervisor + setup | 74 | 88,991 | 185,874 |
| workflow-import | Manuscript importer and source sync | 20 | 13,083 | 31,979 |
| revision | Branch/revision/history workflow | 34 | 23,027 | 59,237 |
| worldbuilding | Worldbuilder + character creator | 50 | 51,202 | 87,146 |
| planning | Plot architect | 38 | 36,237 | 67,890 |
| new-book | New-book entry: planning sessions, human gates and handoff | 15 | 21,208 | 27,875 |
| mckee | Story-structure core + Story Spine reference | 11 | 18,462 | 22,860 |
| research | Researcher | 15 | 7,584 | 20,247 |
| style | Editor core + style reference | 43 | 34,677 | 95,438 |
| release | Editor + bible librarian + canon steward | 62 | 49,647 | 95,874 |
| setup | New-book setup reference | 9 | 4,180 | 9,270 |

Every generated profile includes the shared entry core (`spindle`, 6 entry tools).
Large context/analysis output schemas load optionally through versioned
[response contracts](tool-response-contracts.md); actual results are unchanged.
Their measured byte ceilings are explicit in `tools/profile-budgets.json`;
canonical target caps remain unmet until domain consolidation and compatibility
cleanup complete; `commit_scene` is the first
receipts-inline canonical write in these profiles. `get_skill` and `get_reference` check active profile coverage and
return structured coverage metadata (`covered`, `required_profile`,
`available_units`, `missing_capabilities`) without mutating the active profile.
Generated import has no style dependency; style is a separately selected profile.
Operator-only commands (`configure_agents`, `init_grok_skills`, `test_agent`,
`rebuild_search_index`, `backfill_scene_source_offsets`, and
`operator_delete_scene`) are omitted from every model profile, including unset
and `legacy-all`, and denied by name on stdio and HTTP model transports. Use the
CLI commands in the [release R migration guide](tool-migration-r.md). The
authoring harness reaches `test_agent` only through a server-issued internal
token; an external caller cannot. This is discovery/invocation filtering, not
operator authentication.

`skills/profiles.toml` composes skill and named-reference units. The build resolves
transitive skill dependencies, includes each reference's owning core, rejects
unknown units, duplicate definitions/units, cycles and reserved profile names,
and emits sorted/deduplicated static allowlists. Registry/operator checks remain
in MCP alignment tests, preserving the packaging crate's boundaries. Generated
skill/reference metadata reports enabled profile membership. Reading a skill
never adds tools to a profile; only the enabled units listed by the catalog are
supported workflows until loader coverage messages ship.

`tools/generated-profiles.json` is a checked compiler-output snapshot, not a
second editable composition file. Regenerate it and embedded skill metadata with
`SPINDLE_UPDATE_SKILL_SNAPSHOT=1 cargo test -p spindle-mcp metadata_matches_snapshot`.
Review new profile `tools/list` snapshots and measured caps in the same PR.
Existing five catalog snapshots stay unchanged; the direct unknown-router
snapshot intentionally becomes empty because startup typos no longer fall back.

StdIO, HTTP and secondary proxies use the same profile gate. A secondary filters
the primary's discovered catalog and denies out-of-profile calls before forwarding
(the primary may further restrict availability). If a shared primary uses a
narrower profile, reconnect to a primary with the needed capabilities; selecting a
secondary profile cannot expand its primary. Tests exercise real HTTP sessions
and a secondary over a full primary, including a denied mutation with no database
change. The isolated live probe checks every profile, CLI metadata/help, and
unknown-name rejection before data access. The inherited stalled-child test
observes the real child/workspace from the parent, avoiding child-marker races.

The HTTP operator console is the one exception: it talks to `/console/mcp`,
which always exposes the fixed console tool set (`CONSOLE_TOOLS` in
`crates/spindle-mcp/src/tools/profiles.rs`) regardless of `SPINDLE_TOOL_PROFILE`.
That internal `console` profile is not selectable at startup. Model clients on
`/mcp` see only their configured profile.
