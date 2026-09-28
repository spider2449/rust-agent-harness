# Task 453 — Windows executable file identity audit

Date: 2026-09-28. Task 438 remains **NOT CERTIFIED**.

## Task 452 Gate 1

Task 452 observed one new directly Desktop-owned app-server, alive through five samples, with the saved 0.157.1 SHA256 `8cb0e69e99ff2a158c54815db82d0f2e524d8f301bc30184722cfd1ae5973574`. Its `path_match` was false. The actual child path was not retained. The Gate-1 label B therefore did not establish a different executable file object.

## Old comparison semantics

`task452_process_census` uses `Win32_Process.ExecutablePath` from CIM for the process path. It calls PowerShell `Get-Item -LiteralPath ... .FullName` on expected and actual, then `[StringComparer]::OrdinalIgnoreCase.Equals`. This handles case-only differences but does not canonicalize, strip extended-length prefixes, resolve 8.3 spelling, or compare file handles. It hashes the actual path independently. The old `path_match` is a path-text comparison, not file identity.

## Task 453 method

The diagnostic test resolves the expected executable from the saved baseline store. It obtains the running Desktop-owned child's image path from `Win32_Process.ExecutablePath`, the process-image facility already used by this harness. It opens expected and actual paths and calls `GetFileInformationByHandle` for volume serial and 64-bit file index. It records raw and canonical paths with user prefixes redacted, size, SHA256, and independent raw-path, canonical-path, case-insensitive canonical-path, file-object, size, and hash comparisons. Case, separator, extended-length, and 8.3 differences are measured through those fields; none is presumed.

The test stops before Send. No runtime resolver or production policy is changed. The Task 450 and 452 historical observations remain unchanged.



## Structural regressions and compile gate

The structural test passed (1 test): exact same path has the same file identity; case-only spelling and a `.` component resolve to the same file identity; two separate files with identical contents have equal SHA256 and unequal `(volume serial, file index)`. The first `--exact` filter selected zero tests and is not counted. `cargo fmt --check`, `cargo test -p rah-desktop --no-run`, and `git diff --check` passed before the live attempt. The embedded Task 453 PowerShell process-image query parsed successfully. A diagnostic compile type mismatch was corrected before Connect.

## One production Connect attempt

Exactly one Task 453 production Connect was performed. Pre-launch and pre-Connect app-server candidate counts were both zero. Desktop PID 5984 owned exactly one new direct child, PID 1096. It was present in all five samples and alive at the final sample. The process image came from Windows `Win32_Process.ExecutablePath`; no path was inferred from a command line or resolver result. The test stopped before Send.

| Evidence | Expected saved executable | Actual process image |
| --- | --- | --- |
| Basename | `codex.exe` | `codex.exe` |
| Raw path, sanitized | `\\?\%LOCALAPPDATA%\codex-baselines\0.157.1\codex.exe` | `%LOCALAPPDATA%\codex-baselines\0.157.1\codex.exe` |
| Canonical path, sanitized | `\\?\%LOCALAPPDATA%\codex-baselines\0.157.1\codex.exe` | `\\?\%LOCALAPPDATA%\codex-baselines\0.157.1\codex.exe` |
| File size | 322515248 | 322515248 |
| SHA256 | `8cb0e69e99ff2a158c54815db82d0f2e524d8f301bc30184722cfd1ae5973574` | same |
| Volume serial | 2929214197 | 2929214197 |
| File index | 10414574138354066 | 10414574138354066 |

Independent comparisons: `raw_path_equal=false`; `canonical_path_equal=true`; `windows_path_equivalent=true`; `file_object_equal=true`; `size_equal=true`; `sha256_equal=true`. The old census again reported `path_match=false`, `hash_match=true`. The measured path difference is the Windows extended-length `\\?\` prefix; no drive-case, component-case, separator, or 8.3 difference was observed.

## Primary classification and correction boundary

**A — SAME SAVED FILE OBJECT; PATH COMPARISON DEFECT PROVEN.** The old `Get-Item.FullName` plus ordinal-ignore-case comparison treats the process path without the extended-length prefix as different from the configured path with it. The live certification assertion was narrowed to require exactly one Desktop-owned candidate, matching Windows file-object identity, and matching saved 0.157.1 SHA256. Path text remains diagnostic. The production resolver, selection policy, Connect behavior, provider configuration, and spawn arguments were not changed.

Task 452's `path_match=false` remains historical evidence and is explained by this attempt's measured spelling difference; this does not retroactively prove Task 450's exact mechanism. Task 438 remains **NOT CERTIFIED**. No Send, Tool request, commit, push, CI, tag, or release was performed. RAH 0.32.0, HostExplicit exactly 11, and no selected v0.33 product capability remain unchanged.
