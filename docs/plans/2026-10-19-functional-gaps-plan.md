# 功能檢查後的改進計畫

2026-10-19。依據：完整驗證（fmt、check、clippy、test、`git diff --check` 都通過），以及用 `dev`（mock）和 `default`（llama.cpp，`ornith-1.5-35b`）預設做的端到端功能檢查。每項一個 commit；動到 `rah-api`、`rah-protocol` 或 plugin 協定的項目先寫 ADR（先讀 ADR 0036）。

標記「實測」的缺口是檢查時親眼看到的；標記「推測」的只是從文件和模型回報的工具清單推論，開工前要先對照程式碼確認。

不做：自動重放不確定的外部效果（AGENTS.md 不變量）；對 `sandbox-process` 宣稱更強的隔離（它只是行程監管，要隔離用 `sandbox-os`）。

## 1. 被拒絕的工具呼叫不要被反覆重試（實測）

- 現象：寫檔被拒後，模型對同一個呼叫連續重送 4 次，每次都重新詢問使用者。
- 先查：`denied: rejected by the user` 這個錯誤在哪裡產生、回給模型的文字是什麼。
- 做法：拒絕的結果文字明確告訴模型「使用者已拒絕，不要重試相同操作」。若仍重送，再評估在 loop 端對相同呼叫的連續拒絕設上限（`loop-react`、`loop-plan` 各自設定，不放進 kernel）。
- 驗收：用 mock 模型重送同一個被拒的呼叫的測試；拒絕仍然 fail closed，不改變 policy 判定。
- 結果（2026-10-19，完成）：錯誤在 `rah_api::tools::ToolInvoker::decide` 產生。使用者拒絕的文字改為 `rejected by the user; do not send the same call again, ask what to do instead`（常數 `USER_REJECTION`），前綴 `denied: rejected by the user` 不變，所以既有測試與 UI 的比對不受影響。補了 `a_declined_call_tells_the_model_not_to_send_it_again`（`crates/rah-api/tests/invoker.rs`）。用 llama.cpp（`ornith-1.5-35b`）重跑同一個情境：核准提示由 4 次降為 1 次，模型被拒後停下來詢問。沒有加 loop 端的連續拒絕上限，因為文字已足夠；若換模型又出現重送再評估。這是文字契約的小改動，不動服務契約，不需 ADR。

## 2. 核准提示與工具行的輸出順序（實測）

- 現象：非互動模式下，`[approve]` 與 `[tool]` 的先後偶爾對調。只影響顯示。
- 範圍：`plugins/frontend-cli`，核准提示（`confirm`）與事件輸出的排序。
- 驗收：確定性測試，固定先顯示 `[tool]` 再顯示 `[approve]`。
- 結果（2026-10-19，完成）：原因是 `turn()` 的 `tokio::select!` 在事件串流與核准提示同時就緒時隨機選一邊。loop 在呼叫 invoker 之前就送出 `ToolRequested`，所以改為 `biased;`、事件優先。補了 `a_tool_call_is_shown_before_its_approval_question`（假 loop 要等核准被回答才結束，重複 40 次）；拿掉 `biased;` 時這個測試會失敗，已驗證。`plugins/frontend-cli` 新增 `tokio` 的 `rt` dev-dependency，只給測試用。

## 3. 非互動模式的明確核准選項（實測，需 ADR）

- 現象：`--prompt` 只能從 stdin 讀 `y`；沒給就一律拒絕（預設正確）。在腳本與 PowerShell 管線裡難以使用，PowerShell 5.1 的管線編碼會讓 `y` 變成拒絕。
- 方向：有範圍限制的選項，例如只允許某個權限等級或某些工具，不做全域的 yes-to-all。啟動時像 `minimal` 一樣大聲警告。
- 先決：寫 ADR，說明它不會繞過 `tool.before_invoke` 鏈與 policy 的 fail closed，只是代替人回答 `ToolApproval`。
- 驗收：預設行為不變；選項關閉時與現在完全相同；測試涵蓋範圍外的呼叫仍被拒。
- 結果（2026-10-19，完成，ADR 0054）：使用者選「依工具名稱」。讀程式碼時發現 `decisions = { x = "allow" }` 的判斷在「沒有指派就拒絕」之前，拿它做旗標會讓拼錯或沒指派的工具被放行，違反 fail closed，所以改仿 ADR 0050：`policy-authority` 新增 `approve`（精確工具名稱），只把 Ask 變成 Allow，不蓋過 `decisions`、沒有指派或被拒的等級；`*`、前綴樣式與空名稱在設定時就被拒絕。`rah --approve a,b`（可重複）把名稱加進本次執行所有 `policy-authority` 的 `approve`，不存檔；沒有 `policy-authority` 時失敗；啟動時在 stderr 印警告列出工具。沒有動 kernel、`rah-api` 或 plugin 協定。`rah-desktop` 不加此選項（它有自己的互動核准）。用 llama.cpp 驗證：stdin 空白時，被核准的 `repo.create-file` 直接建立檔案，未核准的 `repo.edit-files` 仍然詢問、被拒、檔案不變。
- 沒做：核准「同一工具的某些參數」。`approve` 信任整個工具；要看參數用 `commands`（ADR 0050）。

## 4. 推理內容送回後續請求（文件已記載，需調查）

- 現象：README 記載推理內容不會送回後續請求；對會思考的模型搭配多步工具呼叫，可能影響品質。
- 先查：各 provider（`model-chat-completions`、`model-responses`、`model-anthropic`）對推理內容的要求不同，Anthropic 有簽章區塊的規則。先在 llama.cpp 上量測有沒有實際差異，再決定要不要做。
- 做法（若要做）：各 provider plugin 內的選項，wire 格式留在 provider 裡；`rah-api` 契約保持 provider 中立。需要 ADR。
- 結果（2026-10-19，已量測，暫不實作）：
  - 現況：整條路徑刻意不帶推理內容。`ModelEvent::ReasoningStarted` 只是進度訊號（`rah-protocol`），三個 provider 的 wire 都丟棄內容，session log 也不記。要回送就得改 `rah-protocol` 的事件、`ContextItem` 與 session 紀錄，並決定要不要把模型的推理寫進 log（`docs/SECURITY.md` 與 ADR 0040 的立場是不攜帶），不是 provider 內的小改動。
  - 量測（llama.cpp，`ornith-1.5-35b`，直接呼叫 `/v1/chat/completions`，工具呼叫兩步，溫度 0）：伺服器回傳 `reasoning_content`（80 字元）；不回送時第二步回答 `The code is **CODE-7731**.`，回送時回答 `The code is CODE-7731.`，內容相同，`prompt_tokens` 從 305 變 324（多 19）。伺服器接受回送，但不回送沒有任何壞處。我們自己的端到端測試（讀檔後回答）也在不回送的情況下正常完成。
  - 未驗證：需要回送的是 DeepSeek 的思考模式與 Anthropic 的思考區塊（`model-anthropic` 只送 `effort`，沒有 `thinking` 參數，是否會產生需要回送的區塊無法在離線環境確認）。這兩個要金鑰，不在這輪範圍。
  - 決定：不實作。理由是沒有可觀察的失敗，而且成本是改動協定與 session log。重新評估的條件：在真實 DeepSeek 或 Anthropic 上看到「工具呼叫後被拒絕」的實際錯誤。屆時先寫 ADR，再從 `model-chat-completions` 開始（它的 `reasoning_content` 已被解析，只是被丟棄）。README 的限制說明維持不變。

## 5. 寫入類工具的缺口（推測，需使用者決定）

- 模型回報的工具只有 `repo.create-file`、`repo.edit-files`、`repo.patch`；沒看到刪除、改名、搬移與 git commit。
- 舊架構有對應的 ADR（0016 到 0020），但那條線不管轄 `master`。若要補回，是新的 plugin，依 `tools-repo-write` 的模式，經 `ToolInvoker` 與 policy 的權限等級。
- 先決：確認這些工具在目前的 `master` 是否真的不存在，並由使用者決定要補哪些。
- 結果（2026-10-19，完成，ADR 0055）：確認目前只有 `repo.create-file`、`repo.edit-files`、`repo.patch`。使用者選了刪除、改名、commit 三個都做。ADR 0036 第 4 點寫「其餘留在 legacy」，理由是舊工具內建很重的 HostExplicit 審核流程；現在審核是通用的 policy `Ask`，理由不再適用，所以依使用者指示新增，並以 ADR 0055 修訂（0036 第 4 點加了註記）。這不是還原 legacy crate，而是在 `tools-repo-write` 內的新工具。
  - `repo.delete-file {path}`、`repo.move-file {from, to}`：宣告 `write`，只處理一般檔案，拒絕符號連結、目錄、不存在的路徑；move 不覆蓋、會建立缺少的上層目錄；路徑規則與每檔鎖同其他工具（拒絕 `.git`、`.rah/`、`..`）。
  - `repo.commit {message, paths}`：宣告 `execute`（寫入 git 歷史並執行 repo 的 hooks）。只有設定了 `commit = { name, email }` 才存在；以該身分提交，不讀 Git 的全域或系統設定，不能 amend、不能略過 hooks、不能 push；`paths` 是字面檔名（`--literal-pathspecs`，`*` 不是樣式，目錄被拒絕），只提交指定的檔案，先前暫存的其他檔案不會被帶進去；沒有變更時拒絕，不產生空 commit。
  - 預設 policy 已加指派（`repo.delete-file`、`repo.move-file` 為 `write`，`repo.commit` 為 `execute`）。
  - 驗證：`tools-repo-write` 在這台 Windows 機器上 14 個測試通過，其中新增 5 個（刪除、移動、commit 設定與等級、commit 只提交指定檔案與身分與刪除、commit 拒絕樣式與目錄等不安全輸入）；另有 1 個符號連結測試只在 Unix 編譯執行，這台沒跑，未經驗證；用 llama.cpp 在暫存的 git 工作區實測，模型依序改名、刪除、提交，Git 顯示 `R100 old.txt → new.txt`、`D junk.txt`，作者為設定的身分，工作樹乾淨。符號連結的測試在這台 Windows 機器上不會執行（`cfg(unix)`）。

## 6. 工作階段內的「一律允許」（推測）

- 現象：核准只有一次性的 y/N。`policy-authority` 已有 `commands` 規則，可能只缺互動入口。
- 做法：在 `ToolApproval` 的回答加上「本 session 內允許這類呼叫」，狀態放在 plugin 的 scope，不寫進設定檔，session 結束即失效。
- 先決：確認不會與 ADR 0050 的 `commands` 規則重複；需 ADR。
- 結果（2026-10-19，完成，ADR 0056）：修正先前的判斷，不需要改 `tool.approval` 的回傳型別。兩個前端各自回答這個 hook，所以由前端在記憶體記住「這個 session 允許這個工具」即可，`rah-api` 與 plugin 協定都不動。使用者選「依工具名稱、CLI 與 Web 都做」。
  - CLI：回答 `a`（或 `always`），提示變成 `[y/N/a=always]`；Web：多一個「Allow for this session」按鈕，WebSocket 訊息多可選的 `always`，`approval_required` 通知多 `may_remember`。
  - 單位是（session、工具名稱），不看參數；存活到程序結束，不寫入 session log、storage 或設定；`/new` 或換 session 會重新詢問。
  - `execute` 等級不提供：宣告為 `execute` 的工具，以及 policy 指派為 `execute` 的工具（核准請求只帶宣告等級，所以另外讀 reason 文字中的 `` `execute` ``，只會更嚴不會更寬）。Web 伺服器自己強制，不信任瀏覽器送來的 `always`。
  - 已知限制（寫在 ADR）：沒有列出或撤銷的指令，要靠 `/new`、換 session 或重啟；`decisions = ask` 第一次仍會問，使用者答 always 後該前端就不再問；reason 文字判斷是因為請求沒有有效等級的權宜之計。
  - 驗證：CLI 4 個單元測試（含順序）、Web 3 個單元測試與 1 個整合測試（WebSocket），瀏覽器測試（真實瀏覽器）通過；用 llama.cpp 實測：要求建立兩個檔案、只輸入一次 `a`，只出現一次提示，兩個檔案都建立。既有整合測試的提示字串因此更新為 `Allow? [y/N/a=always]`。


## 7. Session 內檔案還原（推測，需設計）

- 現象：寫入工具改檔後沒有還原機制（未在測試中確認）。
- 限制：不能暗示可以回滾外部效果（AGENTS.md）；只能還原 RAH 自己寫過、且內容仍與寫入結果一致的檔案。
- 先決：先寫設計說明與 ADR，再決定是 plugin 還是 hook。
- 結果（2026-10-29，完成，ADR 0060）：
  - 設計：不能另做 plugin 掛在 `tool.before_invoke` 取「寫入前」的內容（那是 policy 的 bail 鏈，掛上去會讓 `is_guarded` 變真，且 policy 回 `Allow` 後後面的 handler 不會跑）；`tool.after_invoke` 又太晚。所以由 `tools-repo-write` 自己在檔案鎖內記錄每個寫入呼叫每個檔案的前後內容（缺檔記為缺檔），提供新服務 `file-undo`（`rah_api::undo`）。`ToolContext` 新增 `call`，讓同一 plugin 在 `tool.after_invoke`（優先序 -100，最後跑）依呼叫 id 重讀檔案，`format-on-write` 改寫的結果也算在這次變更裡。
  - 行為：只有使用者能還原（CLI `/undo`、`/changes`；Web 的 Undo 按鈕、`/undo`、`GET/POST /api/sessions/{id}/changes|undo`），不是工具，模型叫不到。每次還原 session 最新的一筆，全部或全不：任何檔案與變更留下的不同就什麼都不寫、該筆移出紀錄（Web 409），較舊的仍可還原。下一輪的輸入前面會加一段給模型的說明，所以寫進 session log、replay 一致。紀錄在記憶體，每 session 50 筆、總共 64 MiB，重啟或重新設定就忘；`repo.commit` 與 `shell.exec` 等其他工具不涵蓋。
  - 驗證：`tools-repo-write` 新增 6 個測試（各寫入工具依新到舊還原、被改過就拒絕且只丟該筆、formatter 改寫後仍可還原、patch 還原與過大檔案不記錄、numstat 解析、上限）；`rah-api` 2 個；`end_to_end.rs`（CLI）、`web.rs`（HTTP 與 WebSocket，含 404、409 與模型收到說明）、`web_browser.rs`（Edge 實測按鈕）各 1 個。用 llama.cpp 實測 CLI 與 `--web`：模型改檔、使用者還原、下一輪模型重讀並回報原內容，session log 的輸入前面有說明。

## 8. 補驗證（檢查時沒測到）

- `--web` 搭配 llama.cpp 的實際對話與核准流程。
- 功能開關 `compact`、`lsp`、`format`、`plan` 搭配真實模型。
- `rah-desktop` 與 WASM plugin。
- `shell.exec` 在 `sandbox-os`（Windows 低完整性）下的實際行為。
- 這些只有單元測試層級的覆蓋；有問題再開條目。已知的間歇性瀏覽器測試問題見 `docs/KNOWN_ISSUES.md`。
- 結果（2026-10-19，進行中；用 llama.cpp 實測）：
  - 五個功能開關（`format`、`lsp`、`plan`、`compact`、`telemetry`）都能正確掛載並出現在 `--show-config`。
  - **找到並修好一個缺陷**：`rah --root X --with format`（`lsp` 同樣）因為 `--root` 先於 `--with` 套用，功能新掛載的 `format-on-write`、`lsp-diagnostics` 沒拿到 `root`，把目前目錄當工作區，檔案不在其下就不處理；預設格式化程式失敗是靜默的，所以看不出來。改為先套用 `--with`（`rah-desktop` 本來就是這個順序），補了 `root_reaches_the_workspace_plugins_that_features_mount`（`crates/rah-cli/tests/binary.rs`）。修好後 llama.cpp 實測：`repo.create-file` 寫入難看的一行 Rust，`rustfmt` 自動格式化，工具輸出告訴模型檔案已變。（負向驗證：修之前 `--show-config` 的 `format-on-write` 沒有 `root`，從工作區目錄執行則正常；沒有把修正還原來看測試失敗。）
  - 環境因素（非缺陷）：沙箱清空環境變數，這台機器的 `rustup` 在 `F:\rust`（自訂 `RUSTUP_HOME`、`CARGO_HOME`），所以 `rustfmt` 卡住；要在沙箱設定 `inherit_env_extra`。README 已補說明。
  - `lsp` 的正向路徑沒能驗證：這台機器的 stable 工具鏈沒有安裝 `rust-analyzer` 元件（明確設定伺服器後看到 `Unknown binary 'rust-analyzer.exe'`）。錯誤處理本身正確。依 AGENTS.md 不替使用者安裝系統相依。
  - 還沒驗證：`plan`、`compact`、`telemetry` 的實際執行，`--web` 搭配 llama.cpp，`rah-desktop`，WASM，`sandbox-os`。
- 結果（2026-10-29，完成；llama.cpp `ornith-1.5-35b`）：
  - `plan`、`telemetry`：三輪對話答案正確；telemetry 寫出 3 筆 turn、10 筆 model request、1 筆 tool。觀察（非缺陷）：`loop-plan` 每一步都記一則 assistant 訊息，這個模型在每步都重複同一個答案，CLI 因此把第一題的答案印了四次；先記下，有需要再調整 loop-plan 的步驟輸出。
  - `compact`：`trigger_chars = 1500` 時歷史沒超過門檻（約 1000 字元）所以沒觸發；改為 300 後在 `storage` 寫出兩份摘要，第三輪靠摘要正確答出使用者名字與第一個數值。
  - `--web`：用 WebSocket 跑一輪改檔（核准由 socket 回答），再呼叫 undo 路由（200，然後 404），下一輪輸入帶到說明，模型重讀並引用原文。
  - `sandbox-os`（Windows Low integrity）搭配 `shell.exec`：`git --version` 正常；`cmd /c echo` 寫在 root 內成功，寫到 root 外得到 `Access is denied.`，檔案沒有產生。
  - `rah-desktop`：以 default 預設啟動，視窗標題 `RAH 0.45.0`，內嵌的 `frontend-web` 在 loopback 監聽，stderr 無錯誤；沒有在原生視窗裡實際跑一輪（它內嵌的就是上面驗證過的 Web UI）。
  - WASM：樹內沒有現成的 `.wasm` 範例模組（host 的測試自行產生），沒做實機驗證；單元與整合測試涵蓋。

## 9. 預設格式化程式與語言伺服器的失敗看不見（新增，建議）

- 現象：預設的 `rustfmt`、`rust-analyzer` 失敗時完全靜默（設計：沒安裝是常態），但「功能開了卻什麼都沒做」很難診斷；這輪因此花了多步才找到原因（自訂 `RUSTUP_HOME`、缺少元件）。
- 方向：每個預設程式第一次失敗時，在 stderr 或工具輸出附一次性的簡短提示（只提一次，之後的靜默維持）；或讓 `--show-config` 檢查程式是否可執行。不改變「沒安裝就跳過」的預設。
- 先決：決定提示放哪裡（工具輸出會進模型的上下文，stderr 不會），屬使用者決定。
- 結果（2026-10-19，完成，ADR 0057）：使用者選「stderr 一次性提示」。`rah_api::host` 新增事件 `HostNotice {plugin, message}` 與 `NoticeOnce`（每個鍵只發一次）；`format-on-write`、`lsp-diagnostics` 在預設程式第一次失敗（無法啟動、逾時、非零結束）時發出。`rah` 印在 stderr，Web UI 顯示為通知（`rah-desktop` 內嵌 Web UI，所以也看得到）。工具輸出與模型的上下文不變；使用者明確設定的程式仍照舊在輸出裡回報。訊息把處理方式放前面、原因放後面（長原因會在 300 字元被截）。
  - 驗證：`NoticeOnce` 單元測試；format-on-write 2 個新測試、lsp-diagnostics 擴充 1 個；Web 整合測試確認瀏覽器收到 `host_notice`。實測重現當初的情況（沙箱裡 `rustfmt` 找不到工具鏈）：stderr 現在出現一行說明哪個程式失敗、原因（rustup 找不到預設工具鏈），以及怎麼處理。
  - 沒做：`rah-desktop` 的原生視窗沒有另外的 stderr 輸出（靠 Web UI 通知）；process plugin 還不能發這個事件。


## 狀態

| 項 | 類型 | 需要 ADR | 狀態 |
| --- | --- | --- | --- |
| 1 拒絕後不重試 | 實測缺口 | 否（只改文字） | 完成 |
| 2 輸出順序 | 實測缺口 | 否 | 完成 |
| 3 非互動核准選項 | 實測缺口 | 是（0054） | 完成 |
| 4 推理內容回送 | 文件缺口 | 是（若做） | 已量測，暫不實作（等實際失敗） |
| 5 刪除、改名、commit | 已確認 | 是（0055） | 完成（v0.45.0） |
| 6 session 內一律允許 | 已確認 | 是（0056） | 完成（v0.45.0） |
| 7 檔案還原 | 推測 | 是（0060） | 完成 |
| 8 補驗證 | 驗證 | 否 | 完成（WASM 無範例模組，未實機驗證；lsp 正向路徑缺 rust-analyzer） |
| 9 預設程式失敗看不見 | 建議 | 是（0057） | 完成（v0.45.0） |

建議順序：1、2 先做（小、可獨立測試、不碰架構）；8 可穿插進行；3、6 共用核准路徑，一起設計；4、5、7 等量測或決定後再動。
