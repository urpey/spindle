# Drafting packets

### McKee 填滿式草稿包（寫作簡報）

McKee 結構專案的 `prepare_scene` 會回傳填滿的 `drafting_packet_v1`，該 packet
就是整份寫作簡報，不需要再自行拼湊其他讀取。`scene_card` 由當前 planned Scene
填入：

- **value_shift**：主要 Value Axis，以及可讀的開場／收場 Charge（`positive`、
  `negative (contrary)`、`negative (contradictory)`、
  `negative (negation of the negation)`）。
- **opposition / gap / turning_beat / turn**：來自計畫的對抗、Gap、結構化
  `turn` Beat 與它的位置；`required_beats` 是全部結構化 Beats。
- **emotion**：`entry` 保留 POV 角色當下的真實狀態；McKee 計畫沒有情緒欄位，
  所以 `exit` 是誠實的 `unchanged (no planned emotion shift)`，不是把 Charge
  當成情緒使用。計畫的收場狀態看 `value_shift.after`。
- **chapter**：`planned_scene_count` 與 `is_closer` 來自章計畫，是章內位置的
  中繼資料，不是寫作約束。

`hard_constraints` 只包含本場到期的 **Key Fact**（kind `canon`）。Key Fact 的
canonical predicate 使用保留命名空間 `key_fact.<kind>:<作者 key>`，寫作面只顯示
作者 key。**秘密**的 Key Fact（例如 `revealed_secret`）只有在知情圈成員在場
（含 POV）時，才會以 `[SECRETS IN PLAY]` 信封出現：信封列出
`Known ONLY to` 名單、`Present and NOT in the know` 與 concealment note。若沒有
知情圈成員在場，整個值都不會被交付，`coverage.unavailable_passes` 會以
`due_key_facts` 明確回報保留了幾筆；不得自行補寫、暗示或假裝它已交付。

`promises_due` 只給承諾的性質（`promise_type` 與 `description`），永不給回收
位置、回收 Turning Beat、目標 Act，或可能編碼這些資訊的 notes。

`character_design_slice` 只含本場被測試的矛盾、`conscious_desire` 與行為線索；
`hidden_self`、`unconscious_desire`、它們的 secret fact id 與 concealment note
永遠不會進入 writer 讀取。

只讓看到該 packet 的子代理動筆（best effort，並非強制）。不要給它章計畫、其他
場景的計畫、上述隱藏層或任何回收位置。依計畫如實寫出轉折與到期事實，不得發明、
遺漏或改寫。沒有 value plan 的 Scene 會回報 `drafting_packet_v1` 不可用（原因在
`unavailable_passes`），不得以 placeholder 文字代替。

`chapter_plan.scenes` 只暴露當前 Scene，並以 `scene_scope.mode =
current_scene_only` 標記這次縮減是刻意的（非 McKee 專案沒有這個欄位）。
`novel.realized_scenes` 的每一筆是 `{scene_order, summary, realized_report?}`；
`realized_report` 就是該場儲存的 realized report（`value_turns` 的 axis、
opening／closing Charge、gap、turning_beat，`character_effects`、
`promise_events`、`key_fact_ids`）。沒有報告的場次會整個省略該欄位，
不是空物件。

### Packet rollout boundary

The typed `DraftingPacketV1` foundation is currently an internal preparation API,
not an active authoring-run mode or a new MCP tool. Keep using the shipped
save/commit continuity-package workflow. Future packet prose tasks will contain
their own compact rubric; do not append this skill, standards manuals or tool
instructions to a stateless packet completion. Do not claim `critique_v1` is
available until the authoring integration and evaluation gates ship.

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
