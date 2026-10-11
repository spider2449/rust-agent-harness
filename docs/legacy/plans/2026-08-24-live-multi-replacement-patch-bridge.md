# Task 074: Windows Live Multi-Replacement repo.patch Bridge Validation

Baseline: Task 073 commit `fa42c0768eb157328891559d8f1b2ddec1f283ee`, with native `codex-cli 0.149.0`.

Run the opt-in fixture with:

```powershell
cargo run -p rah-runtime-codex --example live_trusted_profile_multi_patch_bridge
```

The example creates and commits a fresh temporary Git repository. Its tracked target starts with `alpha = 1`, `beta = 2`, `gamma = 3`, and an unchanged sentinel. It also creates a tracked sentinel control. A real `TrustedStaticProfile::load` and `rah-cli` effective composition construct a fresh registry containing only `repo.patch`, `repo.file-info`, `repo.status`, `repo.diff`, and `repo.diff-staged`, all with `PermissionLevel::Execute`.

The bridge publishes deterministic private aliases in registry-definition order: `repo.diff` -> `rah_tool_0`, `repo.diff-staged` -> `rah_tool_1`, `repo.file-info` -> `rah_tool_2`, `repo.patch` -> `rah_tool_3`, and `repo.status` -> `rah_tool_4`. The live output records this actual mapping rather than relying on a solo-profile alias.

The prompt requires repository observations, one and only one `repo.patch` request in the `replacements` form, then post-mutation observations. The harness independently requires the exact preimage SHA-256 and length, exactly three replacements (`alpha`, `beta`, and `gamma`), absence of legacy old/new fields, a complete requested/started/finished lifecycle, and one underlying replacement execution.

Post-mutation observer assertions require a tracked regular modified target with the exact postimage hash and length, target-only status, a single non-binary semantic diff containing all three intended lines, and an empty staged diff. Raw `.git/index` bytes, HEAD, refs, tracked sentinel bytes, and staged diff remain unchanged. The fixture also requires `Completed`, app-server reaping, and temporary repository cleanup.

Codex-owned shell, arbitrary file writes, MCP, model-selected process execution, network/web, image, apps, and approval bypasses remain disabled. The marker `RAH_MULTI_PATCH_LIVE_OK` is printed only after all assertions pass. This is a Windows live Codex validation claim only; deterministic Ubuntu CI is separate evidence and is not Unix live validation.

On 2026-08-24, three fresh runs with `codex-cli 0.149.0` passed. Each recorded `repo.patch` as `rah_tool_3` with requested/started/finished/native-execution counts of `1/1/1/1`; `repo.file-info` and `repo.status` each ran twice, while `repo.diff` and `repo.diff-staged` each ran once. Every run reached `Completed`, reaped the app-server, removed the temporary fixture, and printed the exact marker.
