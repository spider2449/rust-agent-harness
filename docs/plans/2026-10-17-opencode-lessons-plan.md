# 向 opencode 學習的項目

2026-10-17。研究 `F:\coding\otherPrj\opencode`（v2 重寫版，HEAD `a49f179170`）後整理出的 10 項。依下列順序做，每項一個 commit；動到 `rah-api`、`rah-protocol` 或 plugin 協定的項目先寫 ADR（先讀 ADR 0036）。

注意：這份 checkout 的 opencode **沒有 LSP 實作**，只有設定 schema（`packages/schema/src/config/lsp.ts`）和 `file-mutation.ts`、`tool/plugin/edit.ts`、`write.ts` 裡的 TODO（「寫檔後通知 LSP 並收集 diagnostics」）。第 1 項是照它的設計方向自己做，沒有可抄的程式碼。

不學：Effect-TS 寫法、內建大量 provider（RAH 的 provider 是 plugin）、它的 session SQL 儲存（RAH 用 JSONL）。

## 1. LSP diagnostics plugin（完成，ADR 0048）

- opencode 的設定形狀：`lsp: false | { <name>: { command[], extensions?, env?, initialization?, disabled? } | { disabled: true } }`。
- 實作改為 `tool.after_invoke` hook plugin `lsp-diagnostics`，不新增 `rah-api` 契約（ADR 0048）：write 工具成功後，從呼叫參數（`path`、`edits[].path`、`patch`）取得檔案，送給對應副檔名的 server，把等級足夠的 diagnostics 附在 tool output。
- LSP server 的程序屬於 plugin `Scope`，scope 結束時關閉。診斷結果要有數量與長度上限，且視為不可信輸入。

## 2. Tool 輸入自動修復（完成，ADR 0049）

- 新增 waterfall hook `tool.repair_input`（`before_invoke` 是 bail hook，不能改寫輸入），在 policy 之前執行；plugin `tool-input-repair` 依 JSON Schema 做可明確判定的修正：字串化的物件或陣列、數字或布林字串、optional 的 `null`、封閉物件的多餘欄位、巢狀欄位與 `$ref`（深度上限 6）。
- 只在 schema 明確支援時才修，其餘原樣交給後面的驗證。

## 3. Tool 輸出分三層（已檢查，不改）

- `output`（有 schema、給程式用）、`content`（給模型看、會持久化）、`metadata`（給 UI 的小型 JSON）。先檢查目前 tool result 是否把三者混在一起。
- 結果（2026-10-17）：`ToolOutput` 已是型別化的 `content`（`Text`、`Json`、`Image`）加 `is_error`，沒有把三層混成文字。但目前沒有任何消費者需要另外兩層：模型、Web UI、CLI、匯出都把 `Json` 轉成文字顯示（`ToolContent::Json(value) => value.to_string()`），沒有 CodeMode 會讀機器用的 `output`，UI 也沒有讀 `metadata`。現在加欄位是沒有使用者的協定變更（`rah-protocol`、process 協定、WASM 都要動），所以不做。
- 重新評估的條件：出現第一個真實消費者時才加，例如 UI 的 diff 檢視或 CodeMode。屆時加向後相容的 `metadata: Option<Value>`（serde default），並寫 ADR。

## 4. Tool registry 語意（已檢查，只補測試）

- 同名工具由最新的有效註冊勝出；dispose 註冊後重建並露出被蓋過的舊定義。
- 每次 model request 抓一份當時的快照，之後 reload 不影響進行中的請求。
- MCP 工具清單變動時只更新自己的來源並 reload，不改變其他 plugin 的優先順序。對照 `tools-mcp` 的刷新行為。
- 結果（2026-10-17）：
  - 快照：兩個 loop 每輪 model request 都重新呼叫 `invoker.definitions()`，已符合。
  - dispose：卸載 provider 後它的工具立即消失，露出同名的另一個定義，已符合。
  - 同名衝突：RAH 是**最舊的 provider 勝出**，與 opencode 相反，刻意保留。policy 依工具名稱指派權限，後掛載的 plugin 不能接手已有人提供的名稱。補了測試鎖定這個行為與卸載後的露出（`crates/rah-api/tests/invoker.rs`）。
  - MCP 清單刷新：不適用。`tools-mcp` 只暴露主機在設定中列出的工具（`tools` 對照權限），不做動態探索，所以沒有「清單變動」要處理，也沒有優先序被打亂的問題。

## 5. Shell 命令掃描（完成，改成 policy 的 `commands` 規則，ADR 0050）

- 把命令拆成多個 command，無法分析時回報 `opaque`（`command-substitution`、`heredoc`、`dynamic-command-name` 等）。opaque 只代表分析不了，不是權限決定。
- 讓 `policy-authority` 可做命令前綴層級的 allow，遇到 opaque 就 fail closed。
- 結果（2026-10-17）：RAH 的 `shell.exec` 收 `program` 加 `args` 陣列，沒有 shell 字串，也不經過 shell，所以不需要 parser，也不會有 `opaque`。只採用比對的部分：`policy-authority` 新增 `commands`，規則是 `["git","status"]`（完全比對）或 `["cargo","check","*"]`（最後的 `*` 接受任何後續參數）；只把 Ask 變成 Allow，不蓋過 `decisions`、未指派或被拒的等級；輸入格式不對就不匹配。

## 6. Retry 策略（完成，ADR 0051）

- 指數退避從 2 秒起、單次間隔上限 10 秒、最多 10 次、加 jitter；provider 的 `retry-after` 上限 15 分鐘。
- 結果（2026-10-17）：原本 provider 都設 `retry::never()`，上層也沒有重試。新增 `ModelError::Unavailable`（429/500/502/503/504/529，含 `Retry-After` 秒數）和 `rah_api::retry::stream`，只重試「開啟串流」階段的 `Unavailable` 與 `Transport`，串流已開始就不重試（避免文字重複）。兩個 loop 用 `retries`（10）與 `retry_delay_ms`（2000）設定；新增 `AgentEvent::ModelRetrying`，Web UI、終端顯示，telemetry 計次，session log 不記。

## 7. 檔案寫入（完成）

- 依解析後的路徑加 keyed mutex；多檔交易先排序再上鎖以避免死鎖。
- 保留既有的 UTF-8 BOM，且最多輸出一個。
- 文件註明鎖只在 process 內有效，外部寫入仍可能衝突。
- 結果（2026-10-17）：BOM 已被保留（整份檔案以字串讀寫，BOM 是開頭的字元），補了測試鎖定「恰好一個」。真正的缺口是並行寫入：`tools-repo-write` 的讀、改、寫之間沒有鎖，多個 session、背景工作或 subagent 同時編輯同一個檔會丟失更新（拿掉鎖後測試會失敗）。現在每個檔案一把鎖（多檔編輯依排序取鎖，避免死鎖），`repo.patch` 取獨占鎖等所有編輯結束。鎖是 plugin instance 內的狀態，不是全域；只涵蓋 RAH 自己的呼叫，不擋外部編輯器。

## 8. Compaction 設定（完成 overflow，其餘不採用，ADR 0052）

- 對照 `context-compact`：`auto`、`buffer`（預設保留 10%）、`keep`（保留最近多少 token 原文），以及觸發原因 `auto` / `overflow`（provider 剛拒絕）/ `manual`。
- 結果（2026-10-17）：只採用 `overflow`，因為只有它背後有可觀察的事實（provider 剛拒絕）。新增 `ModelError::ContextOverflow`（三個 provider 讀最多 8 KiB 的 400/413/422 回應本文比對常見片語，但不重複內容）、hook `model.overflow`，`context-compact` 回應它，一次摘要約一半的歷史，loop 只重送一次；session log 不變。`auto` 與 token `buffer` 需要 token 數（RAH 沒有 tokenizer，ADR 0004），掛載 plugin 本身就是開關；`manual` 需要各 frontend 共用的指令介面，不是 plugin 行為。

## 9. Formatter 自動偵測（完成，ADR 0053）

- 每個 formatter 有 `enabled` 檢查（`findUp` 設定檔或可執行檔）與副檔名對應；寫檔後的 formatter plugin，和第 1 項放在同一條 hook 鏈。
- 結果（2026-10-17）：新 plugin `format-on-write`（`tool.after_invoke`，優先序 -5，在 `lsp-diagnostics` 之前），預設 `rustfmt`、`gofmt`，`ruff`/`black` 要 `pyproject.toml`（或 `ruff.toml`）設定，`clang-format` 要 `.clang-format`。**預設不含 prettier**：它的設定可以是 JavaScript、plugin 是程式碼，模型寫的檔案可能讓 formatter 執行它；要用者在 `formatters` 明確設定。格式化改變了檔案時會告訴模型重讀（`repo.edit-files` 要精確比對文字）。
- 研究 LSP 時發現的安全問題一併記錄：language server（`rust-analyzer` 的 build script）會執行工作區的程式碼，模型寫的檔案可能讓它執行，等於 `write` 核准變成程式執行。已寫入 `docs/SECURITY.md` 與 ADR 0048，建議搭配 `sandbox-os`。

## 10. CodeMode（只記錄，不實作）

- 模型寫一段程式串多個工具，只回傳最後結果。選用的進階功能，列為記錄，不實作。
- 結果（2026-10-17）：維持不做。它需要第 3 項的機器用 `output` 層和一個沙箱化的程式執行環境，兩者目前都沒有消費者或需求；等有真實需求時再單獨提案（ADR）。

## 完成狀態（2026-10-17）

| 項 | 狀態 | ADR |
| --- | --- | --- |
| 1 LSP diagnostics | 完成（plugin `lsp-diagnostics`，feature `lsp`） | 0048 |
| 2 輸入修復 | 完成（hook `tool.repair_input`，plugin `tool-input-repair`，preset 預設掛載） | 0049 |
| 3 輸出分三層 | 已檢查，不改 | — |
| 4 registry 語意 | 已檢查，補測試 | — |
| 5 命令比對 | 完成（policy `commands`） | 0050 |
| 6 重試 | 完成（`rah_api::retry`，loop 設定 `retries`、`retry_delay_ms`） | 0051 |
| 7 檔案寫入 | 完成（`tools-repo-write` 的鎖） | — |
| 8 compaction | 完成 `overflow`（hook `model.overflow`），其餘不採用 | 0052 |
| 9 formatter | 完成（plugin `format-on-write`，feature `format`） | 0053 |
| 10 CodeMode | 只記錄 | — |
