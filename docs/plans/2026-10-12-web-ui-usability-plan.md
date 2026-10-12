# Web UI 可用性改善計畫

2026-10-12。使用者接上 llama.cpp 後回報 Web UI 太簡陋:session 沒有名稱、不能改名或刪除，工具和其他設定只能編輯 JSON，也看不到目前的 repo 和分支。分三段做，每段一個 commit。

## U1:Session 管理

- [x] `SessionInfo` 加 `title`、`updated`、`preview`;`SessionLog` 加 `rename`、`rename_if_untitled`、`delete`，以及 `SessionAppended` 事件。
- [x] `session-jsonl`:標題是附加的 metadata 行(不改寫紀錄);刪除時移到 `deleted/`，有 fork 的 session 拒絕刪除。
- [x] 新 plugin `session-titles`:第一輪對話完成後請模型產生標題，失敗時用第一句訊息。
- [x] Web UI:清單依最後活動排序、按日期分組、搜尋、改名、刪除;對話標題列可以改名、匯出(Markdown／JSON)、刪除;編輯重送和重新產生都用 fork 實作，保留原本的 session。
- [x] CLI:`/rename`，`/sessions` 顯示標題。
- 決定:命名用模型摘要;刪除移到 `deleted/` 資料夾，不是永久刪除;不做 Markdown 呈現和複製按鈕(使用者沒選)。

## U2:狀態列

- [x] 顯示工作目錄、git 分支(和是否有未提交的變更)、目前的 model 和 loop plugin、policy 狀態。
- 決定:`ExecEnv` 新增 `root()`(只用於顯示);git 狀態由 `frontend-web` 透過 `sandbox` 服務執行唯讀的 `git status` 取得，不經過 tool，也不直接開程序。沒有 sandbox 時，狀態列就不顯示工作目錄。

## U2b:切換工作目錄

- [x] 點狀態列上的路徑開啟資料夾選擇視窗(輸入路徑、上一層、磁碟、最近用過、子資料夾，git repo 有標記)，選定後不必重啟。
- 決定:由 distribution 實作 `PluginAdmin::set_workspace`，使用和 `--root` 同一份 plugin 清單;session、記憶和 skills 不跟著換目錄。

## U2c:Session 屬於工作目錄

- [x] Session 記錄建立時的工作目錄(header 的 `workspace`，由 `session-jsonl` 從 sandbox root 取得;fork 沿用父 session)。清單預設只顯示目前資料夾，另有「All folders」;打開或在別的資料夾的 session 裡送出訊息時，先自動切換工作目錄。
- 決定(使用者選擇):集中存放、依目錄篩選，不把 session 放進各 repo 的 `.rah/`。
- 決定(使用者選擇):記憶跟著工作目錄(`context-default` 的 `memory_dir`，集中在 `.rah/memory/`，每個資料夾一個檔);skills 維持全域共用。
- 起因:切換工作目錄後沿用同一個 session，模型看到的歷史紀錄是 A 專案，工具卻在 B 專案執行。
- 同時修正:sandbox 沒有傳 `HOME`/`USERPROFILE`，git 讀不到 `~/.gitconfig`，`safe.directory` 失效。

## U3:設定頁

- [x] 工具權限:列出每個工具，用表單設定 allow／ask／deny，寫回 `policy-authority` 的設定。
- [x] 其他設定:system prompt(`context-default`)、模型位址和參數(`model-llamacpp`／`model-openai`)、loop 參數，用表單而不是 JSON。
- 決定:表單由 UI 為內建 plugin 寫死(`assets/settings.js`)，不另做 plugin 設定 schema;其他 plugin 繼續用 Plugins 面板的 JSON。每個表單只改自己的欄位，其餘設定保留。
- 決定:原本的 policy 只能指定等級，而工具宣告的等級只能讓決定更嚴格，表達不了「允許某個寫入工具」或「關掉萬用字元裡的某一個工具」，所以 `policy-authority` 新增 `decisions`(由主機設定的單一工具決定)。
