---
name: value-turns
description: 麥基（McKee）價值轉折的判斷：Value Axis、Charge 的四階與正負、Value Turn、Gap，以及 Scene／Sequence／Act 三層的轉折邏輯與 Act 高潮 Charge、深度進展的檢查方式。
---

# 價值轉折：每一層都要真的轉動一條軸

這個單元只管「這一層有沒有把某條價值軸轉出可感知的幅度」。散文、對白戰術與段落節奏留給 `scene-writer`；若 Scene 沒有轉折，先確認它在 Sequence 與 Act 的位置，再改場景。要判斷 Scene／Sequence／Act 是否只是「有事情發生」、檢查 Act 高潮 Charge 與深度進展、修 broken value chain／沒有 Gap 的 Turning Beat／章末沒有 Value Turn，或確認 Act placeholder 有沒有被 scene batch 兌現時，載入本單元。

## 四個基本詞

- **Value Axis**：可正可負的人類品質，例如信任／背叛、自由／奴役。必須能雙向擺動；「正義」不是軸，「正義／不義」才是。
- **Charge**：某條軸在某刻的位置，同時有 **sign**（positive／negative）與 **depth**（四階）。
- **Value Turn**：一條軸從 opening 移到 closing Charge，移動必須可感知；同進同出就是非事件。
- **Gap**：角色預期與世界實際結果的差，是轉折的引擎；沒有 Gap 通常只能靠旁白宣布改變。

## Charge 的四階

depth 由淺到深固定為：

1. `positive`：價值成立。sign 只能是 `positive`。
2. `contrary`：價值受到挑戰、出現反面，但尚未被否定。
3. `contradictory`：價值被推到自己的反面，正面與反面直接衝突。
4. `negation_of_the_negation`：反面的反面；不是回到原點，而是經過否定後產生的新狀態。

sign 與 depth 必須成對：`positive` 只配 positive sign；其餘三階都是 negative sign。這個型別是封閉的，不要用「差不多 negative」或自創第五階。

深度不是分數：它用來比較相鄰轉折；通常越來越深，但刻意持平或退回也可以是設計。

## Scene 層的轉折邏輯

planned Scene 至少回答：primary Value Axis 是哪條（最多加兩條 secondary）；每條軸的 opening／closing Charge（primary 兩者必須不同）；opposition 是誰或什麼；Gap 是什麼；哪個 Beat 是 Turning Beat（Gap 發生、closing 成立之處）；Inaction Cost 是什麼。

Value chain：同一條軸上，這一場 opening 應等於該軸上一次 closing；跳過一場或對不上就是斷裂。同一條軸連續三場同方向轉，通常是邊際效用遞減的 advisory。

章節結尾：primary axis 要相對於**本章第一場 opening** 真的轉過去。最後一場 closing 等於第一場 opening（net-zero）或根本沒轉，都是 advisory；要修的是最後一場的 closing，不是在章末加鉤子。

## Sequence 層的轉折邏輯

Sequence 是一組 Scene，它自己回答 Act 問題的一部分。判斷重點：

- `climax_scene` 必須是 `member_scenes` 的**最後一場**（story order）；最後一場最弱或高潮不是最後一個 member，就沒有收束。
- `inciting_scene` 必填，且在 climax 之前或同一場。
- 一個 Sequence 只屬於一條 plot line；不要讓一組 Scene 同時服務兩條線。
- Subplot 的 Sequence 高潮要在它的 Convergence Point 或之前；晚於匯流點代表副線沒把力量交給主線。
- `climax_scene` 一定要有 dilemma（角色矛盾被測試）。不是只有同時成為 Act climax 才需要：任何 Sequence climax 缺 dilemma 都會被擋。

### Subplot 的結構檢查

Subplot 是 `is_main_plot=false` 的 plot line；它的 Sequence 要在 Convergence Point（匯流點，Subplot 把力量交給主線的位置）之前或同一場收束。

- **Blocking**：`subplot_sequence_climax_after_convergence_point`。Subplot 的 Sequence climax 晚於該 Subplot 的**每一個** Convergence Point 就擋下；剛好落在匯流點上算通過，章級匯流點接受同章任何一場。沒有匯流點時不套用這條。
- **Advisory**（不拒絕接受）：
  - `subplot_without_convergence_point`：合併後的 Subplot 沒有 Convergence Point。Series-Arc／Book 建立或更新 Subplot 時就會提示，不必等 Chapter batch。
  - `subplot_axis_dormant`：Subplot 的 Value Axis 連續三個被掃描的 Chapter 沒有轉折。中間缺章、某章真的轉了、或換書，run 都重新起算；一個 run 只在第三個未轉章節報一次。
  - `subplot_job_unsupported`：Subplot 至少轉過一次，但沒有任何一次收在 job 要的方向。`contradict` 需要負向收尾，`resonate` 需要正向收尾；`set_up`／`complicate` 不要求方向。
  - `subplot_deeper_than_main_plot`：同一 Act 內，Subplot 的最深 closing Charge 比主線在同一 Act 的最深轉折更深。Act N 的範圍是前一個有 climax 的 Act 之後、到本 Act climax 為止；缺 exact climax 的 Act 不建立邊界，最後一個 Act 之後的 Scene 不比。只有 merged Acts 的 climax 在全域 story order（書、章、場）嚴格遞增時才比較，否則整段略過，避免跨書順序錯亂時把某 Act 的轉折算到另一 Act。Subplot 與主線共用同一條 Value Axis 是合法的：兩邊讀到同一組轉折、深度相等，不會誤報。

**Blocking（拒絕接受）**：缺 `sequence_key` 或 key 在同一 artifact 內重複、或 key 不是 pointer-safe（非空、長度受限、只允許英數與 `-_:`，不可含空白、`/`、`~`）；refinement 改了同一 key 的 `plot_line_id`；缺 plot line 或 question；`member_scenes` 為空、placement 座標不完整、或不是嚴格 story order；缺 `climax_scene`；climax 不是 member 或不是最後一個 member；缺 `inciting_scene` 或 inciting 晚於 climax；把 `sequences` 放在 Chapter 以外的層級（Series-Arc／Book／CastWorld／Contract 一律拒絕）。

**Advisory（只警告）**：Sequence 內連續兩場的 risk 下降。這是節奏提醒，不阻擋接受。

## Act 層的轉折邏輯

Act 是 ordered list，數目由故事決定，不是固定三幕。每一個 Act 在規劃排程裡要有：

- `order`：從 1 開始的連續整數，唯一。
- `climax`：保留給高潮的 scene placeholder，必須有正數的 `book_number`、`chapter_number` 與 `scene_order`。只有章號沒有場號，無法在後續 scene batch 兌現。
- `charge`：這個 Act 高潮之後，Spine Axis 的 Charge（sign + depth）。
- `central_question_part`：這個 Act 回答了 Central Question 的哪一部分，不能空白。
- `answers_central_question`：只有最高 order 的最終 Act 可以、也必須標記為 true。

### 阻擋與警告

**Blocking（拒絕接受）**：

- 某個 Act 高潮的完整 Charge（sign 與 depth 都算）與前一個 Act 相同。第一個 Act 沒有比較對象，不檢查。
- 最高 order 的最終 Act 沒有標記 `answers_central_question`。
- 非最終的 Act 標記了 `answers_central_question=true`。只有最後一個 Act 能回答 Central Question。
- Act 的 order 不是 1..=n、重複、非正數；`central_question_part` 空白；缺 climax、缺 `scene_order`、或 placement 不是正數。
- 同一本書裡，後一個 Act 的高潮章號沒有比前一個 Act 大。跨書不比較章號，但仍要照 order 遞增；這條規則保證推導出的 act breakpoints 不會回頭。
- 把 `schedule.acts` 放在 Series-Arc／Book 以外的層級。Act 只屬於這兩個層級；Chapter／Scene 的 batch 只計畫場景。
- 後續 Chapter／Scene batch 計畫了含某個 Act 高潮的章節，卻沒有在該 `scene_order` 放一場 Scene。這裡的 Act 也包含同一批決策裡還沒接受、但已 staged 的父層 Act。

**Advisory（只警告）**：

- Act 之間的 depth 沒有嚴格變深。把 depth 換成 rank：positive = 0、contrary = 1、contradictory = 2、negation of the negation = 3；rank 序列若不是嚴格遞增，就提出警告。持平或退回都算，但作者可以刻意如此，因此不阻擋接受。

注意：如果兩個 Act 的 depth 相同，它們的完整 Charge 通常也相同（因為 positive 只能配 positive sign），這時 blocking 的「Charge 未改變」會先成立，advisory 只是補強說明。

### Act 的 cause

Act 可選擇性帶 `cause`：`{ "plot_line_id": ..., "sequence_key": ... }`，宣告高潮由某個 Subplot Sequence 的高潮促成。`sequence_key` 是**全域**合併識別：同一 key 出現在不同 Chapter 就是同一條 Sequence 的 refinement，因此 refinement 不得改 `plot_line_id`（`sequence_plot_line_changed`）；key 必須非空、長度受限、只允許英數與 `-_:`，不可含空白、`/`、`~`（`sequence_key_invalid`），因為 key 會出現在 locator。

接受 Series-Arc／Book 時，以及之後每次 Chapter／Scene staging，都會拿 merged Acts 重驗 cause：

- `act_cause_unresolved_subplot_sequence`：plot line 在 merged 集合裡解析不到（例如已 archive）、key 空白、沒有 Sequence 用這個 key、這個全域 key 屬於另一條 plot line（資料錯誤），或 Sequence 沒有 exact 高潮。
- `act_cause_not_a_subplot`：cause 指的 plot line 是 Main Plot。
- `act_cause_after_act_climax`：被引用的 Sequence 高潮晚於 Act 自己的高潮；cause 必須先於 effect。
- `act_cause_after_convergence_point`：被引用高潮晚於該 Subplot 的每一個 Convergence Point（沒有匯流點也算）。

已接受的 Act 用 `/accepted_acts/{order}/cause/...` 定位，staged 父層用 `/staged_acts/{order}/cause/...`，本 artifact 用 `/schedule/acts/{i}/cause/...`。參照本身解析不到（`proposal_ref`／`existing_record_id`）是 staging 的硬錯誤，不是 stage-recordable finding。

工作流程限制：cause 要引用已存在的 Chapter Sequence，forward pass 在 Chapter 之前就跑完 Series-Arc／Book，通常還不能寫 cause；作者要在 Chapter Sequence 被接受後**重新 stage 該層級**補上。已接受的 cause 也會被後續 Chapter／Scene refinement 重新驗證。

Act breakpoints 不是另一個要作者填的欄位：McKee 專案的 act breakpoints 由 Act list 推導（`act_<order>`，值是該 Act 高潮章號除以同書最高的 Act 高潮章號；只有一個 Act 時為 1.0）。若看到有人另外填 act breakpoints，請他刪掉，讓結構與 pacing 用同一份資料。

## 常見錯誤

- 把「有衝突」當「有轉折」：衝突可能只是重複同一個 Charge。
- closing 寫得更極端，卻沒有 Gap 或 Turning Beat 支撐。
- 每個 Act 都停在 contradictory，depth 沒往 negation 走；不是錯，但要有意識。
- 最終 Act 只寫 `answers_central_question=true`，`central_question_part` 仍是空話。
- 為了目前 scene batch 好寫而挪 Act 高潮 placeholder；placeholder 是已接受的承諾，該改 scene batch。
- 把 Act 當 canon 事件；Act 是 outline intent，要改就重走規劃提案與人類接受。

## 界線

- 不教散文、對白或節奏；那些留給 `scene-writer`。
- 不替作者決定 Act 數目，也不把三幕當預設。
- 不用模型文字填 `central_question_part`；空白就讓它 blocking，回到作者討論。
- 非 McKee 專案沒有這些欄位與檢查；不要替它們補 Act，也不要假裝它們有 McKee 行為。
