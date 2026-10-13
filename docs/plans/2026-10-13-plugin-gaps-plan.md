# Plugin 缺口補齊計畫

2026-10-13。v0.40.0 之後，比對 pivot 計畫第 4 節的服務目錄和目前的 `plugins/`，整理出還缺的 plugin。依下列順序做，每段一個 commit；每段開始前重讀 ADR 0036，動到 `rah-api` 或 `rah-protocol` 的段落要先確認是否需要 ADR。

## G1：檔案工具不依賴 git（擴充 `tools-builtin`）

起因：`tools-builtin` 只有 `echo` 和 `fs.read`；列目錄和搜尋只在 `tools-repo`（`repo.list`、`repo.search`），而它要求工作目錄是 git repo。非 git 的資料夾裡，模型沒辦法找檔案。

- [x] `fs.list`：列出目錄（`depth` 1–32），跳過隱藏項目（`hidden` 可開啟），`skip_dirs`（預設 `.git`、`target`、`node_modules`）列出但不展開，可設定。
- [x] `fs.glob`：支援 `*`、`?`、`**`、`{a,b}`；沒有 `/` 的樣式比對任意深度的檔名。matcher 自己寫，不加 `globset`。
- [x] `fs.grep`：在工作目錄內搜尋文字或 regex，回傳檔案、行號和該行內容；略過二進位檔，結果和讀取的位元組都有上限。
- [x] 三個工具都宣告 `read`，沿用 `fs.read` 的路徑規則（相對路徑、沒有 `..`、不跟隨跳出 root 的 symlink）；走訪時完全不跟隨 symlink 和 junction。每次呼叫最多走訪 `max_scan`（預設 20000）個項目、回傳 `max_results`（預設 500）行，被截斷時輸出會說明。preset 的 policy 從 `"fs.read"` 改成 `"fs.*"`。
- [x] `root` 跟隨工作目錄：不需要改。`tools-builtin` 已在 `WORKSPACE_PLUGINS` 裡，`set_workspace` 和 `--root` 會用新的 `root` 重新設定它（計畫寫成時漏看了）。
- [x] Web UI 設定頁的工具清單來自執行中的工具，新工具自動出現，不需要改。
- 決定：只加 `regex`（`fs.grep` 需要 regex 語法）；glob 和目錄走訪自己寫，不加 `globset`、`ignore`。不讀 `.gitignore`（那是 `tools-repo` 的範圍）。

## G2：`context-compact`（長對話壓縮）

起因：`context-default` 的滑動視窗（`max_items`、`max_chars`）會直接丟掉最舊的歷史，長對話會失去前面談過的內容。

- [x] 新 plugin，掛在 `model.before_request`。hook 是優先序高的先跑，所以用 priority 10，排在 `context-default`（0）**之前**：要在視窗丟掉歷史之前就看到完整歷史（計畫原本寫「之後」是錯的）。歷史超過 `trigger_chars` 時，把最舊的部分交給 `model` 服務摘要（不帶 tool、不經過 hook），換成一則 system 摘要，保留最新的 `keep_chars`。切點一定在 user 訊息，不切進目前這一輪。
- [x] 摘要的重用：**改用快取，不寫進 session log。** `ModelRequest` 沒有 session id，hook 不知道請求屬於哪個 session，所以改成以「摘要涵蓋的歷史前綴」的 FNV-1a hash 當 key。同一段歷史的後續請求直接重用；歷史變長時接著擴充最新的摘要；fork 只在共用的前綴上命中，自然只沿用到 fork 點。`dir` 把快取存成檔案，重啟後仍可用。這樣不需要改 `rah-api`，也不需要 ADR。
- [x] 失敗或逾時（`timeout_secs`，預設 120）：這一輪照常進行，沿用已有的摘要（或不摘要），剩下交給 `context-default` 的視窗；同一段歷史五分鐘內不再重試，免得每一輪都卡在逾時。沒有記錄警告：plugin 目前沒有記錄管道，留給 G3 的 `telemetry-log`。
- [x] 可設定：`trigger_chars`、`keep_chars`、`chunk_chars`（一次摘要最多送多少）、`max_summary_chars`、`instructions`（摘要的 system prompt）、`model`（provider 的 model id）、`dir`。**沒有做「指定另一個 model plugin」**：kernel 的 service 查詢只回傳一個 `model`，要選特定 instance 得透過 `bind`，`context-compact` 不宣告 `requires`，暫不支援。
- [x] 測試：plugin 單元測試用假的 model（門檻內不變、摘要一次並重用、擴充、fork 邊界、失敗與退避、逾時、目前這一輪很大、`dir` 跨重啟、設定檢查）；`crates/rah-cli/tests/compact.rs` 用 `dev` preset 加 `model-mock` 端到端驗證它在 `context-default` 之前執行，且 session log 不變。
- 決定：不放進任何 preset（每次摘要都是一次額外的模型請求，對付費 API 有成本），使用者自己掛載；Web UI 的 Plugins 面板可以直接加。

## G3：`telemetry-log`（診斷紀錄與用量）

起因：出問題時只能翻 session log；沒有每輪的延遲、工具耗時、錯誤統計，也看不到 token 用量。

- [x] 新 plugin `telemetry-log`，訂閱 event bus，寫成 JSONL（`<dir>/<YYYY-MM-DD>.jsonl`，UTC；超過 `max_file_bytes` 接著寫 `.1`、`.2`；超過 `keep_days` 的檔案刪除），記錄 `turn`（結果、錯誤代碼與訊息、耗時、模型請求數、工具呼叫數、token 加總）、`model_request`（耗時、第一段文字的時間、是否推理、token）和 `tool`（名稱、決定、耗時、是否錯誤）。
  - 原本 loop 的事件只送進啟動者的 `TurnStream`，bus 上看不到。新增 `rah-api::agent_loop::TurnEvent`：兩個 loop 把每個事件也發到 bus 上，所以背景和 subagent 的 turn 也記得到。
  - 決定的對應：`allowed`（直接執行）、`approved`（問過、同意）、`denied`（policy 拒絕）、`rejected`（問過、拒絕），由 `ToolStarted` 和 `ApprovalRequired` 推得，不必改 `ToolInvoker`。
- [x] 不記錄訊息全文和工具輸出；工具參數只在 `include_arguments = true` 時記錄（截到 500 字元）。紀錄裡沒有 header 或 key。
- [x] Token 用量：`rah-protocol` 新增 `ModelUsage`、`ModelEvent::Usage`、`AgentEvent::ModelUsage`，寫在 ADR 0040。`model-responses` 讀 `response.usage`；`model-anthropic` 讀 `message_start` 和 `message_delta`（cache 讀寫算進 input）；`model-mock` 回報估計值（每 4 bytes 一個 token），讓測試不需要真的模型。
- [x] `frontend-web` 狀態列顯示目前 session 的 input／output token（滑過顯示 cached 和請求數）。**限制：**只算這個頁面開啟之後的用量，重新整理會歸零；用量沒寫進 session log，長期紀錄在 `telemetry-log`。
- [x] 測試：tracker 單元測試（四種決定、token 加總、失敗與取消、不含訊息文字和參數、`include_arguments`）、檔案輪替與刪除、日期換算、bus 端到端；provider 的用量解析（Responses、Anthropic 含 cache）；protocol 序列化；`crates/rah-cli/tests/telemetry.rs` 用 `dev` preset 跑一輪確認紀錄和 token。
- 決定：不放進任何 preset。

## G4：`sandbox-os`（OS 層隔離）

**暫緩（2026-10-13，使用者決定）：等本機有 WSL 可以驗證 Linux 再做。** 待決事項：Windows AppContainer 需要在工作目錄加上永久的 ACL（改動主機設定），而且裝在使用者目錄的工具（例如 `~/.cargo` 的 rustup）在容器內讀不到；Linux 的 landlock 目前只能靠 CI 驗證。

起因：`sandbox-process` 只是程序監督（Job Object／process group），不是隔離；`shell.exec` 跑的程式可以讀寫使用者能存取的任何東西，也能連網。

- [ ] 先寫 ADR：威脅模型、各平台採用的機制、能宣稱和不能宣稱的保證（遵守 AGENTS.md「不宣稱未實作的隔離」）。
- [ ] Windows：AppContainer 或受限 token，加上對工作目錄的 ACL；Linux：landlock（檔案）和 seccomp 或 network namespace（網路），不可用時拒絕啟動，不默默退回。
- [ ] 提供同一個 `sandbox` 服務，可以用 `admin` 的 `replace` 直接換掉 `sandbox-process`。
- [ ] 設定：可寫的路徑清單、是否允許網路。
- [ ] 測試：寫入工作目錄外失敗、連網被擋（設定禁止時）、取消和逾時仍終止整個 process tree。不支援的平台上測試要明確略過並說明。
- 這段工作量最大，可能拆成「Windows」和「Linux」兩個 commit。

## G5：`storage-fs`

原本寫「出現第二個需要持久狀態的 plugin 才做」。G2 的 `context-compact` 用自己的 `dir` 存摘要快取，加上 `context-default` 的記憶檔，條件已經成立；WASM plugin 沒有 WASI，也完全沒辦法保存狀態。2026-10-13 使用者要求開始。

- [x] ADR 0041：`rah-api::storage`，`STORAGE: ServiceKey<dyn KvStore>`，`get`、`put`（原子取代）、`delete`、`list`（前綴、排序）。值是 bytes；每個呼叫帶 namespace，慣例是 plugin id。名稱 1–128 bytes，字元同 plugin id（英數和 `._:-`），區分大小寫。`MemoryStore` 給測試用。
- [x] 不做 `BlobStore`：目前沒有串流或大型二進位的需求，等有消費者再加新合約。
- [x] `plugins/storage-fs`：`<dir>/<namespace>/<key>` 一個檔案一筆；檔名可逆編碼成全小寫（其他 byte 寫成 `%xx`，Windows 裝置名稱跳脫），所以不分大小寫的檔案系統也正確；`put` 寫暫存檔、flush、rename；啟動時刪除超過一小時的暫存檔；`max_value_bytes` 預設 16 MiB。所有 preset 都掛（`dir = ".rah/storage"`）。
- [x] process 和 WASM plugin：`requires` 可以有 `storage`，`service/call`／`call-service` 提供 `get`、`put`、`delete`、`list`，值是 JSON（`rah_api::storage::call_json`，兩個 host 共用）；host 把 namespace 固定成 plugin id，碰不到別人的資料。process protocol 版本不變（3）。
- [x] `context-compact` 改用 `storage`（每次使用時解析，沒有 provider 就只留記憶體快取），移除 `dir`。
- 決定：`context-default` 的記憶繼續是 Markdown 檔，因為使用者會直接打開來看和改；`storage-fs` 的檔名不適合手動編輯。
- [x] 測試：合約和 `call_json`（rah-api）；`storage-fs` 的編碼、CRUD、拒絕無效名稱和過大的值、略過暫存和外來檔案、並行寫入、重啟後仍在、清除舊暫存檔；`context-compact` 重啟後重用、沒有 storage 時不保存、`dir` 被拒；process 和 WASM host 的 namespace 固定和 `requires` 檢查；`crates/rah-cli/tests/compact.rs` 確認 preset 的 `storage-fs` 存了摘要。

## 備註

- **`model-chat-completions`：暫緩。** `model-responses` 只支援 `/v1/responses`，DeepSeek 官方 API、Ollama、vLLM 等只有 `/v1/chat/completions` 的服務目前接不上。使用者目前沒有預算購買 API key，先不做。之後要做時：放在新 plugin（不改 `model-responses`），wire format 留在 plugin 內；自動化測試用本機假伺服器，不需要 key；實際驗證可以先用本機的 Ollama 或 llama-server（不需要 key），DeepSeek 等付費服務等有 key 再驗。
- **`frontend-tauri`**：pivot 計畫已列為選配，`frontend-web` 足夠，不排入。
- 每段完成時跑 AGENTS.md 的完整驗證（fmt、check、test、clippy、`git diff --check`），並更新 `docs/ARCHITECTURE.md` 的 plugin 清單。
