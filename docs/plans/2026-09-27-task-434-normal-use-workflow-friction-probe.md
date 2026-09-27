# Task 434 — Normal-Use Workflow Friction Probe

Date: 2026-09-27

Type: normal-use evidence collection / friction probe

Outcome: **C — NORMAL-USE PROBE INCONCLUSIVE**

## Starting checkpoint

The Windows-local repository started clean. `HEAD` and fetched
`origin/master` both equaled
`58fa2913519ad1ec3d68b949dfc87c5d15449e87`; divergence was `0 0`, and
`git merge-base --is-ancestor origin/master HEAD` succeeded. The required
starting gate passed.

Task 433's supplied exact-head CI run `36295795568` was PASS. Its disposition
was **B — NO v0.33 PRODUCT CAPABILITY SELECTED — CURRENT PRODUCT SURFACE
REMAINS SUFFICIENT**. Task 433 left the product capability unselected,
workspace version at `0.32.0`, and HostExplicit at exactly 11. Tasks 430–432's
Tauri IPC incident was closed. No maintenance or release task was selected.

## Production executable and environment

| Item | Evidence |
| --- | --- |
| Source SHA | `58fa2913519ad1ec3d68b949dfc87c5d15449e87` |
| Build | `cargo build --release -p rah-desktop` — PASS; release profile completed in 1m 36s |
| Executable | `target/release/rah-desktop.exe` |
| Executable SHA-256 | `FDACBC4D280B5A759F6292FD3B554ACED1B3649FB629D7E4DACA41AA15B31D18` |
| Rust | `rustc 1.98.1 (48a229cea 2026-09-01)`, `x86_64-pc-windows-msvc` |
| Cargo | `cargo 1.98.1 (797e8a9bc 2026-08-05)` |
| Windows | Microsoft Windows 11 IoT Enterprise LTSC, version `10.0.26100`, build `26100` |
| Tauri | `tauri 2.11.5`, `tauri-build 2.6.3`, `tauri-runtime-wry 2.11.4`, `tauri-plugin-dialog 2.7.2`, `wry 0.55.1` |
| Codex on PATH | `codex-cli 0.157.1` |
| Certified baseline check | `scripts/codex-baseline.ps1 verify 0.149.0` — verified |
| RAH provider/model | Not selected or observed; chat setup was not reached |

The release executable was launched directly from the repository with its
working directory set to the repository. Process ID was `13228`; it remained
running and its window title was `RAH`. A captured window showed the RAH
`0.32.0` desktop-ready landing view and “No profile remembered.” This proves
that the executable opened to its landing view; it does not prove provider
connection or any chat workflow. The certified `0.149.0` baseline check does
not establish that Desktop selected it. No model or GPT-5.6-TERRA live gate
was run.

## Disposable fixture

The disposable repository was created at
`F:\coding\otherPrj\rah-task434-normal-use` and committed clean at baseline
`fea996895b36e73402cc4275af6ca85757a71d50`. Its tracked tree was:

```text
README.md
docs/architecture.md
docs/usage.md
src/config.rs
src/engine.rs
src/main.rs
tests/engine_test.rs
```

The tracked architecture document contains `RAH434_ARCH_TOKEN` followed by
`The flux coordinator owns the worker lifecycle.`. `src/config.rs` defines
`WORKER_LIMIT` as `4`; `src/engine.rs` returns
`crate::config::WORKER_LIMIT`; the test file contains `RAH434_TEST_TOKEN` as a
comment. The fixture began clean. It was not selected in RAH Desktop and was
not modified by the probe.

## Normal-use runs

No normal-use run began. Each run requires a new human prompt in production
Desktop chat, followed by observation of the model's actual Tool calls and
results. This session could launch and capture the Desktop landing view, but
could not reliably focus or send input to the WebView. UI Automation exposed
only the WebView container, not its controls; keyboard delivery did not reach
the RAH window. No chat transcript, Tool activity, or result was available to
observe.

| Run | Baseline | Fresh conversation | Phases A–F | Result |
| --- | --- | --- | --- | --- |
| 1 | `fea996895b36e73402cc4275af6ca85757a71d50` | Not started | Not run | Inconclusive: Desktop chat input was inaccessible from this session |
| 2 | `fea996895b36e73402cc4275af6ca85757a71d50` | Not started | Not run | Inconclusive: no Run 1 chat evidence and no interactive Desktop input |
| 3 | `fea996895b36e73402cc4275af6ca85757a71d50` | Not started | Not run | Inconclusive: no earlier run context and no interactive Desktop input |

### Phase-by-phase evidence boundary

- **A — structural discovery:** no prompt; no `repo.list` or alternative call
  observed; no accuracy or intervention count can be reported.
- **B — literal tracked-content discovery:** no prompt; no `repo.search`, read,
  or result observed.
- **C — cross-file code understanding:** no prompt; no browse/search/read
  sequence observed.
- **D — bounded authoring:** no prompt; no edit Tool selected or executed.
  The fixture remains at its original value `4` and clean.
- **E — repository review/status:** not exercised; no review clicks or
  presentation evidence.
- **F — Stage/Unstage:** not exercised; no index operation was performed.
- **G — reviewed Commit:** not exercised because the preceding edit and review
  workflow was not reached. No fixture commit beyond the baseline was made.

No direct Rust Tool call, direct Tool invocation, direct Tauri invoke, or
Node-only test was used as a substitute for model-selected Desktop workflow.
Therefore the probe establishes no successful or failed model-selected Tool
dispatch evidence.

## Friction classification and limits

This is **inconclusive evidence access**, not evidence of a RAH product
workflow blocker. No normal user prompt was submitted, so there is no measured
workflow completion rate, workaround, repeated pain, or user cost. The
interaction limitation belongs to this execution session; it is not attributed
to the RAH Desktop product.

The required three-run serial comparison, repository effects, review usability,
Stage/Unstage behavior, and optional reviewed Commit remain unevaluated. The
probe makes no claim about `repo.list`, `repo.search`, authoring Tools, dynamic
Tool dispatch, or UI review surfaces. In particular, the prior nonclaim
`MODEL-SELECTED DYNAMIC TOOL DISPATCH NOT ESTABLISHED UNDER THE APPROVED
GPT-5.6-TERRA LIVE GATE` remains unchanged.

No production Rust or frontend code, Tauri permission, HostExplicit route,
Tool authority, provider composition, Trusted Profile schema, ADR, dependency,
workspace version, or release state was changed. HostExplicit remains exactly
11 by inherited checkpoint evidence; it was not independently recounted in
this incomplete probe.

## Disposition and next step

**C — NORMAL-USE PROBE INCONCLUSIVE.** The smallest evidence-recovery step is
to complete the three prescribed fresh-conversation runs in an interactive
production Desktop session where prompts and Activity/Tool results can be
observed, using the exact executable and fixture baseline recorded above.
Capture the prompt, response, Tool name/input/result, host actions, and final
fixture state for each phase. Do not treat the landing-view capture or the
certified baseline check as chat evidence.

Do not reassess v0.33 from this artifact. The Task 433 decision remains the
last supported product-scope decision: **v0.33 product capability: NONE
SELECTED**. No Task 435 product research question is selected until normal-use
evidence establishes a concrete blocked workflow or repeated material pain.

## Validation and change inventory

This is one research artifact. No production source or authority surface
changed. Post-artifact validation results and Git/CI closeout are recorded by
the task completion report.
