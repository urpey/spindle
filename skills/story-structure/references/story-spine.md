---
name: story-spine
description: Story Spine 的完整參考：Central Question、Controlling Idea、Counter-Idea、Spine Axis、ending type、Charge 四階、系列脊柱與書本 sub-spine 的判斷與範例。
---

# Story Spine 參考

## 一句話定義

Story Spine 是作品「為什麼值得被說完」的那條意義線。它把一個問題、一個主張、一個反主張與一條可翻轉的價值軸綁在一起，讓 Act、Sequence、Scene 的每一次高潮都能被檢查。

系列有一條 spine；每本書可以有一條 sub-spine。Sub-spine 不是複製系列 spine，而是把系列問題切出這本書要回答的那一部分。

## 五個必要欄位

### 1. Central Question

- 是問題，不是主題。例：「Ada 能不能在失去家園之前學會信任？」不是「信任」。
- inciting incident 必須把它提出來；final climax 必須回答它。
- 若問題的答案在故事開始前就已確定，這不是 Central Question，是背景設定。

### 2. Controlling Idea（value + cause）

- 一句斷言：「價值 X 因為原因 Y 而成立／毀滅。」
- `value` 是一個可正可負的價值詞，和 Spine Axis 對齊。
- `cause` 是故事必須戲劇化的因果，不是作者的道德感想。
- 範例：value=`信任`，cause=`Ada 把最後的證據交給曾經背叛她的人，而對方選擇回頭`。
- 反例：value=`愛`，cause=`因為愛很重要`。這是同義反覆，不是因果。

### 3. Counter-Idea

- 與 Controlling Idea 真正衝突的另一個世界觀。
- 它讓故事成為辯論，而不是單方面宣講。
- 檢查法：如果把 Counter-Idea 拿掉，故事是否只剩下「好人堅持到底」？若是，它太弱。
- Counter-Idea 可以由反派、Subplot 或主角的舊信念承載；不必由單一角色代言。

### 4. Spine Axis

- 一個可從正面走到負面的人類品質，例如信任／背叛、自由／奴役、清醒／幻覺。
- 它是 Act climax 的共用尺規。每個 Act 的高潮都要沿著這條軸轉動。
- 若一個 Act 的高潮只推動支線價值，卻沒有推動 Spine Axis，這個 Act 還沒有完成它的結構工作。

### 5. Ending type

| 類型 | 最終高潮的價值方向 | 讀者感受 |
| --- | --- | --- |
| idealist | 正面價值勝出 | 世界比我們以為的更有希望 |
| pessimist | 負面價值勝出 | 世界比我們以為的更冷 |
| ironic | 得與失同時成立 | 帶著矛盾與餘味離場 |

Ending type 不是結局心情，而是對最終 Charge 的設計承諾。選了 ironic，卻在最後一場讓一切圓滿無損，就是結構與標籤不一致。

## Charge：四階與符號

Charge 是某條 Value Axis 在某一刻的位置：

1. **positive**：價值成立。sign=`positive`，depth=`positive`。
2. **contrary**：價值受到挑戰但尚未反轉。sign=`negative`，depth=`contrary`。
3. **contradictory**：價值被相反值取代。sign=`negative`，depth=`contradictory`。
4. **negation of the negation**：連相反值也被否定，落入更深的虛無或新層次。sign=`negative`，depth=`negation_of_the_negation`。

封閉型別只允許兩種組合：正號只能配 positive；其餘三階都必須配 negative。`positive + contrary`、`negative + positive` 都是無效 Charge。

Scene 的 Value Turn 是 opening Charge → closing Charge 的可感知移動；同一個 Charge 開閉就是非事件。Act climax 則要讓 Spine Axis 走到比先前更深的 Charge，或至少明確轉向。

## 系列脊柱與書本 sub-spine

系列脊柱：

```json
{
  "central_question": "Ada 能不能在失去家園之前學會信任？",
  "controlling_idea": {
    "value": "信任",
    "cause": "Ada 把最後的證據交給曾經背叛她的人，而對方選擇回頭"
  },
  "counter_idea": "信任只是另一種交出控制權的方式",
  "spine_axis": "信任",
  "ending_type": "ironic",
  "inciting_incident": {"book_number": 1, "chapter_number": 1, "scene_order": 1},
  "final_climax": {"book_number": 3, "chapter_number": 12, "scene_order": 2}
}
```

書本 sub-spine 加上 `series_spine_id` 與 `book_number`，其 Central Question 應該只問這本書負責的那一段。例如系列問「Ada 能不能學會信任」，第二集 sub-spine 只問「Ada 能不能信任一個說謊的盟友？」。不要讓每本書都回答整個系列的問題。

## 建立與審查流程

1. 先確認專案是 new-book 建立的 McKee 專案；既有專案沒有標記，不能補。
2. 在 Contract 層 stage `create_story_spine`，一次只建一條：先系列，再逐書 sub-spine。
3. 讀 findings。四個必要欄位空白時是 Blocking；不要用「之後再補」跳過。
4. 向作者展示 spine 摘要與尚未解決的創作問題。
5. 作者明確接受後才 persist。第二條系列脊柱會被拒絕；同一本書的第二條 sub-spine 也應被拒絕。
6. 用 `planning_status.story_spines` 與 entity read 核對讀回結果。

## 常見壞味道

- Central Question 其實是主題詞，沒有可回答的「是／否」。
- Controlling Idea 只有 value，沒有 cause；或 cause 只是價值重述。
- Counter-Idea 只是反派的壞脾氣，沒有自己的道理。
- Spine Axis 是一組價值，而不是一條軸；Act 之間各轉各的。
- 每本書的 sub-spine 都重答系列問題，導致系列沒有推進。
- ending type 與 final climax 的 Charge 不一致。
- 把 Theme 當成脊柱，或建立脊柱後刪掉 Theme。Theme 仍可獨立存在。

## 判斷練習

看到一個 spine 時，依序問：

1. 這句話是在問問題，還是在給答案？
2. 這個答案有沒有「因為」？
3. 反方主張能不能讓讀者暫時相信？
4. 每個 Act 的高潮是否都能量在這條軸上？
5. 最終 Charge 是否兑現 ending type？
6. 這本書的 sub-spine 是否只回答系列問題的一部分？

只要其中一題說不清楚，先回到作者討論，不要用生成文字把空白填滿。

<!-- tool: planning_status -->
