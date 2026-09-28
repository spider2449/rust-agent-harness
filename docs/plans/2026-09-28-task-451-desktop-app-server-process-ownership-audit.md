# Task 451 — Desktop app-server process ownership census

Date: 2026-09-28. Disposition: **A — EXPECTED SAVED 0.157.1 CHILD OWNERSHIP CONFIRMED**. Task 438 remains **NOT CERTIFIED**.

## Scope and prior failure

Task 450's first production Desktop live test failed after Connect, before Send, with `Desktop did not spawn exactly one saved app-server executable`. Its ignored test is `task_450_current_codex_desktop_neutral_chat` in `crates/rah-desktop/src/main_tests.rs`. It calls `task_324_d_process_census` before Desktop construction and again after Connect, then `task_324_d_new_app_server_process_ids`. The census includes process rows when executable path matches the saved binary, command line contains structural `app-server`/`--stdio` tokens, or the name resembles Codex. The new-PID filter requires a Codex basename plus both mode tokens. The assertion requires `spawned.len() == 1` and a saved-path identity match in the after census. It did not print either census, the new PID set, or parent relationships. The exact dimension of the **earlier** failure is therefore unrecoverable.

## Expected baseline and diagnostic method

`scripts/codex-baseline.ps1 path 0.157.1` resolved `C:\Users\morefunfun\AppData\Local\codex-baselines\0.157.1\codex.exe`. Length: 322,515,248 bytes. SHA256: `8cb0e69e99ff2a158c54815db82d0f2e524d8f301bc30184722cfd1ae5973574`. The existing exact-version baseline verification and Task 450 controls establish `codex-cli 0.157.1`; the running child was not invoked for version inspection.

A temporary ignored test used the same Tauri `Builder`, model selection, and production `connect_codex` call as Task 450. It captured sanitized `Win32_Process` rows before construction, before Connect, and in five post-Connect samples. The temporary PowerShell census retained PID, parent PID, name, creation time, executable path, file length, SHA256, structural `app-server` and `--stdio` booleans, and PID/name ancestry. No raw command line, environment, credentials, account data, or chat text was retained. PowerShell parse and `cargo check -p rah-desktop --tests` passed before the live run. One initial Cargo invocation used an over-specific `--exact` filter and ran **zero tests**, so no Connect occurred. The next invocation ran the one diagnostic test; it passed. The temporary test and script were then removed.

The five sample timestamps were 00:16:01.511, 00:16:03.262, 00:16:05.009, 00:16:06.727, and 00:16:08.478 UTC. Effective sample spacing was about 1.7 seconds because each full CIM census and executable hashing took time; the observation window was about seven seconds. A process wholly between samples would be invisible. The owned child was present in every sample.

## Census and lifecycle

| Stage | Relevant process evidence |
| --- | --- |
| Pre-launch, 00:15:57 UTC | Unrelated `codex.exe` PID 4156, parent 8092 `node.exe`, npm package path, same binary hash; `app-server=false`, `stdio=false`. Its `codex-code-mode-host.exe` child PID 14248, parent 4156, was also present. Desktop test process PID 2580 was already running because this is an in-process Tauri test, but the app had not been constructed. |
| Pre-Connect, 00:15:58 UTC | Same PIDs 4156, 14248, and 2580; no new Codex/app-server candidate from Desktop construction. |
| Post-Connect, first through final sample | New `codex.exe` PID 14920, parent **2580**, structural `app-server=true`, `stdio=true`, present in all five samples. Existing PIDs 4156 and 14248 remained. No second new candidate appeared. |

Desktop PID 2580 was `F:\Temp\rah-task450-task438-reentry\target\debug\deps\rah_desktop-7dec90348b5a095f.exe`, length 52,879,872 bytes, SHA256 `791089b140dcc73d9b046df002d40ddcc7440d367dd9c7d610e744cd49134f67`. Its process creation time was 08:15:54 local; the census recorded `RAH451_DESKTOP_PID=2580`. The app construction occurred between the pre-launch and pre-Connect samples.

New child PID 14920 was created at 08:15:59 local. Its ancestry began `Desktop/test PID 2580 -> codex.exe PID 14920`, so it was a **direct child**, not merely a process with a matching name. Its canonical executable path was the saved 0.157.1 path above, file length 322,515,248 bytes, and SHA256 exactly the expected hash. Path match: **yes**. Hash match: **yes**. First seen: 00:16:01.511 UTC. Last seen: 00:16:08.478 UTC. Alive at end of observation: **yes**. The diagnostic shut down the live state after observation; it did not Send.

The pre-existing PID 4156 used `C:\Users\morefunfun\AppData\Roaming\npm\node_modules\@openai\codex\node_modules\@openai\codex-win32-x64\vendor\x86_64-pc-windows-msvc\bin\codex.exe`, length 322,515,248, SHA256 equal to the expected binary. Its path did **not** match the saved baseline, and its ancestry was through Node/PowerShell/Code, not through Desktop PID 2580. PID 14248 was its code-mode-host child, with a different executable and hash. Neither was a new Desktop-owned app-server.

## Independent results and decision

| Measure | Result |
| --- | ---: |
| Pre-existing relevant Codex-family processes, excluding Desktop test process | 2 |
| Pre-existing app-server-mode candidates | 0 |
| New app-server candidates observed after Connect | 1 |
| Desktop-owned new app-server candidates | 1 |
| Saved-path matches among new owned candidates | 1 |
| Saved-hash matches among new owned candidates | 1 |
| Transient or replacement app-server PIDs observed | 0 |

Primary outcome **A**: this one production Connect confirmed exactly one persistent Desktop-owned app-server process with both expected saved path and hash. The prior Task 450 combined assertion remains a historical failure with missing evidence. This run supports a narrow **certification-harness** improvement that records independent counts, ownership, path, and hash. It does not prove which predicate failed in Task 450's earlier attempt and does not justify a runtime resolver change. No chat or Tool bridge compatibility was tested, and Task 438 remains **NOT CERTIFIED**.

No resolver, baseline selection, certified list, Connect behavior, provider configuration, Tool bridge, timeout, or authority boundary was changed. RAH remains 0.32.0, HostExplicit remains exactly 11, and v0.33 product capability remains none selected. No commit, push, tag, or release was made.
