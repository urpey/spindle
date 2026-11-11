---
name: story-structure
description: 麥基（McKee）故事結構的核心工作流：先定故事脊柱，再用 Value Axis、Charge、Value Turn、Gap 與 Act／Sequence／Scene 層級檢查規劃。用於新書契約、系列／書本脊柱、結構審查與接續已存在的 McKee 專案。
---

# 故事結構：先定意義，再寫場景

這個單元只處理結構判斷與規劃動作。散文層的 scene/sequel、MRU、語氣與對白質感，留給 `scene-writer` 的 craft 參考；兩者不應互相取代。

## 何時載入

- 作者要為新書建立 Central Question、Controlling Idea 或系列脊柱。
- 作者要檢查一本書的 Act 是否真的轉動 Spine Axis，或某個 Scene 是否只是「有事情發生」。
- 作者要建立書本 sub-spine，讓每一集回答系列問題的一部分。
- 你要讀回已接受的脊柱，或判斷一個缺失欄位為何阻擋接受。

若專案不是透過 new-book 規劃流程建立，就沒有 McKee 標記；不要替它補脊柱、不要切換模式，也不要假裝它具備 McKee 行為。

## 四層鏡頭

1. **Story Spine（故事脊柱）**：整個系列的一句話意義。它由 Central Question、Controlling Idea（value + cause）、Counter-Idea、Spine Axis 與 ending type 組成。
2. **Act（幕）**：一串 Sequence，最後以一個 climax 轉動 Spine Axis。幕的數目由故事決定，不是固定三幕。
3. **Sequence（段落）**：Act 內的一組 Scene，最後一場比先前任何一場更有衝擊，回答 Act 問題的一部分。
4. **Scene（場景）**：在近似連續的時間與地點中，透過衝突行動，至少把一個 Value Axis 轉動一個可感知的幅度。什麼都沒轉的 Scene 是非事件。

Series 只有一個 spine；每本書可以有自己的一個 sub-spine，回答系列問題的一部分，而不是假裝回答全部。Theme 是次要想法，仍保持獨立：脊柱可以引用 Theme，但不能取代 Theme 清單。

## 價值轉折與 Act 清單

Series-Arc 與 Book 層的排程可以帶一份 ordered **Act list**（`schedule.acts`）：每個 Act 有 order、climax scene placeholder、高潮後的 Spine Axis Charge、它回答的 Central Question 部分，以及只有最終 Act 才為 true 的 `answers_central_question`。Act 之間只比完整 Charge（sign + depth）；相鄰兩個 Act 的 Charge 相同，或最終 Act 沒有標記回答問題，都會擋下接受。depth 沒有嚴格變深只是 advisory（positive < contrary < contradictory < negation of the negation），作者可以刻意選擇較淺的進展。

已接受 Act 的 climax scene 是保留位：之後計畫該章節的 scene batch 必須在該 `scene_order` 放一場 Scene，否則接受會被擋下。Act breakpoints 由 Act list 推導，不要另外填。

要判斷 Value Axis、Charge 四階、Gap、broken value chain，或某一層到底有沒有轉折，載入 `story-structure/value-turns`；這裡只留結構位置與閘門。

## 角色設計（Character Design）

Cast & World 層為每個角色建立一份 Character Design：2–5 個會互相拉扯的矛盾、四層自我（public / personal / private / hidden）、自覺與不自覺慾望、對主角的壓力，以及可演出的行為線索。隱藏層與不自覺慾望是作者層祕密，只有規劃讀取看得到全文；writer 只拿到行為線索。要設計、審查或修復 `character_design_missing` 之前，載入 `story-structure/character-design`。

## 主線與副線（Main Plot / Subplot）

Series-Arc 層同時決定每條 plot line 的結構角色。McKee 專案在 Series-Arc 與 Book 接受時會檢查：

- 全專案恰有一條 **Main Plot**（`is_main_plot`），它的 Value Axis 必須等於已接受系列脊柱的 Spine Axis（只忽略前後空白）。
- 其他每條都是 **Subplot**：阻擋條件是剛好一個結構工作——`contradict`、`resonate`、`set_up` 或 `complicate`。Value Axis 仍是必要的敘事指引，但缺 Value Axis 不會單獨擋下接受。
- Main Plot 不是 Subplot，不帶 subplot job。建立或更新 plot line 時，`is_main_plot=true` 再加上 subplot job 會被直接拒絕；規劃器則記下 `plot_line_subplot_job_on_main`。
- 尚未接受系列脊柱時，先不比對 Spine Axis；缺脊柱不是這一步的 finding。

V0058 之前的既有 plot line 不會自動豁免：接受前要補上 `value_axis`、`subplot_job`、`is_main_plot`，否則上面的檢查會擋下接受。

這些欄位是規劃意圖，不是已發生的事件；不要把它們寫成 canon fact。既有非 McKee 專案不需要、也不會得到這些 finding。

## 在契約層建立脊柱

McKee 專案建立後，脊柱透過規劃操作進入人類審查，不會由模型直接寫入。

1. 用 `planning_begin` 開始或接續 session。若作者已有專案，先確認它是透過 new-book 建立的 McKee 專案。
2. 用 `planning_advance` 產生或提交 Contract 層的 `create_story_spine` 操作。系列脊柱不帶 `book_number`；書本 sub-spine 必須帶正整數 `book_number`，並用 `series_spine_id` 指向系列脊柱。
3. 用 `planning_status` 讀回暫存提案、findings 與 `story_spines`。若四個必要欄位有任何一個空白，會出現 Blocking finding；先修復再談接受。
4. 只有在作者明確同意後，才用 `planning_decide` 接受該提案。接受的內容會持久化；同一專案的第二條系列脊柱會被拒絕。

不要在一次 Contract 提案裡同時建立專案與脊柱。先接受專案 root，再單獨審查脊柱。既有專案不能事後補上 McKee 標記。

## 讀回與可見性

- `planning_status` 回傳已接受的 `story_spines`，可用來核對系列脊柱與各書 sub-spine。
- `get_entity` 可用 `table: "story_spine"` 讀取單一脊柱；只要 profile 提供 `get_entity` 就能讀，不需要 planning 工具。
- 若作者只需要創作方向，回傳 Central Question、Controlling Idea、Counter-Idea、Spine Axis 與 ending type 的摘要，不要長篇轉貼整個規劃 artifact。

## 判斷準則

- **Central Question**：一句真正的問題，inciting incident 提出，final climax 回答。不是主題詞，也不是 logline。
- **Controlling Idea**：一句斷言，格式是「價值因為某個原因而成立或毀滅」。`value` 與 `cause` 都必須是具體的；只有價值沒有原因，等於還沒有思想。
- **Counter-Idea**：與 Controlling Idea 真正對立、值得故事與之辯論的主張。它不是反派台詞，也不是比較弱的同一立場。
- **Spine Axis**：一個可從正到負擺動的人類品質，例如信任／背叛。全系列所有 Act climax 都應該轉動這一條軸。
- **Ending type**：idealist 讓正面價值在最終高潮勝出；pessimist 讓負面價值勝出；ironic 同時保留得與失，讓讀者帶著矛盾離場。ending type 是設計承諾，不是事後標籤。
- **Inciting incident / final climax**：兩者都是 Story Placement，必須有正的 book、chapter、scene 座標；final climax 不得早於 inciting incident。

## 阻擋與警告

Blocking：缺 Central Question、缺 Controlling Idea `value`、缺 Controlling Idea `cause`、缺 Spine Axis；重複的系列脊柱；sub-spine 沒有書號或指到另一條 sub-spine；非 McKee 專案要求脊柱。

Advisory 與創作品質有關，不冒充硬規則：脊柱是否只是主題口號、Counter-Idea 是否真的可辯、ending type 與最終 Charge 是否一致。這些要用作者判斷，不要自行改成 blocking。

## 界線

- 脊柱是規劃意圖，不是已發生的事件；不要把它寫成 canon fact，也不要說某個 Act 已經「發生」。
- 不要跳過人類關卡。`planning_decide` 的接受必須有作者明確決定與精確 hash。
- 不要建立角色代理人、Director 或角色分工流程；這個 skill 只提供結構判斷。
- 需要更細的條目、Charge 四階與書本 sub-spine 範例時，載入 `story-structure/story-spine` 參考。

<!-- tool: get_entity -->
<!-- tool: planning_advance -->
<!-- tool: planning_begin -->
<!-- tool: planning_decide -->
<!-- tool: planning_status -->
