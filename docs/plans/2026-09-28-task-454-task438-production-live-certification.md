# Task 454 — production Desktop live certification

Date: 2026-09-28. Disposition: **live PASS; deterministic Clippy STOP; Task 438 NOT CERTIFIED**.

The single production Desktop session used the saved Codex CLI 0.157.1 executable (SHA256 `8cb0e69e99ff2a158c54815db82d0f2e524d8f301bc30184722cfd1ae5973574`) and `gpt-6-luna`. Connect observed exactly one Desktop-owned candidate, verified Windows file-object identity and SHA256, and found it alive. The neutral chat started, produced assistant output, and completed normally without a Tool request or provider/configuration failure. The real `repo.file-info` turn requested, started, and finished one Tool execution, returned the expected clean tracked fixture facts, continued with assistant output, and completed.

The subsequent `cargo clippy --workspace --all-targets --all-features -- -D warnings` failed in `crates/rah-desktop/src/main_tests.rs`: `clippy::cloned_ref_to_slice_refs` at 25836 and `clippy::await_holding_lock` from 26081 through the await at 26101. These are certification harness findings. No production failure was established. The run stopped there; no final deterministic gates, certification commit, push, or exact-head CI were claimed. Task 454's live PASS remains historical evidence. Task 455 corrects and revalidates the harness.
