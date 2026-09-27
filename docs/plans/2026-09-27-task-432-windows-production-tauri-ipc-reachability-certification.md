# Task 432 — Windows Production Tauri IPC Reachability Certification

## Verdict

**A — WINDOWS PRODUCTION TAURI IPC REACHABILITY CERTIFIED**

Task 431 restored production Windows WebView IPC reachability for the
`repo.edit-files`, `repo.create-file`, and `repo.delete-file` HostExplicit
Prepare routes. On the exact Task 431 source, each production form submitted
through `status.js`, returned a Rust Prepare review through Tauri IPC, displayed
that review in the release Desktop WebView, and completed Cancel with no
repository effect. Tauri capability did not block any of the three invokes.

## Exact product and starting gate

| Item | Observed value |
| --- | --- |
| RAH `HEAD` and `origin/master` | `89a589f32cd82eb41a45145f1d69b10538870675` |
| `origin/master...HEAD` | `0 0`; ancestor check passed |
| Starting worktree/index | Clean |
| Task 431 exact-head CI | [run 36292302993](https://github.com/spider2449/rust-agent-harness/actions/runs/36292302993), completed success for `89a589f32cd82eb41a45145f1d69b10538870675` |
| Build command | `cargo build --release -p rah-desktop` from the clean RAH checkout; PASS |
| Executable | `target/release/rah-desktop.exe`, SHA-256 `4088593541F402BA765BDB83141B9637E9AC42AC91574DBC1CF895D2FC3322A2` |
| Launch | `Start-Process -FilePath 'F:\coding\otherPrj\rust-agent-harness\target\release\rah-desktop.exe' -WorkingDirectory 'F:\coding\otherPrj\rust-agent-harness' -PassThru`; PID 7260 |
| Mode | Release profile Desktop executable, launched directly; not an installer or published release package |
| OS | Windows 10 IoT Enterprise LTSC 2024, version 2009, build 26100 (`10.0.26100`) |
| Toolchain | Cargo 1.98.1; rustc 1.98.1 |
| Tauri stack | `tauri` 2.11.5, `tauri-build` 2.6.3, `tauri-runtime-wry` 2.11.4, `wry` 0.55.1 |
| WebView2 | `EBWebView/Last Version` reported `153.0.4234.48` |
| Codex | Certified local `codex-cli 0.149.0` baseline verified by `scripts/codex-baseline.ps1 verify 0.149.0`; Desktop reported Certified baseline, connected |
| Provider/profile | Model provider `inherit`, no model identifier override; no Trusted Profile remembered or loaded; zero MCP providers, Process Plugins, or expected external Tools |

For inspection, the launch shell set
`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=9227 --remote-allow-origins=*'`.
A temporary local CDP client connected to the release WebView2 page at
`http://tauri.localhost/` (`RAH`, `Desktop UI ready`). It set values in the
existing forms, called each form's `requestSubmit()`, read the rendered review,
and clicked its existing Cancel button. It did not call Tauri `invoke` directly,
mock capability resolution, or modify the product bundle. The native folder
picker was used to select the fixture. The application reported repository
selected, Codex connected, repository tools active, Effective Authority
`Current`, and all three HostExplicit forms present.

## Disposable fixture and corrected baseline

The final certification repository was
`F:\coding\otherPrj\rah-task432-ipc-fixture-lf`, outside RAH. It had a clean
root commit `7356ff19795e1f907c331228c6c35ea401a95ef6` with:

| Path | Baseline content / state | Baseline SHA-256 |
| --- | --- | --- |
| `edit-target.txt` | `alpha`, `RAH_TASK432_EDIT_SENTINEL`, `omega`, each LF terminated | `0CB018AFDBF266847BA9F035588535C1A3103D2C74C9124A69335E1932433DEE` |
| `delete-target.txt` | `RAH_TASK432_DELETE_SENTINEL` plus LF | `CF73440F77FD6883EB0683264448A02324A32043742381BA4DDE675F7FD6522F` |
| `keep.txt` | `RAH_TASK432_KEEP_SENTINEL` plus LF | Not needed for the three target checks |
| `created-by-task432.txt` | Absent | N/A |

Before the three-route run, `git status --short`, `git diff`, and
`git diff --cached` were empty; the create target was absent. The tracked
deletion file's worktree blob matched `HEAD:delete-target.txt` exactly
(`c0586689c30685e0f38c640104f18f41fe9b5cf6`). The source RAH repository
was never used as the mutation fixture.

An earlier disposable fixture at `F:\coding\otherPrj\rah-task432-ipc-fixture`
had root commit `f5f600e194f4fa77cb93f6991365906906b8fa11`. Its edit and create
Prepare/Cancel routes succeeded, but delete Prepare returned the RAH backend
`host_invocation_precondition_changed` message, “A reviewed precondition
changed. Prepare a fresh review.” Windows Git had `core.autocrlf=true`; the
file created with PowerShell `Set-Content` had CRLF worktree bytes while its
HEAD blob had LF bytes (`git hash-object --no-filters` differed from
`HEAD:delete-target.txt`). The delete preparer requires byte equality. This
was a fixture precondition failure after Tauri IPC, not a Tauri denial. The
first fixture was left untouched; the complete three-route run below used a
new LF fixture and one unchanged RAH executable. The old error text remained
in the UI error area, while each later successful review and Activity event
was observed separately.

## Production WebView Prepare / Review / Cancel evidence

The source path is the production `status.js` form submission handler:
`repo_edit_files` invokes `host_prepare_repo_edit_files`, `repo_create_file`
invokes `host_prepare_repo_create_file`, and `repo_delete_file` invokes
`host_prepare_repo_delete_file`. Each awaits `invoke(...)` before calling
`openHostReview(...)`; that function creates the visible modal and its Cancel
button invokes `host_cancel_tool_invocation`. The CDP client used the actual
forms and their installed submit handlers, not a Rust/test/direct invoke path.

| Route | Production review observed | Before Confirm | Cancel evidence |
| --- | --- | --- | --- |
| `repo.edit-files` | “Review Host multi-file edit”; operation `repo.edit-files`; one target `edit-target.txt`; expected `RAH_TASK432_EDIT_SENTINEL`, replacement `RAH_TASK432_EDIT_REPLACEMENT`; preimage SHA-256 `0cb018afdbf266847ba9f035588535c1a3103d2c74c9124a69335e1932433dee`; postimage SHA-256 `6f7f70eefde848d9da1eb2e98cea013741ad9e914fe3c10d56d3e83145b5d5b3` | `git status`, worktree diff, and staged diff empty; edit hash unchanged | Existing Cancel button clicked; modal closed; Activity recorded `repo.edit-files: Host action cancelled before start`; hash and Git state unchanged |
| `repo.create-file` | “Review Host new-file creation”; operation `repo.create-file`; path `created-by-task432.txt`; complete `RAH_TASK432_CREATE_SENTINEL\n`; content SHA-256 `7d78d9e8d8ad38056db3cd6101f4184f1e2ee263fa9bfb4ec9d3f71ecd97d39b`; worktree/HEAD/index target absent | Target absent; Git state unchanged | Existing Cancel button clicked; modal closed; Activity recorded `repo.create-file: Cancelled without effect`; target remained absent |
| `repo.delete-file` | “Review Host file deletion”; operation `repo.delete-file`; `delete-target.txt`; clean HEAD-tracked stage-0 mode `100644`; complete `RAH_TASK432_DELETE_SENTINEL\n`; content SHA-256 `cf73440f77fd6883eb0683264448a02324a32043742381ba4dde675f7fd6522f`; review said worktree bytes equal HEAD blob | File existed with the baseline SHA-256; Git state unchanged | Existing Cancel button clicked; modal closed; Activity recorded `repo.delete-file: Cancelled without effect`; file still existed with the same hash |

The visible review modal is positive IPC evidence: `status.js` opens it only
after the literal Prepare `invoke` resolves. The operation-specific review
data and `Host action prepared` Activity entries establish successful Rust
handler returns. The page origin, release executable, real Tauri runtime,
existing submit handlers, and successful reviews establish that Tauri accepted
all three production invokes. No corrected-fixture route produced a Tauri
“command not allowed,” command-not-found, or invoke-rejected result. The
earlier backend precondition failure is recorded above and not counted as IPC
denial.

## Final no-effect and source proof

After all three Cancels, the final fixture still had HEAD
`7356ff19795e1f907c331228c6c35ea401a95ef6`. `git status --short`,
`git diff`, and `git diff --cached` were empty. `edit-target.txt` was still
`0CB018AFDBF266847BA9F035588535C1A3103D2C74C9124A69335E1932433DEE`;
`delete-target.txt` still existed and was
`CF73440F77FD6883EB0683264448A02324A32043742381BA4DDE675F7FD6522F`;
`created-by-task432.txt` was absent. No Confirm, Stage, or Commit occurred in
the fixture after its baseline commit.

Before this document was added, RAH `git status --short` was empty and RAH
`HEAD` remained `89a589f32cd82eb41a45145f1d69b10538870675`. Building and
launching the release Desktop application created no tracked source changes.

## Authority interpretation, limits, and follow-up

This is a Windows release Desktop WebView IPC reachability certification for
three host-driven HostExplicit Prepare routes, followed by Review and Cancel.
It does not newly certify arbitrary WebView IPC permissions, backend
authorization beyond existing evidence, model-selected invocation, Confirm
or destructive effects, rollback, Linux/macOS, network authority, or unrelated
Tauri commands. HostExplicit remains exactly 11. No production permission,
frontend, Rust authorization, provider, ADR, or release artifact changed.

The maintenance incident can close as Task 430 defect identified, Task 431
configuration corrected with deterministic CI guard, and Task 432 real Windows
production IPC path certified. Recommended next task: **Task 433 — v0.33
Post-Maintenance Candidate Reassessment**, bounded to genuinely new evidence
and preserving Tasks 427 and 428's existing no-feature conclusions unless
that evidence changes them.
