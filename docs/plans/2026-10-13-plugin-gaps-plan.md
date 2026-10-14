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

2026-10-13 暫緩（等 WSL）；2026-10-14 本機有 WSL（Ubuntu 24.04、Linux 6.18、landlock ABI 7）後先做 Linux。

起因：`sandbox-process` 只是程序監督（Job Object／process group），不是隔離；`shell.exec` 跑的程式可以讀寫使用者能存取的任何東西，也能連網。

- [x] ADR 0042：威脅模型、機制、能宣稱和不能宣稱的保證（預設 `readable = "/"` 不隱藏憑證；Unix socket 檔案（`ssh-agent`、`docker.sock`）擋不住；沒有 CPU／記憶體限制；`setsid` 的子孫可逃離 process group）。
- [x] Linux：landlock（ABI 3 以上，處理 kernel 支援的所有檔案權限，含 `TRUNCATE`）＋ seccomp（`network = false` 時拒絕 `AF_UNIX` 以外的 socket 和 `io_uring_setup`，其他架構的 syscall 直接終止）。ABI 6 以上另外限制 signal 和 abstract Unix socket。在 `fork` 之後、`exec` 之前套用，所以涵蓋所有子孫。不可用時拒絕啟動，不退回。
  - 網路改用 seccomp 而不是 network namespace：Ubuntu 24.04（CI runner）用 AppArmor 限制非特權 user namespace，netns 在 CI 上無法驗證。代價是 loopback 也被擋。
- [x] 提供同一個 `sandbox` 服務；放在 `sandbox-process` crate 裡當第二個 catalog plugin（共用監督程式碼，`model-responses`、`tools-mcp` 已有一個 crate 兩個 plugin 的先例），`rah-api` 和 protocol 不變。加入 `WORKSPACE_PLUGINS`，`--root` 和 `set_workspace` 會移動它的 root。不放進 preset（Windows 不能用；預設不能連網）。
- [x] 設定：`readable`（預設 `/`）、`writable`（相對路徑以 `root` 為準，必須存在）、`network`（預設 `false`），加上 `sandbox-process` 原有的 `root`、`inherit_env`、`max_timeout_ms`。另外建立權限 0700 的私人暫存目錄當 `TMPDIR`，unmount 時刪除。
- [x] 測試（WSL 實跑；CI 是 ubuntu-latest）：寫入工作目錄外失敗、TCP 和 UDP 被擋、`writable` 和 `network = true` 可以開放、縮小 `readable` 後讀不到、逾時／取消／孤兒子孫／unmount 仍終止 process tree、無效設定、`TMPDIR` 在 unmount 後刪除；`crates/rah-cli/tests/sandbox_os.rs` 端到端經過 `policy-authority` 和 `shell.exec`。landlock ABI 低於 3 的 kernel 印出原因後略過；非 Linux 平台則斷言 plugin 拒絕啟動。
- [x] Windows（2026-10-14，ADR 0043）：用 Low integrity token，不用 AppContainer。原型結果：AppContainer 必須建立 profile，且容器內打不開 `NUL`，Git 一啟動就失敗；Low IL 下 Git、`NUL`、`cmd` 都正常。
  - token：`CreateRestrictedToken`（拿掉所有 privilege，Administrators 改為 deny-only）再降到 Low。std 不能指定 token（`raw_attribute` 仍 unstable），所以自己用 `CreateProcessAsUserW`：suspended 啟動、先加入 Job Object 再 resume（`sandbox-process` 的逃脫空檔不存在）、只繼承三個標準 handle、標準輸出入走 named pipe。supervise 和 spawn 改用共用的 `Process` 抽象，兩種 child 都適用。
  - root、`writable` 和私人 `TEMP` 加上可繼承的 Low label（會套用到既有檔案）。**label 不移除**（使用者確認）；已有 label 就略過。拒絕磁碟根目錄、使用者目錄和 Windows 目錄。
  - 讀取和網路都不限制：`readable` 直接拒絕，`network` 必須設 `true`（預設 `false` 時拒絕啟動）。batch 檔拒絕，要明確用 `cmd.exe /c`。
  - 測試在本機 Windows 跑（CI 只有 Linux）；起初只在 elevated token 下測過。
  - 2026-10-14 以一般使用者帳號（Medium、非系統管理員）實測：設 label 需要 `WRITE_OWNER`，只有 Modify 權限的目錄（非系統磁碟的預設 ACL）會 Access denied。改為被拒時先給目前使用者加一個可繼承的 `WRITE_OWNER` ACE 再設 label，ACE 和 label 一樣保留（使用者確認；ADR 0043 修訂）。

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

- **`model-chat-completions`：2026-10-14 完成。** 原本暫緩（`model-responses` 只支援 `/v1/responses`，DeepSeek 官方 API、Ollama、vLLM 等只有 `/v1/chat/completions` 的服務接不上；使用者沒有預算買 API key）。做法照原計畫：新 plugin `plugins/model-chat-completions`（不改 `model-responses`），wire format 留在 plugin 內，`rah-api` 和 protocol 不變，不需要 ADR。
  - `provider`（`deepseek`、`ollama`、`llamacpp`、`vllm`）帶入位址、模型和 key 變數，其他欄位可覆寫；`base_url` 必須是 HTTPS，或是 loopback 的 HTTP。沒設模型時用 `<base_url>/models` 唯一的模型。新增 `deepseek` preset，`--model` 和 Web UI 模型設定都支援。
  - 測試：loopback 假伺服器的單元測試和 `crates/rah-cli/tests/model_chat_completions.rs` 端到端；本機 llama-server 實測一輪（工具呼叫、推理、token 用量含 cached）。DeepSeek 官方 API 等有 key 再驗。
  - 已知限制：推理內容不重送（中性 context 沒有這個欄位）。若 DeepSeek 的 thinking 模式在工具呼叫之間要求帶回 `reasoning_content`，`deepseek-reasoner` 可能會被拒，要等實測確認。
- **`frontend-tauri`**：pivot 計畫已列為選配，`frontend-web` 足夠，不排入。
- 每段完成時跑 AGENTS.md 的完整驗證（fmt、check、test、clippy、`git diff --check`），並更新 `docs/ARCHITECTURE.md` 的 plugin 清單。
