# Task 510B2Q — Rustfmt recovery and certification seam publication

Starting HEAD: `436470337269546a1209d0b9803c2b8aa9e1f6b4`.

Scope: exact rustfmt correction only, then serial withheld closure gates. Preserve
teardown, typed diagnostics, snapshot algorithm, production rejection of 0.160.0
and preferred 0.157.1. Stop at first required failure; publish only under A,
then normal master push and natural exact-head CI. Do not begin Task 510B.

Evidence directory: `F:/temp/task510b2q-evidence`.

Rustfmt --edition 2024 on certification_support.rs changed only the match arm
at line 397: braces around Error::other("unstable certification measurement")
and collapsed its argument onto one line. No payload/control-flow/import change.
The requested git diff is empty because the file is untracked. Saved before-file
and git diff --no-index show exactly the documented formatting hunk (exit 1
means differences). Diff guard PASS. Source frozen during validation.

cargo fmt --check PASS; git diff --check PASS. Workspace all-targets/all-features
Clippy with -D warnings PASS, exit 0. Workspace check PASS, exit 0.
Existing duplicate-target Cargo manifest warning remains; no Rust/Clippy finding.

All required closure gates completed below.

## Closure results

Final classification: **A — CROSS-CRATE CERTIFICATION CONSTRUCTION SEAM VALIDATED**.

| Gate | Result |
| --- | --- |
| Exact rustfmt correction / diff guard | Formatting only; PASS |
| cargo fmt --check / git diff --check | PASS / PASS |
| Authoritative workspace Clippy, all targets/features, -D warnings | PASS, exit 0 |
| cargo check --workspace | PASS, exit 0 |
| cargo test --workspace | 1085 passed / 0 failed / 24 ignored; 58 summaries |
| Canonical Windows Desktop | 358 passed / 0 failed / 20 ignored; harness PASS, exit 0 |
| Frontend syntax/static | All 8 frontend JS syntax checks and all 7 existing frontend suites PASS |
| Remembered workspace layout | PASS on first closure execution; no recurring Edge timeout |
| Tauri inventory | 47 runtime / 47 manifest / 47 generated / 47 default allows / 47 frontend commands |
| Metadata | Windows-filtered offline metadata: 14 workspace members/packages, all 0.33.0, edition 2024 |
| Cargo.lock | Task-start SHA256 9a717f964d02c8a714d89d35bd4a38b688a114a98408a1fbb28dfa2c22e80cbe unchanged |
| HostExplicit | Static exact allowlist 11; executable host_allowlist_is_exact PASS, 1/0/0, 377 filtered |
| Final preserved snapshot | SHA/file ID/length all match; independently measured before staging |

Frontend suites: model_picker_test.js, model_preflight_test.js,
remembered_workspace_layout_test.js, remembered_workspace_test.js,
repository_membership_test.js, runtime_model_state_test.js, status_authority_test.js.
Tauri script syntax check and full inventory also PASS. No frontend correction.

Canonical evidence: desktop/20261004-215939-781-1efa8a63526b4d91b800d8a8c6219d03,
with its complete logs and summary. Both target variables remained
F:/temp/rah-task510b2g-target. No source edits while gates ran.

Preserved snapshot path:
`F:/Temp/rah-codex-certification-snapshot-18440-1791116997708427800-0/codex.exe`.
SHA256: `fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d`.
File ID: `0x0000000000000000002e0000000a80a1`. Length: **326872368**.
No recreation or live npm fallback.

## Authority, dependency and ADR review

Complete tracked source diff and both new source files inspected. Certification
support is behind the non-default certification-harness feature; Desktop test
module additionally requires test + Windows. No certification IPC or frontend,
configuration, CLI or environment activation surface. Real factory retains
ordinary host-selected provider/workspace and narrow existing Tool mediation.
Public diagnostics remain sanitized, typed measurement causes remain process-local.
No new neutral API or RAH crate dependency edge.

Existing Task 510B2 WIP windows-sys 0.61.2 adapter dependency/lock edge supports
Windows artifact identity; preserved unchanged from task start. Q adds no dependency.
No changes to ToolRegistry authorization, active repo authority, repo switching,
leases, permissions, Trusted Profiles, mutation uncertainty, remembered workspace
semantics, or provider/model authority. Production admission constants and baseline
sources unchanged: certified set only codex-cli 0.157.1, preferred 0.157.1.
Candidate 0.160.0 remains **unadmitted / uncertified**. No live certification,
inference or Tool execution was performed by Q. P's two smoke/cleanup passes and
5 typed/snapshot plus presentation regression passes remain historical evidence.
Proven runtime/conversation → original persistence → lexical app/state scope →
fixture cleanup order is unchanged; external inert persistence remains.

**ADR-B**: ADR 0005, ADR 0030 and Task 498 diagnostic envelope apply; no new ADR.

## Publication workflow

A authorizes publication of the complete intended Task 510B2 implementation and
historical reports. Pre-staging and staged audits must inspect only intended paths,
with diff checks PASS. Commit message: `test: add Codex runtime certification seam`.
Normal push to GitHub master, then natural exact-head CI required. No force push,
tag, release, version bump or Task 510B execution. Publication SHA/run/status and
final repository evidence will be recorded in the external evidence directory and
final return, avoiding a self-referential commit identifier in its own report.
Task 510B resume authorization remains conditional on exact-head CI PASS.
