# RAH Plugin-Kernel Pivot Plan ("Everything is a plugin")

- 日期：2026-10-11
- 狀態：**草案，待人審核**（審核通過後，P0 會轉成 ADR 0036 並重寫 AGENTS.md）
- 路線：**C（混合）**——新的 plugin kernel 為核心；舊 RAH 的授權模型保留成一個*可選*的 policy plugin
- Legacy 基線：tag `v0.35.0`、branch `legacy/authority-first`

## 1. 背景與決策

RAH 的初衷是「**Rust 版的 DeepSeek Harness**」。DeepSeek Harness（dsh）建在 Cordis
plugin 框架上：kernel 只負責 plugin 的掛載、卸載和依賴，model adapter、tool registry、
agent loop、session、sandbox、storage、scheduling、UI 全部都是可替換、可重組的 plugin；
session 是 append-only log，可以 resume、fork、replay。

但 RAH 從 2026-08-20 第一個 commit（架構 guardrails）起，就走成了 **authority-first**
架構，與初衷不同：

- 擴充點是固定的 trait（`AgentRuntime`、`ModelBackend`、`Tool`、`SessionStore`、`Sandbox`），
  沒有 plugin 生命週期，也不能在執行時掛載。
- Provider 在**編譯時**用 Cargo feature 選擇。
- **Agent loop 寫死在每個 runtime adapter 內**（`rah-runtime-openai::Job::run`），無法替換。
- 大約 115k 行 Rust 中，`rah-desktop` 佔 ~53.5k、`rah-tools`（以 repository 修改權限工具為主）
  佔 ~37k、`rah-runtime-codex` 佔 ~11.5k；kernel 和 plugin 機制則是 0 行。
- 35 份 ADR 和 AGENTS.md 把「host 擁有權限、provider 不能 bypass、不可有通用 shell」寫成硬性原則，
  直接阻擋 everything-is-plugin。

決策：

1. 以 **v0.35.0** 作為 authority-first 架構的最終版本並發布，標註為 legacy line。
2. `master` 轉向 plugin kernel。舊程式碼保存在 tag 和 legacy branch，**不在 master 上維持相容**。
3. 採路線 C：保留舊授權模型最有價值的部分（模型輸出不可信、不自動重放副作用、
   權限 fail-closed），但從「kernel 內建的硬約束」改成「預設掛載的 policy plugin」。

## 2. 設計原則

| # | 原則 | 說明 |
|---|---|---|
| P1 | **Everything is a plugin** | model、loop、tools、session、storage、sandbox、policy、prompt/context、skills、scheduler、frontend 全部都是 plugin。kernel 不包含任何 agent 領域邏輯。 |
| P2 | **Kernel 極小** | kernel 只做：plugin 生命週期、service 註冊與注入、事件和 hook、scope 化的資源回收、設定。目標 < 2k 行（不含測試）。 |
| P3 | **以 service 介面耦合** | plugin 之間只透過 service 介面（`rah-api` 中的 trait）互相依賴，不直接依賴彼此的 crate。 |
| P4 | **替換 = 掛另一個 plugin** | 不需要 patch 核心。同一個 service 可以有多個實作，依設定選擇。 |
| P5 | **掛載即信任，模型永不被信任** | 使用者在設定裡掛上的 plugin 等同於 dependency（受信任的程式碼）；模型輸出和 provider metadata 永遠只是*請求*。這是路線 C 保留的核心安全觀。 |
| P6 | **副作用不自動重放** | session replay 是「重讀 log」，不是重新執行 tool。延續 legacy ADR 0033 的精神，改由 kernel 和 session 合約保證。 |
| P7 | **生命週期有明確擁有者** | 每個 task、子程序、連線都屬於某個 plugin scope。unmount 時一定回收，不留孤兒 task。 |
| P8 | **誠實的隔離聲明** | process supervision 不等於 OS sandbox；不宣稱沒有實作的隔離。 |

## 3. Kernel 設計（Rust 版 Cordis）

### 3.1 核心概念

```text
Kernel
 ├─ PluginRegistry   plugin 描述（manifest）與實例狀態
 ├─ ServiceTable     ServiceKey -> 多個具名實作（Arc<dyn Trait>）
 ├─ EventBus         typed 事件（fire-and-forget / parallel）
 ├─ HookChain        有優先序的攔截鏈（bail / waterfall / serial）
 ├─ Scope tree       每個 plugin 一個 Scope；dispose 時回收 effects、tasks、services、hooks
 └─ ConfigStore      每個 plugin 的設定（serde + JSON Schema），可熱更新
```

### 3.2 Plugin 生命週期

```text
Registered -> Pending(等待 requires) -> Starting -> Active -> Stopping -> Disposed
                   ^                                   |
                   └──── 被依賴的 service 消失 ─────────┘ （自動停止，service 恢復後重新啟動）
```

- `requires` 中的 service 全部可用時才會啟動。required service 被卸載時，依賴它的 plugin
  會連帶停止；service 恢復後再重新啟動（Cordis 的行為）。
- `optional` 依賴不阻擋啟動，但 service 變動時會通知。
- 熱替換：卸載舊 model plugin、掛上新的 → 依賴 `model` 的 loop 自動重新解析。

### 3.3 API 草案

```rust
// rah-kernel
pub struct PluginManifest {
    pub id: PluginId,                    // "model-llamacpp"
    pub version: semver::Version,
    pub kernel_api: semver::VersionReq,  // 相容的 kernel API 版本
    pub provides: Vec<ServiceName>,      // ["model"]
    pub requires: Vec<ServiceName>,      // ["storage"]
    pub optional: Vec<ServiceName>,      // ["policy"]
    pub config_schema: Option<serde_json::Value>,
}

#[async_trait]
pub trait Plugin: Send + Sync + 'static {
    fn manifest(&self) -> &PluginManifest;
    /// 在 scope 內註冊 service、hook、effect；回傳後即視為 Active。
    async fn apply(&self, cx: &mut Scope, config: serde_json::Value) -> Result<(), PluginError>;
}

impl Scope {
    pub fn provide<S: ?Sized + Send + Sync + 'static>(&mut self, key: ServiceKey<S>, name: &str, svc: Arc<S>);
    pub fn inject<S: ?Sized + Send + Sync + 'static>(&self, key: ServiceKey<S>) -> Result<Arc<S>, InjectError>;
    pub fn on<E: Event>(&mut self, handler: impl Fn(&E) + Send + Sync + 'static);
    pub fn hook<H: Hook>(&mut self, priority: i32, handler: H::Handler);
    pub fn spawn(&mut self, fut: impl Future<Output = ()> + Send + 'static); // dispose 時取消（CancellationToken）
    pub fn effect(&mut self, cleanup: impl FnOnce() -> BoxFuture<'static, ()> + Send + 'static);
    pub async fn plugin(&mut self, p: Arc<dyn Plugin>, config: serde_json::Value) -> Result<PluginHandle, PluginError>; // 巢狀 plugin
}
```

- `ServiceKey<S>` 是 typed key（內部 `TypeId` + 名稱），避免字串查表的型別錯誤。
- 所有 `inject` 拿到的都是 `Arc`；service 卸載時 kernel 會停止依賴者，因此不會出現懸空引用的語義問題。
- async runtime 固定用 tokio（RAH 現有程式碼已經全部基於 tokio）。

### 3.4 Hook（中介層）

Hook 是 everything-is-plugin 的關鍵。policy、logging、prompt 改寫、記憶注入都靠它，不靠改 core。

| Hook | 型態 | 用途 |
|---|---|---|
| `tool.before_invoke` | bail（Allow / Deny(reason) / Ask） | 權限、確認、速率限制 |
| `tool.after_invoke` | waterfall | 結果截斷、遮罩、記錄 |
| `model.before_request` | waterfall | system prompt、context、記憶、skills 注入 |
| `model.after_response` | waterfall | 過濾、統計 |
| `turn.before_start` / `turn.after_end` | serial | session 記錄、排程 |

### 3.5 Plugin 的三種載入形式

| 形式 | 機制 | 適用 | 階段 |
|---|---|---|---|
| **Static** | Rust crate 實作 `Plugin`，由 distribution（`rah` binary）連結並註冊 | 內建 plugin、效能敏感 | P1 |
| **Process** | JSON-RPC 2.0 over stdio（由 `rah-tools-plugin` 泛化成 protocol v2） | 任意語言、第三方 | P4 |
| **WASM** | wasmtime + WIT component | 可攜、可沙箱化的第三方 plugin | P6 |

**不採用** dylib／`abi_stable`：Rust 沒有穩定的 ABI，太脆弱（延續 ADR 0008 第 6 條的判斷）。

Process plugin protocol v2（由 v1 泛化）：

```text
host -> plugin  initialize {kernel_api, config}
plugin -> host  {manifest: provides/requires/optional, hooks:[...]}
host -> plugin  service/call {service, method, params}     # host 呼叫 plugin 提供的 service
plugin -> host  service/call {service, method, params}     # plugin 呼叫 requires 中的 service（只限 requires 清單內）
host -> plugin  hook/invoke {hook, payload} -> {result}
both            event/emit, $/cancel
host -> plugin  shutdown
```

### 3.6 設定與 Preset

```toml
# rah.toml
[[plugin]]
use = "model-llamacpp"
config = { endpoint = "http://127.0.0.1:8080" }

[[plugin]]
use = "loop-react"

[[plugin]]
use = "policy-authority"        # 路線 C：預設 preset 會掛
config = { allow = ["read"], ask = ["write", "execute"] }

[[plugin]]
use = "process:./plugins/my-tool"   # process plugin
```

- 內建 preset：`default`（含 policy）、`minimal`（不含 policy，啟動時明確警告 *unguarded*）、`dev`（mock model）。

## 4. Service 目錄（"everything" 的具體內容）

所有 trait 放在新的 `rah-api` crate（依賴 `rah-protocol`，不依賴 kernel 之外的東西）。

| Service | Trait 草案 | 預設 plugin | 來源 |
|---|---|---|---|
| `model` | `ModelProvider::stream(ModelRequest, Cancel) -> ModelEventStream`；只負責「一次模型請求」，**不含 loop** | `model-openai-responses`、`model-llamacpp`、`model-mock` | 拆自 `rah-runtime-openai`、`rah-model::mock` |
| `loop` | `AgentLoop::run_turn(TurnRequest, TurnCx) -> AgentEventStream` | `loop-react`（model → tool → model） | 拆自 `rah-runtime-openai::Job::run` |
| `tools` | `ToolProvider`（提供多個 `Tool`）＋ `ToolInvoker`（kernel 層級的呼叫入口，**一定經過 `tool.before_invoke` hook**） | `tools-builtin`、`tools-mcp`、`tools-repo` | `rah-tools`、`rah-tools-mcp` |
| `session` | `SessionLog`：append-only；`append`、`read`、`fork(at)`、`resume` | `session-jsonl` | 新寫；取代 `rah-session` |
| `storage` | `KvStore`、`BlobStore` | `storage-fs` | 新寫 |
| `sandbox` | `ExecEnv::spawn(ProgramSpec) -> SupervisedChild` | `sandbox-process` | `rah-sandbox` |
| `policy` | 不是 service，而是 hook 提供者 | `policy-authority` | `rah-tools::authorized_dispatch`、`PermissionLevel`、`experimental_host` 的 lease 語義 |
| `context` | `ContextBuilder`（system prompt、歷史視窗、記憶） | `context-default` | desktop 的 sliding replay window、work-log memory |
| `skills` | `SkillSource`（文字＋工具定義，不帶權限） | `skills-fs` | 新寫 |
| `scheduler` | `Scheduler`（排程、背景任務、subagent） | `scheduler-basic` | 新寫 |
| `frontend` | 不提供 service，消費 `loop`、`session` | `frontend-cli`、`frontend-web`（HTTP＋WS＋Web UI）、`frontend-tauri`（選配） | `rah-cli`、desktop frontend 資產 |
| `telemetry` | 事件訂閱者 | `telemetry-log` | desktop diagnostic log |

### 4.1 一個 turn 在新架構下的流程

```text
frontend-web --(TurnRequest)--> loop-react
  loop-react: hook model.before_request (context/skills/memory 注入)
           -> model.stream()            (model-llamacpp)
           -> 收到 ToolCall -> ToolInvoker.invoke()
                 -> hook tool.before_invoke  (policy-authority: Allow/Deny/Ask)
                 -> ToolProvider            (tools-repo / tools-mcp / process plugin)
                 -> hook tool.after_invoke
           -> 回到 model.stream() ... -> Completed
  所有 AgentEvent -> EventBus -> session-jsonl（append）、frontend、telemetry
```

### 4.2 Session log 格式（append-only JSONL）

```json
{"seq":1,"ts":"...","kind":"turn_started","turn":"t1","input":{...}}
{"seq":2,"kind":"model_request","turn":"t1","request":{...}}
{"seq":3,"kind":"tool_call","turn":"t1","call":{...}}
{"seq":4,"kind":"tool_result","turn":"t1","call_id":"...","output":{...},"effect":"none|completed|uncertain"}
{"seq":5,"kind":"turn_completed","turn":"t1","output":{...}}
```

- `fork(at=seq)`：新建一個 session，header 記錄 `parent` 和 `at`，不複製檔案。
- `replay`：重新渲染或重新送進 model context，**不重新執行 `tool_call`**（P6）。
- `effect: uncertain` 的紀錄要原樣保留，交給使用者判斷（延續 legacy 不自動重放的原則）。

## 5. 保留／改造／刪除對照表

圖例：**保留**＝搬過去並小幅修改；**改造**＝拆開或重包成 plugin；**刪除**＝從 master 移除（legacy branch 仍保存）。

### 5.1 Crates

| Crate（行數） | 處置 | 去向與理由 |
|---|---|---|
| `rah-protocol`（~690） | **保留** | ID、`Message`、`ToolDefinition`／`ToolCall`／`ToolOutput`、`AgentEvent` 保留；`live_evidence.rs` 刪除（Codex 和 desktop 認證證據）；新增 `ModelEvent`、session log entry 型別。仍維持 dependency-bottom。 |
| `rah-runtime-openai`（~2.5k） | **改造** | `sse.rs`、`protocol.rs`、`error.rs` 改成共用的 Responses 傳輸模組；`llamacpp.rs` 變成 `model-llamacpp`；`lib.rs` 的 HTTP 部分變成 `model-openai-responses`；`Job::run` 的 loop 邏輯抽出成 `loop-react`。`tests.rs` 的 fixture 測試沿用。 |
| `rah-runtime`（~2.3k） | **改造後刪除** | `experimental_host.rs` 的授權、lease、in-flight 上限、可修正錯誤分類 → `policy-authority` 和 `ToolInvoker`；`failure.rs` → `rah-api` 錯誤型別；`AgentRuntime`、`ConfiguredRuntimeFactory`、`RuntimeInstance`、`HostToolPort` trait 刪除（被 `loop` 和 `model` service 取代）；`minimal.rs` 刪除。 |
| `rah-model`（~330） | **改造後刪除** | `ModelBackend` 由 `ModelProvider` 取代；`mock.rs` 變成 `model-mock`。 |
| `rah-tools`（~37k） | **拆分** | 見 5.2。 |
| `rah-tools-mcp`（~1.5k） | **保留** | 變成 `tools-mcp` plugin（提供 `ToolProvider`）；外部工具的權限指派移到 `policy-authority` 的設定。 |
| `rah-tools-plugin`（~2k） | **改造** | 泛化成 kernel 的 process plugin host（protocol v2，可提供任何 service，不只 tool）；監督、環境清空、上限、取消的機制全部沿用。 |
| `rah-sandbox`（~1.3k） | **保留** | 變成 `sandbox-process` plugin（提供 `ExecEnv`）。 |
| `rah-session`（~220） | **刪除** | 由 `session-jsonl` 取代（介面不同：append-only＋fork）。 |
| `rah-profile-composition`（~1.1k） | **刪除** | Trusted Profile 由 kernel 設定＋preset 取代；有價值的驗證規則（未指派 → fail-closed）移到 `policy-authority`。 |
| `rah-core`（1 行） | **刪除** | 空 crate。 |
| `rah-cli`（~260） | **改造** | 變成 distribution binary `rah`：讀 `rah.toml` → 建 kernel → 掛 plugin；REPL 部分變成 `frontend-cli`。 |
| `rah-runtime-codex`（~11.5k） | **刪除** | Codex 相容性工作已在 legacy 停止。日後如果需要，再以「外部 agent 當 `loop` plugin」的形式重做，不搬舊程式碼。 |
| `rah-desktop`（~53.5k） | **刪除（P5 之後）** | `main.rs` 10k 行＋`main_tests.rs` 28k 行，深度耦合 legacy 授權和 49 個 Tauri 指令，不值得搬。由 `frontend-web` 取代；`frontend/` 的 CSS、版面、主題可以作為新 Web UI 的視覺參考。`work_log.rs`、`conversation_persistence.rs` 的概念移到 `context-default` 和 `session-jsonl`。`frontend-web` 達到可用之前，desktop 先留在 workspace 但移出 default members。 |

### 5.2 `rah-tools` 細拆

| 模組 | 處置 | 去向 |
|---|---|---|
| `lib.rs` 的 `Tool` trait、`ToolRegistry`、`ToolContext`、`ToolError` | **保留** | 移到 `rah-api`（`Tool`）和 `ToolInvoker`（registry 改為 plugin 間共享的聚合） |
| `authorized_dispatch.rs`、`external_permissions.rs` | **改造** | `policy-authority` |
| `echo.rs`、`fs_read.rs` | **保留** | `tools-builtin` |
| 唯讀 repo 觀察工具：`repository_list`、`_search`、`_stats`、`_file_info`、`_status`、`_diff`、`_diff_staged`、`git_status`、`repository_observer`、`repository_boundary`、`repository_git_layout`、`git_support` | **保留** | `tools-repo`（P4） |
| repo 修改工具：`repository_worktree_patch`、`_multi_file_edit`、`_multi_file_preflight`、`_create_file`、`_create_directory`、`_delete_file`、`_rename_file`、`repository_mutation`、`git_stage`、`git_unstage`、`repository_commit`、`repository_branch_create` | **延後決定** | 預設：只搬 `patch`、`edit-files`、`create-file` 到 `tools-repo-write`（P6），其餘留在 legacy。這些工具各自內建很重的 HostExplicit 審核流程，需要先簡化。 |
| `native_repository_create.rs`、`repository_admission_identity.rs` | **刪除** | desktop 專用 |
| `trusted_profile.rs`、`trusted_profile_source.rs` | **刪除** | 由設定和 preset 取代 |
| `shell_exec.rs`、`host_execute.rs` | **改造** | `tools-shell`（opt-in，requires `sandbox`；預設 preset 中由 policy 設為 `ask`）。注意：這需要 ADR 0036 取代 legacy「不可有通用 shell 權限」的原則。 |
| `cargo_version.rs`、`bin/` | **刪除** | 示範和測試用 |

### 5.3 文件

| 項目 | 處置 |
|---|---|
| `docs/adr/0001`–`0035` | 保留檔案，狀態改為 `Superseded by ADR 0036` 或 `Legacy`（只在檔頭加一行）。 |
| `docs/RAH_V*_RELEASE_GATE.md`、`docs/plans/*`（2026-10-11 之前）、各版 roadmap 和 audit | 移到 `docs/legacy/`，保留歷史。 |
| `docs/ARCHITECTURE.md`、`docs/SECURITY.md`、`ARCHITECTURE_GUARDRAILS.md` | 現有內容移到 `docs/legacy/`，改寫成 plugin kernel 版本。 |
| `AGENTS.md` | **重寫**（P0）：改成 everything-is-plugin 的原則、kernel 小、service 介面、掛載即信任、模型不可信、不重放副作用、生命週期擁有權。 |
| `README.md`、`CHANGELOG.md` | 更新為新方向；CHANGELOG 保留 legacy 歷史。 |

## 6. 新的 crate 拓撲

```text
rah-protocol                         (dependency bottom：資料型別)
rah-kernel                           (生命週期 / service / hook / event / scope / config；不依賴任何 RAH crate)
   ^           ^
rah-api       -> rah-protocol, rah-kernel   (service trait：ModelProvider, AgentLoop, Tool, SessionLog, ...)
   ^
plugins/*     -> rah-api（只依賴 rah-api；plugin 之間互不依賴）
   model-openai-responses, model-llamacpp, model-mock
   loop-react
   tools-builtin, tools-mcp, tools-repo, tools-shell
   session-jsonl, storage-fs, sandbox-process
   policy-authority, context-default, skills-fs, scheduler-basic
   frontend-cli, frontend-web
rah-plugin-host-process -> rah-api        (process plugin protocol v2)
rah (bin)     -> rah-kernel, rah-api, 所有內建 plugin（distribution；用 feature 控制內建清單）
```

架構測試：用 `cargo metadata` 檢查「`plugins/*` 只依賴 `rah-api` 和 `rah-protocol`」以及「`rah-kernel` 不依賴 `rah-api`」，放進 CI。

## 7. 安全模型（路線 C）

| Legacy 原則 | 新的位置 |
|---|---|
| MODEL REQUEST IS NOT AUTHORIZATION | **保留**為整體原則：model 的 ToolCall 一律經過 `ToolInvoker` → `tool.before_invoke`。由 `policy-authority` 實作；`minimal` preset 沒有 policy 時，啟動會明確警告。 |
| 缺少權限指派 → fail-closed | `policy-authority` 的預設行為。 |
| Provider metadata 不能提升權限 | `policy-authority` 只看自己的設定，不看 tool 自報的 metadata。 |
| 不自動重放不確定的副作用 | **提升為 kernel 和 session 合約**（不依賴 policy plugin）。 |
| Process supervision ≠ OS sandbox | 保留在文件與 `sandbox-process` 的說明。 |
| Repo 讀、worktree 寫、index、commit 分離 | 由 `tools-repo` 和 `tools-repo-write` 的 permission 標記搭配 policy 設定表達，不再是 kernel 的硬性分類。 |
| HostExplicit（UI 審核流程，11 個） | 刪除；由 policy 的 `Ask` → frontend 確認對話框取代（通用機制，而不是每個工具各寫一套）。 |
| 不可有通用 shell | **取代**：`tools-shell` 是 opt-in plugin，需要 `sandbox`，預設為 `Ask`。 |

信任邊界：**static 和 process plugin 是使用者在設定中明確掛載的受信任程式碼**；process plugin 只能呼叫自己 `requires` 清單內的 service（host 端強制檢查）；WASM plugin（P6）才提供真正的 capability 隔離。

## 8. 階段與驗收標準

每個階段原則上是一個或數個獨立 commit。每個階段結束都要跑
`cargo fmt --check`、`cargo check --workspace`、`cargo test --workspace`、
`cargo clippy --workspace --all-targets --all-features -- -D warnings`。

### P0：凍結 legacy、建立新規範
- [x] v0.35.0 legacy 最終版本發布（tag＋GitHub Release＋`legacy/authority-first` branch）
- [ ] 撰寫並 accept **ADR 0036：Plugin kernel（everything is a plugin）**，明確 supersede 0001、0002、0003、0011、0021–0026、0033 中衝突的部分
- [ ] 重寫 `AGENTS.md`；舊版移到 `docs/legacy/AGENTS.authority-first.md`
- [ ] 將 legacy 文件移到 `docs/legacy/`（第 5.3 節）
- [ ] 從 workspace 刪除 `rah-runtime-codex`、`rah-core`；desktop 移出 default members
- 驗收：workspace 仍可 build 和 test；CI 綠燈

### P1：`rah-kernel`
- [x] Plugin、Manifest、Scope、ServiceKey、生命週期狀態機、依賴解析、hook chain、event bus、config store（2026-10-11 完成）
- [x] 測試：mount 和 unmount 順序；required service 消失 → 依賴者停止，恢復 → 重新啟動；dispose 會取消 spawn 的 task 並執行 effect；hook 的 bail、waterfall 語義；循環依賴偵測；同一 service 多實作的選擇
- 驗收：kernel < 2k 行（不含測試），不依賴任何 agent 領域型別
- 實作備註（與第 3.3 節草案的差異）：
  - `rah-kernel` 不依賴任何 RAH crate（連 `rah-protocol` 都不依賴），由架構測試強制。
  - `kernel_api` 用單一整數 `KERNEL_API = 1`，不引入 `semver` 依賴；`version` 為字串。
  - `config_schema` 會被保存但尚未驗證；設定以 `serde_json::Value` 傳入，`Scope::config_as::<T>()` 做型別化解析。Schema 驗證延到需要時再加。
  - Hook 分成三個 typed trait：`BailHook`、`WaterfallHook`、`SerialHook`。handler panic 會回傳 `HookError::Panicked`，安全相關呼叫端必須視為拒絕（fail closed）。
  - Service、hook、event handler 只能在 `apply` 期間註冊，`apply` 成功才一次 commit；task 和 dispose effect 綁在 scope 上。
  - 在 `apply` 或 dispose effect 內呼叫 lifecycle 操作會回傳 `Reentrant`，不會 deadlock。巢狀 plugin（`scope.plugin(...)`）延後。

### P2：`rah-api`＋最小的端到端流程
- [x] service trait（第 4 節）：`model`、`loop`、`session`、tools＋`ToolInvoker`、標準 hook（2026-10-11 完成）
- [x] `model-mock`、`loop-react`、`tools-builtin`（echo、fs.read）、`session-jsonl`、`frontend-cli`
- [x] `rah` binary 讀 `rah.toml` 並組合（`rah-cli` 改寫；legacy demo 移除）
- 驗收：`rah --preset dev` 用 mock model 完成「ToolCall → 結果 → 回覆」，session log 可以 resume 和 fork；**在執行中熱替換 model plugin** 的測試通過（全部由 `crates/rah-cli/tests` 驗證）
- 實作備註：
  - 資料型別（`ContextItem`、`ModelRequest`、`ModelEvent`、`SessionEntry`、`ToolEffect` 等）放在 `rah-protocol`；service trait 放在 `rah-api`，並由 `rah-api` re-export kernel。
  - **Tools 的 service key 是 `rah-api` 私有的**：tool provider 只能用 `provide_tools` 註冊，loop 只能透過 `ToolInvoker` 呼叫 tool，因此每次呼叫一定經過 `tool.before_invoke`。policy hook panic 一律視為拒絕；`Ask` 沒有 approver 也視為拒絕。
  - 兩種使用 service 的方式：`inject` 是靜態綁定（provider 消失就重啟）；`KernelHandle::get` 是每次使用時解析。`loop-react` 每一輪才解析 model、`frontend-cli` 每次才解析 loop 和 session，所以換 model 不會重啟 loop 或 CLI。
  - Kernel 新增 `get_all`（列出某 service 的所有 instance）和 `has_handlers`（判斷是否有 policy）。
  - 每個 turn 是 `loop-react` scope 擁有的 task；loop 被卸載時 turn 會被取消，事件串流結束。
  - Session log 重建 context 時，沒有結果的 tool call 會補上「結果未記錄、效果未知、不重放」的錯誤結果。
  - `dev` preset 不掛 policy，啟動時印 `UNGUARDED` 警告；有 policy 的 `default` preset 在 P4。
  - 尚未做：REPL 的 Ctrl-C 只會結束程式，不會只取消當前 turn；`ToolStarted` 事件在 tool 執行完後才送出（順序正確，但時間點延後）。

### P3：真實模型
- [x] `model-openai`、`model-llamacpp`（2026-10-11 完成；同一個 crate `plugins/model-responses`，從 `rah-runtime-openai` 移植 SSE、Responses 驗證和 llama.cpp normalizer）
- [ ] 刪除 `rah-runtime-openai`、`rah-model`、`rah-runtime` → **延到 P5**（2026-10-11 決定：legacy Desktop 仍依賴這三個 crate，和 Desktop 一起移除）
- 驗收：fixture 測試全綠；本機 llama-server 手動驗證（不進 CI）
- 結果：fixture 測試全綠（plugin 13 個、`rah-cli` e2e 2 個）。**本機 llama-server 尚未手動驗證**：這台機器上沒有在執行的 llama-server；未啟動時 `rah --preset llamacpp` 會回報 unreachable 並以 exit code 1 結束。OpenAI 同樣只有 fixture 驗證（`OPENAI_LIVE_NOT_VERIFIED`）。
- 實作備註：
  - 兩個 plugin 共用 Responses 傳輸，所以放在同一個 crate，避免 plugin 之間互相依賴。
  - **Tool 名稱編碼**：OpenAI function name 只接受 `[A-Za-z0-9_-]`，所以 `fs.read` 送出時是 `fs_read`，回來時對應回原名；名稱衝突會加數字後綴。legacy adapter 直接送原名，對真的 OpenAI 會被拒。
  - `model-openai` 只能把金鑰送到官方 endpoint 或 loopback URL；金鑰在 `apply` 時從環境變數讀取（預設 `OPENAI_API_KEY`），缺少時 plugin 為 `Failed`。
  - `model-llamacpp` 只接受 loopback origin；`/health` 和 `/v1/models` 延到第一個請求才檢查並快取，所以 plugin 可以比 server 先啟動；傳輸錯誤會清掉快取。沒有指定 model 時只接受「剛好一個」served model。選用金鑰預設 `RAH_LLAMA_CPP_API_KEY`，不會退回用 OpenAI 金鑰。
  - Context 用 `store: false` 重送；reasoning 內容不跨出 plugin，只送 `ReasoningStarted`。不重試任何請求。
  - `rah` 新增 `llamacpp`、`openai` preset 和 `--model` flag。

### P4：policy、tools、sandbox、process plugin
- [x] `policy-authority`（搬 `authorized_dispatch`、未指派 fail-closed、Allow／Deny／Ask；2026-10-12 完成。in-flight 上限未搬，見下方備註）
- [x] `sandbox-process`、`tools-shell`（opt-in）＋ CLI 的 `Ask` 確認（2026-10-12 完成）
- [x] `tools-repo`（唯讀；2026-10-12 完成）
- [x] `tools-mcp`（2026-10-12 完成）
- [x] `rah-plugin-host-process`（protocol v2）＋一個非 Rust 範例 plugin（Python 寫的 tool；2026-10-12 完成）
- [x] 刪除 `rah-session`（2026-10-12）
- [ ] 刪除 `rah-tools-plugin`、`rah-profile-composition` → **延到 P5**（2026-10-12 決定：legacy Desktop 透過 `rah-profile-composition` 依賴它和 `rah-tools-mcp`，和 Desktop 一起移除）
- 驗收：沒有 policy 時啟動會警告；有 policy 時未指派的 tool 被拒；process plugin 呼叫 `requires` 以外的 service 會被拒
- 結果（2026-10-12）：三項驗收都有自動化測試（`crates/rah-cli/tests/binary.rs`、`policy.rs`；`crates/rah-plugin-host-process/tests/process.rs`）。
- 實作備註（policy-authority，2026-10-12）：
  - 設定：`allow`、`ask`（permission level 清單，預設 `["none","read"]`、`["write","execute"]`）和 `tools`（tool 名稱或結尾 `*` 的前綴 → level）。同一 level 同時出現在 `allow` 和 `ask` 是設定錯誤，plugin 為 `Failed`。
  - 決策只看 policy 自己的設定：沒有指派 → Deny；有效 level 取「指派」和「tool 自報」中較嚴格者，所以 tool metadata 只能收緊、不能授權（legacy 的 external permission 指派規則推廣到所有 tool）。
  - Preset 改為 `default`（llama.cpp，也是 `--preset` 的預設值）、`openai`、`dev`（mock）、`minimal`（mock，不掛 policy，印 `UNGUARDED`）。`llamacpp` preset 改名為 `default`。除了 `minimal`，policy 都是第一個掛載。
  - 設定了 `policy-authority` 但它沒有變成 Active 時，`rah` 會在掛載後續 plugin 前中止啟動（exit 2），不會退化成 unguarded 執行。
  - legacy 的 in-flight 上限沒有搬：只靠 hook 無法可靠地在拒絕、取消和 Ask 失敗時釋放計數，之後若需要，改由 `ToolInvoker` 或獨立 hook 處理。

- 實作備註（sandbox-process、tools-shell，2026-10-12）：
  - `rah-api` 新增 `sandbox` service。與第 4 節草案不同：`ExecEnv::run(ProgramSpec, cancel) -> ExecOutput` 一次跑到結束，不回傳 `SupervisedChild`；串流輸出等真的需要時再加。
  - `sandbox-process` 移植 `rah-sandbox` 的 Job Object／process group 監督。監督在呼叫端的 future 裡完成，沒有 detached reader task；dispose（unmount）、取消、逾時、輸出超限或 drop future 都會終止整個 process tree。程式結束後仍佔用 pipe 的子孫程序在 2 秒後被終止。這仍然只是 process supervision，不是 OS sandbox。
  - `tools-shell` 的 `shell.exec` 宣告 `execute`。被 sandbox 終止或監督失敗回報為錯誤，`ToolInvoker` 記為 `uncertain`；無法啟動或輸入被拒記為 `none`。
  - `frontend-cli` 註冊 `tool.approval`：turn 執行中在終端機詢問，只接受 `y`／`yes`，EOF 視為拒絕。`--prompt` 模式也會詢問。
  - `model-mock` 新增 `/call <tool> <json>`，讓測試能驅動任意 tool。
  - `tools-shell` 不在任何 preset 中，要用設定檔掛載（README 有範例）。

- 實作備註（tools-repo，2026-10-12）：
  - 沒有逐行移植 legacy 的 8k 行。提供 `repo.status`、`repo.diff`（`staged` 取代 legacy 的 `repo.diff-staged`）、`repo.list`、`repo.search`（`path`／`text`），名稱和輸入形狀沿用 legacy；`repo.file-info`、`repo.stats` 延後。
  - Git 透過 `sandbox` service 執行（plugin 宣告 `requires = ["sandbox"]`），環境沿用 legacy 的 hardening，另加 `GIT_LITERAL_PATHSPECS=1`。每次呼叫先確認設定的目錄是 work tree 的 top level，避免 Git 往上找到外層 repo。不設 `safe.directory`：不屬於使用者的 repo 會被 Git 拒絕。
  - 宣告 `read`（legacy 因為會 spawn git 而宣告 `execute`）。限制寫在 SECURITY：Git 仍會讀 repo 自己的 `.git/config`（例如 filter driver），legacy 的 layout 和 ownership 檢查沒有移植。
  - 所有 preset 都掛 `sandbox-process` 和 `tools-repo`，policy 指派 `repo.*` = `read`；`tools-shell` 仍不在 preset 中。
  - 測試在暫存 repo 中跑真的 Git（plugin 單元測試用一個最小的測試 `ExecEnv`，因為 plugin 不能依賴 `sandbox-process`；`rah-cli` 的端到端測試用真的 `sandbox-process`）。

- 實作備註（tools-mcp，2026-10-12）：
  - `sandbox` service 新增 `ExecEnv::spawn(SpawnSpec) -> Spawned`（stdin、stdout、`ProcessHandle`），也就是第 4 節草案裡的 `SupervisedChild`。預設實作拒絕；`sandbox-process` 用自己 scope 的 task 監督，unmount 或 drop handle 會終止 process tree。MCP server 因此也經過 sandbox，不由 `tools-mcp` 自己 spawn。
  - `rah-api` 新增 `ToolError::NotStarted`（effect 記為 `none`），`tools-shell` 的 spawn 失敗改用它。
  - 一個 plugin instance 對應一個 server。只暴露設定 `tools` 列出的 tool，名稱 `mcp.<server>.<tool>`，permission 由設定宣告（legacy 的 external permission 指派），policy 仍要另外指派。server 的 annotations 不採用。legacy 要求 tool 集合完全相符、schema 固定；新版只要求列出的 tool 存在，schema 不固定。
  - server 發來的 request 除了 `ping` 以外一律回 `-32601`；未知 id 的 response（已取消的請求）忽略，不視為協定錯誤（legacy 會斷線）。
  - 測試用 in-process 的腳本化 server（duplex pipe）。真實 stdio 只做了手動驗證（`rah` + legacy `rah-mcp-echo-server` + `sandbox-process`），沒有自動化測試。

- 實作備註（process plugin host，2026-10-12）：
  - 新 crate `crates/rah-plugin-host-process`（只依賴 `rah-api`、`rah-protocol`，架構測試強制）。協定文件在 `docs/PROCESS_PLUGINS.md`。
  - Manifest 由 plugin 在 `initialize` 回報。因為 kernel 在 mount 前就需要 manifest，`rah` 開機時先啟動一次 plugin 讀 manifest 再關掉；之後每次 apply 都啟動新的 process，並檢查 services 和開機時相同（tool 清單可以變）。
  - v2 目前只支援：provide `tools`、requires／optional 只能是 `session`、host 提供 `session.list`／`session.read`。hook、其他 service、event 延後。
  - Process 不經過 `sandbox` service：plugin 是使用者掛載的受信任程式碼，而 sandbox 本身也是 plugin，可能沒掛。host 只做 env 清空、kill on drop；不追蹤子孫 process。
  - `rah` 的 `use = "process"` 必須有 `id`。

### P5：Web frontend
- [ ] `frontend-web`：HTTP＋WebSocket API、Web UI（對話、activity、session 列表、fork 和 resume、`Ask` 確認、plugin 管理頁可以啟用、停用、設定 plugin）
- [ ] `context-default`（sliding window、記憶）
- [ ] 刪除 `rah-desktop`（或另開選配的 `frontend-tauri` 殼，只內嵌 Web UI）；同時刪除 Desktop 依賴的 `rah-runtime-openai`、`rah-model`、`rah-runtime`（從 P3 延後）、`rah-profile-composition`、`rah-tools-plugin`、`rah-tools-mcp`、`rah-tools`、`rah-sandbox`（從 P4 延後）
- 驗收：功能上可以取代 desktop 的日常使用；**在 UI 上切換 model 和 loop plugin 不需要重啟**
- 切片（2026-10-12 決定）：
  - P5a：`frontend-web` 核心——HTTP＋WebSocket server、存取控制、session 列表／讀取／建立／fork、經 WebSocket 串流的 turn 和取消、`Ask` 確認、唯讀 plugin 狀態、最小 Web UI。**2026-10-12 完成**。
  - P5b：plugin 管理——`rah-api` 新增 plugin catalog service（由 distribution 提供，因為 plugin 不能依賴 `rah-cli`），UI 可以掛載、卸載、重啟、改設定，並在不重啟的情況下切換 model 和 loop。**2026-10-12 完成**。
  - P5c：`context-default`（sliding window、記憶）。
  - P5d：刪除 Desktop 和延後的 legacy crate，更新文件。
- 設計決定（2026-10-12）：
  - Server 用 `axum` 0.8（含 `ws`）。新依賴的理由：Web frontend 需要 HTTP 和 WebSocket server；`hyper`、`tower` 已經在依賴樹中。
  - 只綁 loopback（`127.0.0.1`、`::1`、`localhost`），設定成其他位址是錯誤。
  - 存取控制：每次啟動產生隨機 token，`rah` 印出 `http://127.0.0.1:<port>/#token=<token>`；所有 `/api` 請求和 WebSocket 都要帶 token。另外檢查 `Host`（防 DNS rebinding），WebSocket 和會改變狀態的請求檢查 `Origin`。
  - UI 是內嵌在 binary 裡的靜態 HTML／JS／CSS，不需要 build 步驟，不載入外部資源。
  - `Ask`：`frontend-web` 註冊 `tool.approval`，推給已連線的瀏覽器；沒有瀏覽器連線時不回答（等於拒絕），等待逾時也是拒絕。
- 實作備註（P5a，2026-10-12）：
  - `rah-api` 新增 host event `FrontendListening { plugin, url }`；`rah` 負責印出（含 token），plugin 自己不印。測試也透過這個 event 拿到位址和 token。
  - `rah --web [--bind]` 把 preset 裡的 `frontend-cli` 換成 `frontend-web`；不能和 `--prompt`、`--session` 一起用。程式在 Ctrl-C 時結束。
  - HTTP 連線由 axum／hyper 的 task 處理（請求很短）；WebSocket handler 會監看 plugin scope 的 cancellation token，unmount 時關閉（有測試）。
  - 每條 WebSocket 同時只跑一個 turn；斷線會取消該 turn。
  - 同時掛 `frontend-cli` 和 `frontend-web` 時，兩者都註冊 `tool.approval`（priority 0），由先註冊的回答。
  - UI 的 JavaScript 只做了語法檢查（`node --check`）和用 curl 確認資源可以載入；沒有自動化的瀏覽器測試。
- 實作備註（P5b，2026-10-12）：
  - Service 叫 `admin`（`PluginAdmin`），不只是 catalog：除了列出 kind，也負責 mount、unmount、configure、restart、replace，並記錄每個 instance 的 kind 和設定（kernel 有存設定但沒有 getter，也不知道 kind；不為此改 kernel）。
  - `rah` 開機時一定先掛 `distribution` plugin（提供 `admin`），`boot` 和 `admin` 共用同一個建構路徑（含 `process`）。`admin` 拒絕卸載或改設定自己。
  - 「切換」用 `replace`：先掛新的（id 被佔用時自動加數字後綴），新的變成 Active 才卸載舊的；失敗時保留舊的並回 409。Model 和 loop 都是每次使用時解析，所以下一個 turn 就換過去，WebSocket 和 session 不受影響（端到端測試驗證）。
  - 驗收「在 UI 上切換 model 和 loop 不需要重啟」：API 層有自動化測試；UI 的 Plugins 面板沒有在瀏覽器中實際操作過。

### P6：生態擴充
- [ ] WASM plugin host（wasmtime＋WIT）
- [ ] `skills-fs`、`scheduler-basic`（含 subagent：loop 也是 service，可以巢狀呼叫）
- [ ] `tools-repo-write`（patch、edit-files、create-file）
- [ ] 第二種 loop（例如 plan-and-execute），證明 loop 確實可以替換

## 9. 已決事項（2026-10-11 核准，記錄於 ADR 0036 第 5 節）

1. **預設 policy**：`default` preset 掛 `policy-authority`；`minimal` 不掛，但啟動時要大聲警告 *unguarded*。
2. **名稱與版本**：沿用 RAH；新架構的第一個 release 是 `0.40.0`（workspace 版本在準備該 release 前維持 `0.35.0`）。
3. **Frontend**：P5 只做 `frontend-web`；Tauri 殼列為選配。
4. **Repo 修改工具**：只搬 `repo.patch`、`repo.edit-files`、`repo.create-file` 到 `tools-repo-write`（P6），其餘留在 legacy。
5. **文件語言**：ADR、`AGENTS.md`、架構與安全文件用英文；計畫類文件可以用中文。

## 10. 風險

| 風險 | 緩解 |
|---|---|
| Rust 的動態型別 service 查找（`dyn Any` downcast）容易出錯 | 使用 typed `ServiceKey<S>`；kernel 測試涵蓋型別不符的情況 |
| Hot reload 時正在跑的 turn | 卸載 plugin 前先取消它 scope 內的 task；turn 以 `Cancelled` 結束；不確定的 tool effect 標為 `uncertain` 寫進 log |
| Plugin 間的事件順序和重入 | hook chain 明確定義優先序；event bus 不保證跨 plugin 的順序，需要順序時改用 hook |
| 範圍膨脹 | 嚴格依照階段推進；P2 的端到端流程是第一個硬里程碑，之前不做 UI |
| 刪除 desktop 太早而失去可用版本 | desktop 保留到 P5 驗收；legacy v0.35.0 隨時可以用 |
