# 降低操作難度

2026-11-06。依據 ADR 0067。目標：使用者只需要知道一份 `rah.toml` 和四個概念（模型、權限、工作目錄、features），不用知道 plugin 的組成方式。kernel、`rah-protocol` 與 plugin 協定不變；只有第 4 項改 `rah-api`（`PluginAdmin::persist`）。

依下列順序做，每項一個 commit，各自通過完整驗證後再做下一項。

狀態：五項皆已完成（2026-11-06）。與原計畫不同之處：舊旗標依決定直接刪除、不留 alias；第 4 項的 `ConfigChange` 改為 `Model`、`Plugin`、`Removed`、`Feature` 四種（見 ADR 0067 §5）；Plugins 面板的停止與啟動視為執行期切換，不寫入檔案。

## 現況問題

| # | 問題 | 對應項目 |
| --- | --- | --- |
| 1 | 設定來源有五層，Web UI 存在 storage 的設定會蓋掉 `rah.toml` | 1、4 |
| 2 | `default` 跑 llama.cpp，換供應商要換 preset 加環境變數，可能還要 `--model` | 1、3 |
| 3 | `--help` 有 21 個旗標，多數只對單一 plugin 有效 | 2 |
| 4 | `--show-config` 列出 19 個 plugin 和 JSON | 2 |
| 5 | `--approve` 要寫完整工具名稱 | 1 |
| 6 | README 660 行，CLI、Web、Desktop 的選項不一致 | 2、5 |

## 1. `rah.toml` 的使用者鍵

- `Distribution` 新增 `model`、`model_options`、`root`、`permissions`（`features` 已存在）。
- 有任何使用者鍵時，依 ADR 0067 §3 組合：guarded 基底 → 模型 → 權限與 root → `[[plugin]]`/`remove`（隱含 extends，沿用 `extend.rs`）→ features。沒有使用者鍵的檔案行為不變。
- 供應商對照表（`anthropic`、`openai`、`deepseek`、`llamacpp`、`mock`）寫成一個常數表，`--model` 與檔案共用；`main.rs` 現有的四個 kind 迴圈刪掉。
- `permissions` 編成 policy 的 `allow`/`ask`；`full` 每次啟動警告。`--permissions` 旗標覆寫一次。
- `--model` 改為 `<provider>/<id>`，同時決定 model plugin。`--preset` 在第 2 項刪除。
- 測試：每種供應商產生的 plugin 與設定；四種權限模式的 policy 設定，以及 `full` 下未指派的工具仍被拒絕；使用者鍵加 `[[plugin]]` 修改；舊格式檔案不變（現有測試照跑）；未知供應商與未知權限模式在解析時被拒絕。

## 2. 子命令與精簡輸出

- clap 改為子命令：`run`、`web`、`sessions list|export`、`config show|check|init|edit`、`features`。全域旗標：`--model`、`--permissions`、`--root`、`--with`、`--config`、`--data-dir`。
- 舊旗標全部刪除，不留 alias：`--preset`、`--prompt`、`--web`、`--export`、`--show-config`、`--check-config`、`--init-config`、`--list-features`、`--approve`、`--session-dir`。`binary.rs` 等測試與 `scripts/` 改用新形式。
- `rah config show` 預設只印使用者鍵與來源（檔案、旗標、預設值），`--plugins` 才列 plugin 清單。
- `rah sessions list` 是新功能（目前只能從 Web UI 看）：列出 id、標題、時間、工作目錄。讀 session 目錄，不啟動模型。
- `rah-desktop` 接受相同的全域旗標（共用一個 clap 結構，放在 `rah-cli` 的 lib）。
- 測試：每個子命令一個 binary 測試；舊旗標被 clap 拒絕。

## 3. 第一次啟動

- 沒有 `rah.toml`、`--config` 與 `--model` 時，不再默默跑 llama.cpp。
- stdin 與 stdout 都是終端機時執行 `rah config init` 互動流程：列出供應商，標記哪些已設定 key 的環境變數、本機 llama.cpp（`127.0.0.1:8080`，短逾時）是否有回應；問模型 id（各供應商給預設值）、權限模式、工作目錄；寫出只有使用者鍵的 `rah.toml`。
- 非終端機：退出碼 2，訊息寫明 `--model anthropic/<id>` 與 `rah config init` 兩種做法。
- 不碰任何 key 的值：檔案只記環境變數名稱，不寫入、不讀出 key。不修改系統環境變數。
- `rah config init --model ... --permissions ...` 可以無互動地寫檔（CI 與腳本用）。
- 測試：互動流程以注入的輸入與輸出測試（不依賴真的終端機）；非終端機的錯誤訊息與退出碼；llama.cpp 探測用本機假伺服器。

## 4. Web UI 設定寫回 `rah.toml`（ADR 0067 §5）

- `rah-api::admin`：新增 `ConfigChange` 與 `PluginAdmin::persist`，預設實作回傳「不支援」，讓其他 admin 實作不受影響。
- `rah-cli` 的 distribution plugin 實作 `persist`：用 `toml_edit` 修改檔案，保留註解與順序，寫入暫存檔後改名。沒有使用者鍵的檔案或唯讀檔案拒絕並說明原因。
- `frontend-web`：settings 的四個 slot、web search 開關、Plugins 面板的設定修改、ADR 0066 的「從此允許此指令」改成先套用、生效後呼叫 `persist`。`settings::restore`、`panel::restore` 的套用設定部分刪除；modes、groups、preferences 的 UI 狀態保留在 storage。
- 遷移：啟動時若在 `storage/frontend-web/` 找到舊的 `settings-*` 或面板記錄，發出 `HostNotice`；`rah config migrate` 把它們轉成 `ConfigChange` 寫入檔案後刪除。
- 測試：每種 `ConfigChange` 對檔案的修改（含註解保留）；Web UI 存檔後重新啟動，設定來自檔案；舊 storage 設定不再生效且有提示；migrate 前後 `config show` 一致；寫檔失敗時執行中的設定維持、回報錯誤。
- 文件：`SECURITY.md` 註明 Web UI 可以修改 `rah.toml`（僅限 loopback，與現狀的信任等級相同）。

## 5. 文件

- README 重寫成一頁：安裝、`rah config init`、`rah`、`rah web`、一份最小 `rah.toml`。其餘移到新的 `docs/CONFIGURATION.md`（使用者鍵、權限模式、features、進階 `[[plugin]]`、組合順序）。
- `--help` 與 `rah config init` 寫出的檔案註解指向 `docs/CONFIGURATION.md`。
- `CHANGELOG.md` 列出每個被刪除的旗標與對應的新寫法。
- ADR 0067 改為 Accepted；ADR 0058 與 0066 標註被取代的部分。

## 不在範圍內

- 減少預設掛載的 plugin（例如把 `plugin-builder`、`scheduler-basic` 改成 feature）。這是功能範圍的問題，另案討論。
- 把 API key 存進 RAH 自己的檔案或系統 keyring。
- 改動 kernel、`rah-protocol` 或 plugin 協定。
