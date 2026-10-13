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

- [ ] 新 plugin，訂閱 event bus，寫成 JSONL（每天一個檔、大小上限、保留天數），記錄每輪和每個模型請求的開始、結束、耗時，工具呼叫的名稱、決定（allow／ask／deny）、耗時和結果類別，以及錯誤。
- [ ] 不記錄訊息全文和工具參數（預設），可以設定開啟；紀錄裡不得出現 API key 或 header。
- [ ] Token 用量：`rah-protocol` 的 `AgentEvent` 目前沒有用量欄位。新增一個 provider 中立的用量事件（input／output／cached tokens，皆為選填），由 `model-responses` 和 `model-anthropic` 從各自的回應中填入。這是 protocol 變更，要寫 ADR。
- [ ] `frontend-web` 在對話標題列或狀態列顯示本 session 的累計 token（有資料時才顯示）。
- [ ] 測試：事件寫入格式、輪替、不含敏感欄位、mock 模型回報用量後累計正確。

## G4：`sandbox-os`（OS 層隔離）

起因：`sandbox-process` 只是程序監督（Job Object／process group），不是隔離；`shell.exec` 跑的程式可以讀寫使用者能存取的任何東西，也能連網。

- [ ] 先寫 ADR：威脅模型、各平台採用的機制、能宣稱和不能宣稱的保證（遵守 AGENTS.md「不宣稱未實作的隔離」）。
- [ ] Windows：AppContainer 或受限 token，加上對工作目錄的 ACL；Linux：landlock（檔案）和 seccomp 或 network namespace（網路），不可用時拒絕啟動，不默默退回。
- [ ] 提供同一個 `sandbox` 服務，可以用 `admin` 的 `replace` 直接換掉 `sandbox-process`。
- [ ] 設定：可寫的路徑清單、是否允許網路。
- [ ] 測試：寫入工作目錄外失敗、連網被擋（設定禁止時）、取消和逾時仍終止整個 process tree。不支援的平台上測試要明確略過並說明。
- 這段工作量最大，可能拆成「Windows」和「Linux」兩個 commit。

## G5：`storage-fs`（視需要）

計畫列了 `storage` 服務（`KvStore`、`BlobStore`），但目前沒有 plugin 需要共用的持久狀態：session 和記憶各自寫檔是合理的。只有在 G1–G4 或之後的工作出現第二個需要持久狀態的 plugin 時才做，屆時在 `rah-api` 新增服務合約並寫 ADR。現在不做。

## 備註

- **`model-chat-completions`：暫緩。** `model-responses` 只支援 `/v1/responses`，DeepSeek 官方 API、Ollama、vLLM 等只有 `/v1/chat/completions` 的服務目前接不上。使用者目前沒有預算購買 API key，先不做。之後要做時：放在新 plugin（不改 `model-responses`），wire format 留在 plugin 內；自動化測試用本機假伺服器，不需要 key；實際驗證可以先用本機的 Ollama 或 llama-server（不需要 key），DeepSeek 等付費服務等有 key 再驗。
- **`frontend-tauri`**：pivot 計畫已列為選配，`frontend-web` 足夠，不排入。
- 每段完成時跑 AGENTS.md 的完整驗證（fmt、check、test、clippy、`git diff --check`），並更新 `docs/ARCHITECTURE.md` 的 plugin 清單。
