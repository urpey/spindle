---
name: value-turns
description: 麥基（McKee）價值轉折的判斷：Value Axis、Charge 的四階與正負、Value Turn、Gap，以及 Scene／Sequence／Act 三層的轉折邏輯與 Act 高潮 Charge、深度進展的檢查方式。
---

# 價值轉折：每一層都要真的轉動一條軸

這個單元只管「這一層有沒有把某條價值軸轉出可感知的幅度」。Scene 的寫法、對白戰術與段落節奏屬於 `scene-writer` 的 craft 參考；結構判斷留在這裡。若你正在修一個沒有轉折的 Scene，先確認它在 Sequence 與 Act 裡的位置，再改場景本身。

## 何時載入

- 作者要判斷某個 Scene、Sequence 或 Act 是否只是「有事情發生」。
- 作者要檢查 Act 高潮的 Charge 是否真的轉動 Spine Axis，或 Act 之間的深度有沒有推進。
- 作者要修復 broken value chain、沒有 Gap 的 Turning Beat，或 chapter 結尾沒有落在 Value Turn 上。
- 作者要確認 Act 高潮的 scene placeholder 有沒有被後續 scene batch 兌現。

## 四個基本詞

- **Value Axis（價值軸）**：一組可正可負的人類品質，例如信任／背叛、自由／奴役、真相／謊言。一條軸必須能雙向擺動；「正義」單獨不是軸，「正義／不義」才是。
- **Charge（極性）**：某條 Value Axis 在某個時刻的位置。它同時有 **sign**（positive／negative）與 **depth**（四階），不是抽象的「氣氛」。
- **Value Turn（價值轉折）**：一條 Value Axis 從 opening Charge 移動到 closing Charge。移動必須可感知；同一個 Charge 進、同一個 Charge 出，就是非事件。
- **Gap（鴻溝）**：角色預期的結果與世界實際給他的結果之間的差。Gap 是轉折的引擎；沒有 Gap 的場景，通常只能靠旁白宣布改變。

## Charge 的四階

depth 由淺到深固定為：

1. `positive`：價值成立。sign 只能是 `positive`。
2. `contrary`：價值受到挑戰、出現反面，但尚未被否定。
3. `contradictory`：價值被推到自己的反面，正面與反面直接衝突。
4. `negation_of_the_negation`：反面的反面；不是回到原點，而是經過否定後產生的新狀態。

sign 與 depth 必須成對：`positive` 只配 positive sign；其餘三階都是 negative sign。這個型別是封閉的，不要用「差不多 negative」或自創第五階。

深度不是分數，也不是越大越好看。它的用途是比較 **相鄰兩個轉折**：故事通常應該越來越深，但刻意停在同一階或退回較淺一階，也可以是設計。判斷時先問「作者要的是什麼效果」，再決定要不要提出警告。

## Scene 層的轉折邏輯

一個 planned Scene 至少要能回答：

- 這一場的 **primary Value Axis** 是哪一條？最多再加兩條 secondary axis。
- 每條軸的 opening Charge 與 closing Charge 分別是什麼？primary axis 的兩者必須不同。
- 這一場的 opposition 是誰或什麼？沒有對抗，就沒有壓力，也就很難有真正的 turn。
- 這一場的 Gap 是什麼？角色以為會發生什麼，實際得到什麼？
- 哪一個 Beat 是 **Turning Beat**？它是 Gap 發生的地方，也是 closing Charge 成立的瞬間。
- 角色的 **Inaction Cost** 是什麼？如果什麼都不做也能活下來，這一場的壓力不夠。

Value chain 的連續性：同一條軸上，這一場的 opening Charge 應該等於該軸上一次 closing Charge。若中間跳過一場、或 opening 與上一場 closing 不同，就是 value chain 斷裂。若同一條軸連續三場往同一個方向轉，通常會出現邊際效用遞減；這是 advisory，不是硬規則。

章節結尾：一章要算「落在 Value Turn 上」，得讓 primary axis 相對於**本章第一場的 opening Charge** 真的轉過去。若最後一場的 closing Charge 等於第一場的 opening Charge（整章 net-zero），或最後一場的 primary axis 根本沒轉，都是 advisory。真正要修的是最後一場的 closing Charge，而不是在章末加一句鉤子。

## Sequence 層的轉折邏輯

Sequence 是一組 Scene，它自己回答 Act 問題的一部分。判斷重點：

- Sequence 以最強的一場收束：`climax_scene` 必須是 `member_scenes` 的**最後一場**（story order）。若最後一場反而最弱，或高潮不是最後一個 member，Sequence 沒有收束。
- `inciting_scene` 不是選填：它必須存在，而且排在 climax 之前或同一場。
- 整個 Sequence 只屬於一條 plot line。不要讓一組 Scene 同時服務兩條主線而兩邊都沒推進。
- Subplot 的 Sequence 高潮應該落在或早於它的 Convergence Point；晚於匯流點，代表副線沒有把力量交給主線。
- Sequence 的 `climax_scene` 一定要有 dilemma（角色矛盾被測試）。它**不是**只有同時成為 Act climax 才需要：規劃閘門對任何 Sequence climax 都會擋下缺少 dilemma 的提案。

**Blocking（拒絕接受）**：缺 `sequence_key` 或 key 在同一 artifact 內重複；缺 plot line 或 question；`member_scenes` 為空、placement 座標不完整、或不是嚴格 story order；缺 `climax_scene`；climax 不是 member 或不是最後一個 member；缺 `inciting_scene` 或 inciting 晚於 climax；把 `sequences` 放在 Chapter 以外的層級（Series-Arc／Book／CastWorld／Contract 一律拒絕）。

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

### 檢查 Act 的順序

1. 先按 order 排序，確認 1..=n、沒有重複，而且每個 Act 都在 Series-Arc／Book 這一層。
2. 把每個 Act 的完整 Charge 寫出來，兩兩比較相鄰項；只在不同時才往下看 depth。
3. 把同一本書裡的高潮章號照 order 排好，確認嚴格遞增；跨書不比章號。
4. 把 depth 換成 rank，確認是否嚴格遞增；不是就寫 advisory，不要逕自改成 blocking。
5. 確認最高 order 的 Act 標記了 `answers_central_question`（其他 Act 都不標），且它的 `central_question_part` 真的回答了問題的一部分，而不是主題詞。
6. 確認每個 climax placement 的 chapter／scene 座標可被後續 scene batch 找到；placeholder 是保留位，不是願望。

Act breakpoints 不是另一個要作者填的欄位：McKee 專案的 act breakpoints 由 Act list 推導（`act_<order>`，值是該 Act 高潮章號除以同書最高的 Act 高潮章號；只有一個 Act 時為 1.0）。若看到有人另外填 act breakpoints，請他刪掉，讓結構與 pacing 用同一份資料。

## 常見錯誤

- 把「有衝突」當成「有轉折」：衝突可能只是重複同一個 Charge。
- 把 closing Charge 寫得比 opening 更極端，卻沒有 Gap 或 Turning Beat 支撐。
- 每個 Act 都停在 contradictory，depth 沒有再往 negation 走；這不是錯，但要有意識地選擇。
- 最終 Act 只寫 `answers_central_question=true`，`central_question_part` 仍是空話。
- 把 Act 高潮的 placeholder 往後挪，只為了讓目前的 scene batch 好寫；placeholder 是已接受的承諾，應該改 scene batch，而不是改 Act。
- 把 Act 當成 canon 事件。Act 是 outline intent，不是已發生的事；不要把它寫進 canon fact，也不要透過 canon delta 修改。要改 Act，重新走一次規劃提案與人類接受。

## 界線

- 這個單元不教散文、對白或節奏；那些留給 `scene-writer`。
- 不要替作者決定 Act 的數目，也不要把三幕結構當成預設答案。
- 不要用模型生成的文字填滿空白的 `central_question_part`；空白就讓它顯示為 blocking，回到作者討論。
- 既有的非 McKee 專案沒有這些欄位與檢查；不要替它們補 Act，也不要假裝它們有 McKee 行為。
