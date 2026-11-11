---
name: character-design
description: 麥基（McKee）角色設計的判斷與規劃：2–5 個矛盾、四層自我、自覺與不自覺慾望、對主角的壓力，以及把隱藏層轉成可演出的行為線索。
---

# 角色設計：把深度設計成可演出的東西

這個單元只管 **Character Design** 的創作判斷與它在 Cast & World 規劃層的落點。角色的聲音（voice profile）、情緒基線與即時狀態是另一組資料；角色設計不是角色小傳，也不是把台詞寫得更華麗。

## 何時載入

- 作者要為主要角色建立或檢查一份 Character Design。
- Cast & World 提案被 `character_design_missing` 或矛盾數量擋下，需要知道怎麼補。
- 作者要把「隱藏層／不自覺慾望」改寫成 writer 可以直接演出的行為線索。
- 作者要區分某個 arc milestone 是角色的 **change** 還是 **reveal**。

若專案不是透過 new-book 規劃流程建立的 McKee 專案，就不要替它補角色設計；既有專案沒有這個介面，也不會得到相關 finding。

## 這是什麼

一份 Character Design 屬於一個角色，包含五組東西：

1. **矛盾（contradictions）**：2–5 個彼此拉扯的傾向。不是缺點清單，也不是「善良又勇敢」這種同向疊加；要是「對陌生人慷慨／對家人苛刻」這種會在同一場戲裡互相拉扯的張力。
2. **四層自我**：`public_self`（給世界看的）、`personal_self`（給親近者看的）、`private_self`（只給自己看的）、`hidden_self`（連自己都不願承認的）。
3. **慾望**：`conscious_desire`（角色在頁面上說得出口、會去追的）與 `unconscious_desire`（角色真正需要、卻說不出口的）。兩者最好能互相妨礙。
4. **對主角的壓力（pressure_on_protagonist）**：這個角色讓主角必須付出什麼、選擇什麼。配角不是背景，是設計給主角的壓力來源。
5. **行為線索（behavior_cues）**：把 hidden 層與不自覺慾望翻譯成可觀察的動作、習慣、反射。這是 writer 唯一該拿到的「深層」材料。

**hidden_self 與 unconscious_desire 是作者層的祕密**：它們以既有的 circle-of-trust 祕密機制保存，只有規劃工具能讀到全文。其他讀取（角色快照、實體讀取、章節簡報、場景上下文）只會拿到去掉這兩層的 redacted 版本。因此行為線索不是裝飾，而是你唯一能讓 writer 演出深度的通道：不要寫「她其實很害怕」，要寫「她在開口前先數一次出口」。

## 規劃流程

1. 用 `planning_advance` 在 Cast & World 層先提出建立角色的 operation，再提出對應的 `create_character_design`。`character_id` 用同一個提案裡的 `proposal_ref` 指向剛建立的角色；既有角色可以用 `existing_record_id`。
2. 一份設計只能屬於一個角色，一個角色也只能有一份設計。重複的設計會被擋下；已接受的設計在這個階段視為凍結，不要用第二個提案覆寫。
3. 用 `planning_status` 讀回完整設計（含 hidden 與 unconscious 全文）與 findings。Cast & World 的 blocking finding 會讓提案可以暫存、但不能被接受。
4. 只有作者明確同意後，才用 `planning_decide` 接受。接受後才建立角色設計與兩個祕密事實。

## 阻擋與修復

McKee 專案的 Cast & World 接受前會檢查：

- 這個提案建立的每個角色都必須有 **恰好一份** Character Design（`character_design_missing`）。
- 每份設計的矛盾數量必須在 **2–5** 之間（`character_design_too_few_contradictions` / `character_design_too_many_contradictions`）；空白字串不算一個矛盾。
- 同一個角色出現兩份設計，或角色已有已接受的設計（`character_design_duplicate`）。

finding 的路徑會指到出問題的 operation 或其 `input/contradictions`；修復方式是改寫同一個 Cast & World 提案，而不是放寬檢查。非 McKee 專案完全沒有這些 finding；對非 McKee 專案提出角色設計則會在暫存階段被拒絕。

## 把隱藏層寫成行為線索

好的行為線索有三個特徵：

- **可被鏡頭拍到**：動作、停頓、物件、空間習慣，而不是內心形容詞。
- **與壓力有關**：在角色被逼到牆角時會出現，平時不一定。
- **不解釋自己**：線索不能順便說出原因；原因留在 hidden 層，讓 reader 自己拼。

檢查方式：把 hidden_self 遮住，只讀行為線索，讀者是否仍能感覺到「這個人有東西沒說」？如果遮住之後什麼都不剩，線索就只是標籤。

## Arc milestone：change 與 reveal

角色的 arc milestone 可以標記 `kind`：

- **change**：角色真正改變了（價值觀、選擇模式、關係位置）。這種變化必須有前置鋪陳與代價，不能在一場戲裡突然完成。
- **reveal**：讀者或角色得知了原本就存在的事實。reveal 改變的是「知道什麼」，不是「成為誰」。

兩者不可互換：把 planned change 只寫成 reveal，等於把成長偷渡成情報；把 reveal 當成 change，則是沒有付代價的轉彎。舊資料沒有 `kind` 時代表未分類，不要自行補值。

## 判斷準則

- 矛盾必須**互相拉扯**：如果兩個特質永遠不會在同一情境下衝突，它們不是矛盾，是形容詞。
- 四層要**逐層收窄**：public 是面具、personal 是日常、private 是獨處、hidden 是連自己都否認的。若四層內容可以互換，等於只有一層。
- 不自覺慾望要**與自覺慾望不同調**：兩個慾望一致時，角色沒有內在張力；把它們設計成「想要 A，卻需要 B」。
- 壓力要**具體到選擇**：寫出這個角色讓主角在什麼兩難裡選，而不是「帶來麻煩」。
- 行為線索要**少而準**：三到五個以內，寧可讓 writer 有空間，也不要用十個標籤把角色釘死。

## 界線

- 角色設計是規劃意圖，不是已發生的事件；不要把它寫成 canon fact，也不要宣稱某個改變「已經發生」。
- 不要建立角色代理人、Director、角色分工或訊息傳遞流程；這個單元只提供設計判斷與規劃落點。
- 不要把 hidden_self 或 unconscious_desire 直接貼進給 writer 的上下文；用行為線索。
- 不要跳過人類關卡；接受需要作者明確決定與精確 hash。

<!-- tool: planning_advance -->
<!-- tool: planning_status -->
<!-- tool: planning_decide -->
