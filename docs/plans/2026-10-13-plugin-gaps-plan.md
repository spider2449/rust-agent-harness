# Plugin 缺口補齊計畫

2026-10-13。v0.40.0 之後，比對 pivot 計畫第 4 節的服務目錄和目前的 `plugins/`，整理出還缺的 plugin。依下列順序做，每段一個 commit；每段開始前重讀 ADR 0036，動到 `rah-api` 或 `rah-protocol` 的段落要先確認是否需要 ADR。

## G1：檔案工具不依賴 git（擴充 `tools-builtin`）

起因：`tools-builtin` 只有 `echo` 和 `fs.read`；列目錄和搜尋只在 `tools-repo`（`repo.list`、`repo.search`），而它要求工作目錄是 git repo。非 git 的資料夾裡，模型沒辦法找檔案。

- [ ] `fs.list`：列出目錄（可選遞迴深度、上限筆數），跳過隱藏和常見的大型資料夾（`.git`、`target`、`node_modules`），可設定。
- [ ] `fs.glob`：依 glob 樣式找檔案，結果有上限。
- [ ] `fs.grep`：在工作目錄內搜尋文字或 regex，回傳檔案、行號和該行內容；略過二進位檔，結果和讀取的位元組都有上限。
- [ ] 三個工具都宣告 `read`，沿用 `fs.read` 的路徑規則（相對路徑、沒有 `..`、不跟隨跳出 root 的 symlink）。
- [ ] `root` 目前預設為程序的工作目錄；改成有 `sandbox` 時跟隨 `ExecEnv::root()`，切換工作目錄（U2b）後不必重啟。沒有 sandbox 時維持現狀。
- [ ] Web UI 設定頁的工具權限清單自動出現新工具（確認即可，不另寫表單）。
- 待決：新依賴（`globset`、`regex`、`ignore`）要寫明理由；若 `ignore` 太大，只用 `globset` 加自己的走訪。

## G2：`context-compact`（長對話壓縮）

起因：`context-default` 的滑動視窗（`max_items`、`max_chars`）會直接丟掉最舊的歷史，長對話會失去前面談過的內容。

- [ ] 新 plugin，掛在 `model.before_request`，排在 `context-default` 之後（hook 優先序）。歷史超過門檻時，把會被丟掉的那段交給 `model` 服務摘要，換成一則摘要訊息。
- [ ] 摘要以附加的 metadata 行寫進 session log（不改寫既有紀錄），之後的請求重用，不重複摘要；fork 沿用父 session 的摘要，但只到 fork 點為止。
- [ ] 摘要請求失敗或逾時時，退回滑動視窗的行為並記錄一次警告，不讓這一輪失敗。
- [ ] 可設定：觸發門檻、保留最近幾輪原文、摘要的 system prompt、摘要用的 model plugin（預設同一個）。
- [ ] 測試用 `model-mock`：門檻前不摘要、門檻後摘要一次並重用、摘要失敗的退回、fork 之後的摘要邊界。
- 待決：如果需要 `SessionLog` 新增「附加 metadata」的通用方法（目前只有標題用的 metadata 行），屬於 `rah-api` 合約變更，先確認 ADR 0036 範圍內即可，或補 ADR。

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
