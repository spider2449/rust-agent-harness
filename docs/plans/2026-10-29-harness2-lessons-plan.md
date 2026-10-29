# 向 harness2 學習的項目

2026-10-29。研究 `https://github.com/google-research/harness2`（HEAD `1cfe254e8d`，2026-10-13，論文 arXiv 2610.14224）後整理出的 7 項。依下列順序做，每項一個 commit；動到 `rah-api`、`rah-protocol` 或 plugin 協定的項目先寫 ADR（先讀 ADR 0036）。

harness2 不是 agent harness，而是**自動優化 harness 的研究框架**（Python）：模型權重固定，以 OpenCode 或 Codex 為執行器，反覆修改執行器周邊的 8 個元件（system prompt、guardrails、memory、tool guidance、subagents、skills、scripts、plugins）。任務層用 Propose–Probe–Compose（提出修改、在同一任務上各跑一次、依軌跡差異合成），不需要標準答案；跨任務層把採用的 harness 與教訓存成依領域分類的庫。

對 RAH 的價值有兩種：一是讓 RAH 可以當它的執行器（第 1、2、7 項），從而第一次能用 benchmark 量化 RAH 的 preset；二是它的 8 元件中 RAH 缺少的部分（第 3 到 6 項）。

不學：自動優化循環本身（每任務 4k 次執行加 3k 次改進，研究性質，不屬於 harness 核心）、benchmark 防洩題檢查（grader verdict 掃描、answer-key 路徑）、預算恆等式帳本、OpenCode 的 TypeScript plugin 形式（RAH 是 static Rust、process、WASM）。

## 對照

| harness2 元件 | RAH 現況 |
| --- | --- |
| c1 system prompt | `context-default` 的 `system`，只能寫在設定裡 |
| c6 guardrails（常駐規則） | 無；policy 是權限，不是給模型的文字 |
| c3 memory | `context-default` 的 `memory_dir` / `memory_file`、`memory.remember {note}` |
| c4 tool guidance | 無；工具說明寫死在各 plugin |
| c5 subagents | `scheduler-basic` 的 `agent.delegate {task}`，只有一種通用 subagent |
| c6 skills | `skills-fs`，啟動前不檢查內容 |
| c7 scripts | 不適用；skills 只是文字，可執行能力由 shell 工具與 policy 控制 |
| c8 plugins | RAH 本身就是 plugin 架構 |
| 執行器介面：無互動執行 | `--prompt`、`--config`、`--data-dir`、`--root`、`--approve` |
| 執行器介面：讀回軌跡 | session-jsonl；只有 Web UI 能匯出（`/api/sessions/{id}/export?format=md|json`），CLI 不能 |
| 執行器介面：花費 | `ModelEvent::Usage` 只有 token 數，沒有一次執行的彙總 |
| 執行器介面：preflight | `--check-config`（ADR 0059） |

## 1. CLI 匯出 session 與軌跡格式

- `frontend-cli` 新增匯出：`--export <session-id>` 加 `--format md|json|trace`，輸出到 stdout，不啟動模型。
- `trace` 是 harness2 共用的軌跡文法：每個工具呼叫一行 `[ACTION] <tool> <args>`，結果是 `[OBSERVATION]`（錯誤為 `[ERROR]`），助手文字原樣。引數與結果有長度上限（可設定，預設比照 harness2：引數 400 字元、結果 50 000 字元），被截斷處標明省略的字元數。
- 目前 md/json 的轉換在 `frontend-web` 的 `export` 模組裡，plugin 之間不能互相依賴。做法二選一，實作時決定：把轉換移到 `rah-api` 當共用的純函式（不是服務契約，不需 ADR，但要確認架構測試允許），或在 `frontend-cli` 另寫一份。偏好前者，避免兩份格式漂移。
- `uncertain` 的工具結果要在軌跡中標明，不能看起來像成功。
- 測試：用 `model-mock` 跑一輪含工具呼叫與錯誤的 session，比對三種格式的輸出。

## 2. 無互動執行的摘要

- `--prompt` 加 `--summary <file>`：結束時寫一份 JSON，內容為 session id、是否正常結束、錯誤訊息、執行時間、模型請求次數、token 彙總（`ModelUsage` 各欄位加總，provider 沒回報的欄位保持未知，不當成 0）、工具呼叫次數與其中失敗、拒絕、`uncertain` 的數量。
- 對應 harness2 的 `RunView`（ok、delivered、cost、wall time、error）。不算金額：RAH 沒有價格表，價格隨 provider 變動，交給呼叫端用 token 換算。
- 退出碼維持現狀；摘要是附加資訊，不改變既有行為。
- 這一項只彙總 bus 上已有的事件（ADR 0040），不新增事件型別。
- 測試：mock model 回報部分 usage 時，未回報的欄位在摘要中為 `null`。

## 3. 專案指令檔與檔案形式的 system prompt（需 ADR）

- 新 plugin `context-instructions`（`model.before_request` hook），不修改 `context-default`（「Replace, don't patch」）。每次請求從 `sandbox` 服務的工作區根目錄讀指令檔，預設名稱清單 `["AGENTS.md"]`（可設定，例如加 `CLAUDE.md`），以一個 system item 放在 system prompt 之後。
- 只讀工作區根目錄，不往上層目錄找（往上找會讀到使用者沒預期的檔案）；有大小上限（預設 32 KiB），超過時截斷並註明。
- 同一 plugin 提供 `files` 設定：使用者指定的額外文字檔（例如 guardrails、工具使用說明），路徑相對於資料目錄。這同時涵蓋 harness2 的 c4 tool guidance 與 c6 guardrails，不另做工具說明覆寫機制。
- `context-default` 加 `system_file`（與 `system` 互斥），讓整份 harness 可以是一個可版本控制的目錄。這是小型設定擴充，與上面同一份 ADR。
- 信任：工作區的指令檔可能來自不可信的 repo。它只是給模型的文字，不授予任何權限，與 skills 相同；在 `docs/SECURITY.md` 註明它可能是 prompt injection 的來源，且 policy 照常生效。
- 加入 `default` preset 與否在 ADR 中決定；偏好以 feature（`--with instructions`）提供，預設不掛載。
- 測試：切換 sandbox 根目錄後讀到不同檔案；檔案不存在時不產生 system item；超過上限時截斷。

## 4. Skills 啟動前檢查

- `skills-fs` 實作 ADR 0059 的設定檢查：front matter 無法解析、名稱重複、名稱與目錄不符、description 為空、數量超過 `max_skills`、單一 skill 超過大小上限。
- 每個問題一行，寫明檔案路徑與修正方式（harness2 的做法：錯誤訊息要能直接拿來修）。
- 執行時的行為不變：目錄仍在每次請求時重讀，壞掉的 skill 照現狀處理。
- 測試：每種錯誤一個案例，`--check-config` 回報全部問題且退出碼為 2。

## 5. 具名 subagent（需 ADR）

- `scheduler-basic` 支援具名定義：`agents_dir`（預設 `.rah/agents`），每個 `<name>.md` 有 front matter（`name`、`description`、`tools`）與 system prompt 本文；`agent.delegate` 加選填的 `agent`。
- 委派清單（name 與 description）寫進 `agent.delegate` 的工具說明，讓模型知道有哪些可用。
- `tools` 只能**縮小**子 session 看得到的工具，不能放寬；子 session 的每個工具呼叫照常經過 `ToolInvoker` 與 policy。定義檔不授予權限。
- 待調查：目前沒有「依 session 限制工具清單」的機制，可能要新增 hook 或擴充服務契約，這部分決定 ADR 的範圍。若需要改 `rah-api` 契約，先讀 ADR 0036。
- 測試：子 session 呼叫清單外的工具被拒；未知的 agent 名稱回報錯誤並列出可用名稱；`max_depth` 仍生效。

## 6. 結構化記憶（先評估）

- harness2 要求每條教訓有觸發情境、教訓本身與佐證（cue、lesson、evidence），缺一就拒收，避免記憶塞滿沒有根據的推測。
- 評估 `memory.remember` 加選填的 `evidence`，或在工具說明中要求模型寫出佐證。不改既有 `{note}` 輸入（向後相容）。
- 結果寫回本節；若不做，記下原因。

## 7. harness2 執行器指南

- 新增 `docs/HARNESS2.md`：說明如何以 RAH 作為 harness2 的執行器，包含 8 元件對應到 RAH 的哪個設定或檔案、每次執行的指令（`--config`、`--data-dir`、`--root`、`--prompt`、`--summary`、`--export --format trace`）、以及 preflight 用 `--check-config`。
- 配接器本身是 Python，屬於 harness2 那一側，不放進本 workspace；文件只列介面對應與一個最小範例。
- 安全：harness2 預設執行器網路不受限、Codex 關閉沙箱。文件要求搭配 `sandbox-os` 與 `default` preset 的 policy，並說明 process 監督不等於 OS 沙箱、RAH 不提供網路隔離。
- 依賴第 1、2 項完成；第 3、5 項完成後補上對應。

## 結果（2026-10-29）

1. 完成。旗標改為 `--session <id> --export md|json|trace`（沿用既有的 `--session`，不另設 `--format`）；不跑任何一輪，但仍照設定啟動 plugin。三種格式移到 `rah_api::session::export`（純函式，不是服務契約；架構測試不受影響），`frontend-web` 改用它，Web API 也接受 `format=trace`。軌跡另加 `[USER]` 與 `[OBSERVATION ERROR]`（與 harness2 的 OpenCode 轉換相同），沒有結果的呼叫顯示為 `[OBSERVATION ERROR]` 加上 `UNRECORDED_RESULT`。上限在 `TraceLimits`，CLI 用預設值（400／50 000 字元），沒有開放旗標。Web UI 沒有加 trace 按鈕（給工具用，不是給人看）。
2. 完成。`outcome` 為 `completed`、`failed`、`cancelled` 或 `stopped`。工具統計取自這一輪的 session 紀錄（`calls`、`errors`、`uncertain`、`unrecorded`）。**沒有單獨的「拒絕」數**：被拒的呼叫在紀錄中是 `is_error` 加 effect `none`，與其他不執行的錯誤分不開，要分開得在 session 紀錄加欄位（協定變更），不值得。subagent 的呼叫在它自己的 session，不計入。
3. 完成，ADR 0061。`names` 只取第一個存在的檔（避免 `AGENTS.md` 與 `CLAUDE.md` 內容重複時送兩次），只接受檔名。`files` 缺檔時以 `HostNotice` 告知一次。`system_file` 在 plugin 啟動時讀一次，讀不到則掛載失敗。`rah` 的資料路徑解析擴充為可解析路徑清單（`files`）。以 feature `instructions` 提供，preset 預設不掛載。
4. 完成，但**改為啟動與讀取時的 `HostNotice`（ADR 0057），不是 `--check-config`**。原因：`check_config` 同時在掛載時執行，若內容有問題就讓檢查失敗，一個壞掉的 `SKILL.md`（模型也可能寫出來）會讓整個 plugin 無法掛載，與「執行時行為不變」矛盾；而且目錄在每次請求都重讀，啟動前的檢查本來就不完整。現在啟動時掃一次，之後每次讀取發現新問題時各告知一次。另外改為依目錄名排序讀取，同名時由誰勝出不再取決於檔案系統順序。
5. 部分完成，**不含 `tools` 限制**。具名 subagent 已做：`agents_dir`（`rah` 預設為資料目錄下的 `agents`），啟動時讀取，清單與 `enum` 寫進 `agent.delegate` 的說明與 schema，未知名稱回報可用名稱，檔案問題以 `HostNotice` 告知。指示放在子 session 的第一則使用者訊息（指示在前、任務在後），因為沒有依 session 設定 system prompt 的機制。
   `tools` 縮小工具清單需要架構變更，未做：`ModelRequest` 沒有 session，`model.before_request` 無法依 session 過濾工具清單；在 `tool.before_invoke` 加處理器可以拒絕呼叫，但它會讓 `is_guarded()` 成立，`minimal` preset 的 UNGUARDED 警告就會消失（安全退步），而且模型仍會看到所有工具。可行做法是在 `TurnRequest` 與 `Invocation` 加上選填的工具允許清單，由 loop 過濾定義、由 `ToolInvoker` 強制拒絕；這要改 `rah-api` 契約與兩個 loop，需另寫 ADR，待決定。
6. 評估後不做。harness2 的佐證之所以有用，是因為它的選擇器會拿佐證對照實際軌跡；RAH 的佐證會由模型自己填，無法驗證，必填欄位只會增加 token 而不增加可信度。`memory.remember` 的說明已要求一則筆記一個事實、上限 1000 字元，維持現狀。
7. 完成：`docs/HARNESS2.md`。每次執行用一份設定目錄的複本當 `--data-dir`（相對資料路徑以資料目錄為準）。

## 完成狀態

| 項 | 狀態 | ADR |
| --- | --- | --- |
| 1 CLI 匯出與軌跡格式 | 完成（`--export`，`rah_api::session::export`） | — |
| 2 無互動執行摘要 | 完成（`--summary`） | — |
| 3 專案指令檔、`system_file` | 完成（plugin `context-instructions`，feature `instructions`） | 0061 |
| 4 skills 檢查 | 完成，改用 `HostNotice` | — |
| 5 具名 subagent | 完成，`tools` 限制待決定 | 待寫 |
| 6 結構化記憶 | 評估後不做 | — |
| 7 harness2 執行器指南 | 完成（`docs/HARNESS2.md`） | — |
