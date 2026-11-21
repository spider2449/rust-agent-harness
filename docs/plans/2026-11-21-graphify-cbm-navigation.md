# Graphify 補強與 CBM 整合計畫

## 目標與基準

讓 RAH 使用者能從圖上定位 Python／Qt 的接線、成員存取和跨檔目標。Graphify 負責呈現逐位置的原始碼證據。CBM 作為可選的符號與關係查詢來源，供 agent 找候選並回查原始碼。維持 Graphify 目前的實驗性定位，待實際專案驗收後再評估預設啟用。

- Graphify 基準為 RAH 管理的 `graphifyy[mcp]==0.9.107`。現有補強能處理單一 constructor callable 賦值及部分跨檔 constructor alias。已回報的 Qt 接線、property 和成員讀寫缺口見 [KNOWN_ISSUES](../KNOWN_ISSUES.md#graphify-incomplete-python-and-qt-relationships)。
- CBM 基準為 Windows v0.12.1。隔離的 RAH 0.44.0 試點已透過現有 stdio MCP 查詢符號、呼叫路徑、片段及邊屬性。合成樣本中 `unique_name` 曾將 `self.ik2fk()` 誤連到無關類別，`trace_path` 回傳目標時省略了解析策略。
- napari `3ce2a1cb12d40fdaffc3dcf9fb6575320ad834ba` 的 `_qt` 共掃描 294 個 Python 檔案與 433 個 `.connect()`。335 個可判定的同類別呼叫點對應 296 組方法與 callback 關係。CBM 返回 291 組正確的 `CALL_REFERENCE`，其餘 5 組可查到正確目標的 `USAGE`。7 個巢狀 `self` callback 只找到 1 條正確引用。這批是靜態 AST 對照，範圍也包含一般事件物件。

## P0 建立可重跑的基準

- 將暫存試驗的合成案例縮成離線 fixtures，固定原始碼位置和預期目標。涵蓋同一 slot 多個 signal、多行接線、同名干擾、繼承、重複賦值、跨檔 alias、property、成員讀寫、索引後 receiver 和非 Qt 的同名 `connect()`。
- 用現行 managed Graphify 和 CBM 跑同一組 fixtures，記錄來源檔案與行號、關係類型、正確目標、漏連、誤連、重複合併與查詢耗時。普通測試使用 fixtures 和假 MCP。真實二進位與 napari 批次評測獨立執行並記錄版本。
- 固定 napari 的案例清單與判分方法。分別計算接線呼叫點、解析出的目標、錯目標與未解析。報告分母及未判定原因。使用者的原始 Python／Qt 專案可取得時，加入同樣的離線驗收。

**驗收**：同一份案例能重複產出可比較結果，並重現 CBM 對 callable 屬性的誤連及多接線點合併。

### P0 實測基準

重跑方式、固定案例與判分規則見 [Graphify 評測說明](../../examples/graphify/README.md#python-and-qt-navigation-baseline)。使用 Windows x64、managed Graphify 0.9.107，以及 fast 模式的 CBM 0.12.1。CBM 執行檔 SHA-256 為 `39f6a789cd858d7428561ac5afaefb9a47b018dcba0ad6fa41ae71a45e0ed656`。

| 資料集 | 來源分母 | Graphify | CBM |
| --- | --- | --- | --- |
| 合成 fixtures | 12 個位置，10 個有預期目標，其中 5 個為接線 | 命中 3 個位置，漏 7 個，誤連 0 個。3 個命中為 callable alias、property 定義及 property 讀取 | 命中 5 個位置的目標，漏 4 個，誤連 1 個。4 組正確的接線關係均無行號 |
| napari `_qt` | 294 檔、433 個接線位置。330 個位置有可判分目標，對應 291 組關係 | 漏 330 個位置的目標 | 330 個位置均找到正確目標，無誤連。291 組均有 `CALL_REFERENCE`，其中 2 組另有 `USAGE`。位置命中為 0 |

CBM 在 `window.py:18` 以 `unique_name` 及信心值 0.38 把 `self.ik2fk(partI)` 指到 `Decoy.ik2fk`。正確目標是 `Window.convert`。兩個 signal 在 `window.py:36`、`window.py:37` 連到同一 slot，CBM 只提供一條沒有行號的 `CALL_REFERENCE`。

napari 的 103 個未判分位置包含 90 個同類別規則以外的 callback 形式，以及 13 個缺少唯一、無裝飾器的同類別方法宣告。此規則排除了前次試驗納入的 5 個裝飾方法呼叫點，因此使用 330 作為此次分母。使用者原始 Python／Qt 專案仍待納入。

| 單次量測（秒） | AST 來源掃描 | Graphify 抽取 | Graphify 本機圖檔讀取 | Graphify MCP 範例查詢 | CBM 冷索引 | CBM CLI 關係查詢 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 合成 fixtures | 0.015 | 8.265 | 0.012 | 0.009 | 2.466 | 2.037 |
| napari | 1.016 | 75.553 | 0.753 | 0.016 | 9.266 | 1.636 |

以上為新暫存目錄的 wall-clock 量測，期間同機亦在執行倉庫檢查。Graphify 圖檔讀取包含解析及關係欄位整理，MCP 範例查詢自初始化後計時，查詢詞分別為 `menuHelp_triggered` 和 `QtViewer`。CBM CLI 時間包含程序啟動及批次關係查詢。重複評測的命中、漏連及誤連計數一致。

## P1 Graphify 保留逐位置的接線證據

- 在現有 Python AST 抽取處記錄可辨識的 `signal.connect(slot)`，保存相對檔案、行列、signal／slot 表達式及所在符號。有 Qt 型別或匯入依據時標為 Qt 接線候選，其餘辨識出的 `.connect()` 使用一般事件接線候選。節點與關係帶來源，供人員查閱原文。
- 每個呼叫點有獨立識別。先用 fixture 確認 Graphify 匯出及 RAH 消費端都能區分同一對端點上的多個呼叫點。若上游圖合併平行邊，使用版本化的接線證據欄位或獨立接線位置節點，並走既有驗證與原子發布流程。
- Desktop／Web 沿用 [ADR 0079](../adr/0079-declarative-relationship-graphs.md) 的文字欄位與圖上限。提供可複製的來源位置、證據和省略數。Managed MCP 維持目前的工作區綁定、刷新與資料上限。

**驗收**：合成案例中的每個接線位置均可查回，同一 slot 的兩個接線位置分別顯示，不支援的 callback 形式保留可查的原文。Qt 與一般同名案例的分類符合預期。舊圖、重建失敗、工作區切換及既有視圖測試通過。

P1 採用獨立的接線位置節點。固定版本的 Graphify 匯出測試涵蓋 5 個原有接線位置與 2 個同起點的鏈式呼叫，保留各自的節點和來源位置。關係視圖可用 callback 表達式搜尋，測試涵蓋同一 slot 的兩處接線。資料欄位見 [Graphify 關係說明](../../examples/graphify/RELATIONS.md#python-connection-evidence)。

## P2 Graphify 補強 Python 目標與成員存取

- 沿用現有 constructor 賦值與 import alias 邏輯，先處理 `self.infoI.selSwCtl_trd` 與 `self.ik2fk(partI)`。要求類別與候選目標唯一，遇到多次賦值、候選衝突或未知 receiver 時標示未解析，保留呼叫位置。
- 區分 property 定義、成員讀取、成員寫入及呼叫。驗證 `switchValue` 讀取與 `isSetKey` 寫入。先限於 AST 可證明的存取位置與綁定。索引後的物件鏈只在元素型別和目標可判定時列入後續增量。

**驗收**：針對 P0 具可判定目標的案例逐筆比對。任何同名誤連都阻擋該解析規則啟用。讀取、寫入與呼叫有各自的關係及位置。重跑 napari，公開命中、漏連與誤連計數。

P2 的合成案例命中 10／10 個可判分位置，其中 5 個為 callback。未觀察到誤連。索引後 receiver 和重複賦值的 callable 保留未解析的來源位置。

在固定版本的 napari `_qt` 重跑，433 個接線位置中有 330 個可判分。330 個位置全部命中，共 291 組關係，漏連與誤連各為 0。103 個未判分位置中，90 個超出同類別規則，13 個缺少可唯一識別且無裝飾器的方法。此次 Windows 擷取耗時 49.118 秒，MCP 範例查詢耗時 0.016 秒。

## P3 加入可選的 CBM MCP 設定

- 以 v0.12.1 為初始實測版本，提供可重跑的設定範例與文件，使用現有 `tools-mcp`、`sandbox-process` 和 `policy-authority`。以已核對 hash 的二進位絕對路徑和隔離快取啟動。首版由使用者顯式準備二進位、建立索引及啟用設定。
- 開放 `search_graph`、`trace_path`、`get_code_snippet` 和唯讀 `query_graph`，逐項賦予 `read`。索引建立及其他寫入操作留給明確的使用者命令。由受監督的啟動器或既有可配置的約束機制綁定工作區所屬的索引，檢查來自 model 的 `project` 並限制回傳大小。切換工作區時重新綁定，缺少索引或無法確認索引歸屬時回傳明確錯誤。
- 把 `unique_name` 等低證據結果視為待核對的候選。關係查詢保留 CBM 提供的策略、信心值和行號。需要接線位置時回查 Graphify 的位置證據或原始碼。記錄索引來源、更新方式及新鮮度限制。

**驗收**：用 mock model 經 `ToolInvoker` 逐項呼叫 4 個工具。其他工具未對 model 暴露。跨工作區、未知 `project`、缺少索引、超大輸出及停止程序均有確定結果。Windows 用真實 CBM 重跑 napari 案例並檢查結果與直接 CLI 一致。其他平台逐一記錄驗證結果。

P3 的 [Windows 設定範例](../../examples/mcp/CBM.md) 使用既有的 MCP、程序監督與權限插件。Windows x64 已以 CBM 0.12.1 在固定版本的 napari 驗證四個查詢工具。關係查詢的三筆樣本與同索引的直接 CLI 結果一致。macOS 與 Linux 的 CBM 執行檔與程序生命週期仍待驗證。

## P4 整合驗收與發行決策

- P0 先完成。P1 與 P3 可各自推進。P2 依賴 P1 先固定接線位置與來源證據的表示方式。P4 整理兩條線的結果後再決定是否提供安裝命令、離線套件或自動索引。
- 用同一批案例量測 Graphify、CBM 和原始碼搜尋。至少分開報告接線位置、目標命中與誤連、冷索引與單次查詢耗時。記錄測量條件。真實專案的結果缺少時保留試驗定位。
- 依階段執行相關 Graphify Node／Python 與 RAH MCP 測試。階段結束按倉庫規則執行 `cargo fmt --check`、`cargo check --workspace`、`cargo test --workspace --no-fail-fast`、`cargo clippy --workspace --all-targets --all-features -- -D warnings` 及 `git diff --check`。持續存在的缺失記入 `KNOWN_ISSUES.md`。

### P4 驗收結果

使用 Windows x64、Python 3.14.6、managed Graphify 0.9.107、CBM 0.12.1 fast 與 ripgrep 15.2.0。兩組資料各自使用新的 Graphify 輸出目錄及隔離的 CBM 快取與 daemon 目錄。報告可用 [評測指令](../../examples/graphify/README.md#python-and-qt-navigation-baseline) 重跑。

| 案例 | Graphify | CBM | ripgrep |
| --- | --- | --- | --- |
| 合成案例 | 保留 5／5 個接線位置。10／10 個可判分目標命中，無漏連或誤連。其中 5 個接線目標都有位置。 | 10 個可判分目標中，命中 5 個 callback、漏 4 個、誤連 1 個。接線目標均無位置。 | 找到 5／5 個接線名稱所在行，無多餘命中。輸出為來源文字。 |
| napari `_qt` | 保留 433／433 個接線位置。330 個可判分目標全部命中並附位置，無漏連或誤連。 | 330 個可判分目標全部命中，無漏連或誤連。接線目標均無位置。 | 找到 433／433 個接線名稱所在行，無多餘命中。輸出為來源文字。 |

合成案例有 2 個目標超出判分範圍。CBM 將 `self.ik2fk(partI)` 經 `unique_name` 誤連至同名干擾方法。napari 有 103 個接線目標未判分。對 napari 的兩項圖工具，冷索引計時涵蓋完整 checkout，接線判分限於 `_qt`。ripgrep 的位置以 `connect` 名稱所在行為準，Graphify 的位置則以呼叫起點的行列為準。

| 資料集 | Graphify 冷擷取 | CBM 冷索引 | Graphify MCP 單次查詢 | CBM CLI 單次查詢 | ripgrep 單次查詢 | CBM 批次關係查詢 | ripgrep 接線掃描 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 合成案例 | 0.840 s | 1.872 s | 0.007 s | 1.093 s | 0.020 s | 1.333 s | 0.117 s |
| napari | 48.534 s | 4.757 s | 0.010 s | 1.071 s | 0.020 s | 1.334 s | 0.026 s |

每個數字為同機單次 wall-clock 觀測，量測期間也執行倉庫驗證。單次查詢分別以 `menuHelp_triggered` 和 `QtViewer` 為搜尋詞。Graphify MCP 在初始化完成後計時。CBM CLI 和 ripgrep 計入程序啟動。三者的回傳內容不同，表中耗時反映各自的操作流程。

**發行決策**：維持 Graphify 的可選試驗定位，CBM 以 Windows x64 設定範例供手動啟用。此次暫緩 CBM 安裝命令、離線套件、自動索引及預設啟用。原始使用者專案與其他平台仍待驗證。41 項 Node 測試、9 項 Python 測試、mock 模型工作區切換、Rust 格式與全工作區檢查及測試通過。後續修正 Web 工作區 fallback 的 lint 後，Windows 全工作區嚴格 Clippy 通過。

本計畫使用既有插件與檢視契約。若實作需要改動 kernel、服務契約或協議，先依倉庫規則審查 ADR 0036 和 pivot plan，再另行決定變更範圍。
