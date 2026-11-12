---
name: scene-design
description: 麥基（McKee）場景設計的判斷：Value Axis 的壓力、opposition、Scene Question、Gap、Turning Beat、Inaction Cost 與結構化 Beats 的設計順序與檢查清單。
---

# 場景設計：讓一場戲真的轉動一條價值軸

這個單元只管「這場戲要設計成什麼」，不寫散文，也不替角色決定他最後選哪邊。散文層的 scene/sequel、MRU 與語氣留給 `scene-writer` 的 craft 參考；結構判斷留在這裡。

## 何時載入

- 作者要為一個 planned Scene 填 primary Value Axis、opposition、Gap、Turning Beat、Inaction Cost 或結構化 Beats。
- 作者問「這場戲是不是只是有事情發生」，或要修一個被擋下的場。
- 作者要在既有 Sequence／Act 裡安排下一場戲，或判斷某一場該不該存在。
- 你要讀回已接受的 scene plan，核對它是否真的設計了一個轉折。

## 設計順序（不是寫作順序）

先決定意義，再決定拍。建議的順序：

1. **壓力**：這一場把哪一條 Value Axis 放在壓力下？最多再加兩條 secondary axis。
2. **反作用力**：誰或什麼會反對角色？反對者要有自己的慾望與手段，不是牆上的標語。
3. **Scene Question**：角色在這一場被逼著回答什麼問題？它是對角色的一次局部壓力測試，角色不需要在文本裡說出來。
4. **Beats**：角色用什麼策略、說了什麼、真正想要什麼；世界怎麼回。
5. **Gap**：角色預期會得到什麼，實際得到什麼。
6. **Turning Beat**：哪一個 Beat 的 Gap 讓 closing Charge 成立。
7. **Inaction Cost**：如果角色什麼都不做，他會失去什麼。
8. **出口**：這一場結束時，讀者被什麼未解決的壓力推向下一場。

設計順序是檢查用的；真正寫作時不需要照順序寫出來，也不需要把七個答案都塞進正文。

## 三禁預設

不要把結果寫進設計裡：

- 不預設這一場**必須發生什麼事**。
- 不預設價值**必須往哪個方向**轉。
- 不預設角色**必須完成什麼成長**。

Scene 的結束由實際發生的 value、gap 與 cause 決定。設計只負責把壓力架好，讓角色有空間做出會付出代價的選擇。

## Scene Question 怎麼長出來

好的 Scene Question 通常是「面對 X，他敢不敢／願不願意／如何選擇」。它由三樣東西長出來：

1. 上一場最重要的價值變化；
2. 那個變化造成的角色後果；
3. 到這一場為止仍然未解決的壓力。

Scene Question 不是主題句，也不是劇情摘要。它是一句角色會用行動回答的問題；答案可能是「他選擇不選」，那也是答案。

## Opposition：反作用力必須真的存在

沒有反對，就沒有壓力，也就很難有真正的 turn。Opposition 可以是：

- 另一個角色（有自己的目標，不必是反派）；
- 環境、制度、時間或物理限制；
- 角色自己的矛盾（Character Design 裡的兩條矛盾互相拉扯）。

判斷標準：這個 opposition 能不能**主動反擊**？如果它只會站著讓主角通過，這一場就沒有對抗。配角不必為了推進劇情進場，而是因為他自己的慾望與立場，對主角施加最有效的反作用力。

## 結構化 Beats

一個 Beat 是一次「用策略去拿東西」的嘗試，欄位如下：

| 欄位 | 寫什麼 |
| --- | --- |
| `kind` | `action`（角色行動）、`reaction`（對手或世界回應）、`turn`（轉折拍） |
| `actor` | 這一拍由誰執行；沒有明確執行者時可省略 |
| `tactic` | 用什麼手段去拿：詢問、交換、威脅、討好、迴避、沉默、試探 |
| `said` | 說出口的那一句；無聲的一拍用空字串或一行動作 |
| `wanted` | 角色**真正**想要的東西（潛台詞），不是這句台詞的表面意思 |
| `gap` | 這次嘗試的 expected 與 actual：他以為會拿到什麼，世界實際給了什麼 |
| `swain_label` | 可選的自由文字標籤（例如 goal、conflict、disaster）。只是標籤，不是結構 |

Beat 的邊界是**策略的改變**，不是句數。同一個策略連續幾句微動作算同一拍；換了手段才換拍。反應拍（`reaction`）寫的是世界或對手實際做了什麼，不是角色的內心解讀。

## Turning Beat

**Turning Beat 是 Gap 把 Value Axis 推過臨界點的那一拍**：它讓 closing Charge 成立。它不一定是情緒最高點，也不一定是最後一拍，但必須是「世界給了和預期不同的答案，價值因此位移」的那一刻。

- 一場戲**恰好一個** `turn` Beat；`turning_beat` 指向它在 `beats` 陣列裡的 1-based 位置（第一個 Beat 是 1）。
- 其餘的 Beat 是 `action` 或 `reaction`；不要用兩個 `turn` 假裝一場戲轉兩次。
- 轉折拍依賴某個資訊時，那個資訊必須先有 Narrative Promise 的鋪陳；沒有鋪陳的轉折是 deus ex machina。
- 如果寫不出轉折拍，通常不是「拍不夠多」，而是這條 Value Axis 在這一場根本沒有被真正施壓。

## Gap

Gap 是預期與實際之間的差。每一拍都可以有自己的小 Gap；Scene 層的 `gap` 則是整場轉折的那個大 Gap。

- Gap 越大、越不可逆，下一步的風險就越高。
- 角色如果輕易得到他想要的，就沒有 Gap；沒有 Gap，就沒有理由冒更大的險。
- Gap 不是「意外」；它是角色**根據自己的預期**行動後，世界給出的不同答案。

## Inaction Cost

如果角色什麼都不做也能活下來，這一場的壓力不夠。Inaction Cost 要具體：

- 他會失去誰、失去什麼證據、錯過哪個時間窗；
- 代價要隨著時間惡化，而不是停在原地；
- 「不行動」本身也可以是一個選擇，但它必須有代價。

## 收束與銜接

- closing Charge 是這一場留給下一場的狀態；它必須可以被下一場的 opening Charge 接上（同一條軸的 value chain）。
- 章節最後一場要讓 primary axis 相對於章節第一場真的轉過去；在章末加一句鉤子不算 Value Turn。
- 收束停在動作或對白上，不要用敘述者總結「這意味著什麼」。

## 檢查清單

- [ ] primary Value Axis 明確，且 opening 與 closing Charge 不同。
- [ ] opposition 有自己的慾望，能主動反擊。
- [ ] Scene Question 是由前一場的價值變化長出來的。
- [ ] 每一個 Beat 都有 tactic、said、wanted 與 gap。
- [ ] 恰好一個 `turn` Beat，`turning_beat` 指向它。
- [ ] 轉折拍依賴的資訊有 promise 鋪陳。
- [ ] Inaction Cost 具體且會惡化。
- [ ] closing Charge 接得上下一場。

## 常見錯誤

- 把 Beat 寫成劇情摘要（「他們爭吵」）而不是一次策略嘗試。
- 把 `reaction` 寫成角色的心理活動，而不是世界實際的回應。
- 為了讓場面刺激而加事件，卻沒有讓任何 Value Axis 位移。
- 用兩個以上的 `turn` Beat 補救一個不夠強的轉折。
- 把 `swain_label` 當成第二套結構分類，讓同一個 Beat 同時是 goal 又是 turn。

## 界線

- 不要把 planned Scene 寫成 canon fact；它是意圖，不是已發生的事件。
- 不要替角色決定他的選擇或成長；設計只提供壓力。
- 不要建立角色代理人、Director 或角色分工流程；這個單元只給結構判斷。
- 需要 Value Axis、Charge 四階或 value chain 的判斷時，載入 `story-structure/value-turns`；需要對白的戰術與潛台詞時，載入 `story-structure/dialogue-tactics`。
