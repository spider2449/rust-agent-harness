# Task 225A - MCP startup timeout test portability fix

## Scope

This maintenance change makes the deterministic MCP startup-timeout test
scheduler-tolerant. It changes no MCP product behavior, timeout policy,
authority boundary, dependency, or v0.19 feature.

## Diagnosis

On Windows development runs, the test's old precise assertion required each
failed connection to complete in less than 3 seconds. One pre-change
reproduction completed in approximately 3.25 seconds and failed at that
assertion. Subsequent repetitions completed the two-mode test in approximately
5.05 seconds, demonstrating scheduler/process variance rather than a stable
functional failure.

Production MCP startup uses a 2-second timeout for each initialization or
discovery request. Failed-connect cleanup waits up to `SHUTDOWN_TIMEOUT * 4`,
with `SHUTDOWN_TIMEOUT` equal to 500 ms. The test therefore needs to bound the
whole failed connection, including cleanup, rather than impose a precise
machine wall-clock threshold.

## Test contract

`initialize_and_discovery_timeouts_return_no_partial_provider` now wraps each
`connect_error` call in a 5-second Tokio watchdog. The watchdog proves that
startup timeout plus bounded provider cleanup completes finitely and still
fails if initialization or cleanup hangs. Existing assertions continue to
require an initialization error containing the not-replayed guarantee; no
partial provider is admitted.

This was a test portability defect, not an authority or MCP product failure:
the production startup and cleanup budgets are unchanged, and the test still
checks fail-closed initialization classification and uncertain-operation
non-replay semantics.

## Validation

Validation was performed sequentially on Windows.

- Pre-change isolated repetitions: one failure at approximately 3.25 seconds;
  two later repetitions passed, with the full two-mode test taking
  approximately 5.05 seconds.
- Post-change isolated repetitions: 3 sequential passes, each completing in
  approximately 5.05-5.06 seconds.
- `cargo test -p rah-tools-mcp`: PASS; 1 unit test and 31 integration tests
  passed.
- `cargo fmt --check`: PASS.
- `cargo check --workspace`: PASS.
- `cargo test --workspace`: PASS; all reported test-result sections passed,
  including 172 Desktop tests (2 ignored), 31 MCP integration tests, and 15
  Process Plugin integration tests.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: PASS.
- `git diff --check`: PASS.
- `cargo metadata --no-deps --format-version 1`: PASS; 13 packages, all
  `0.18.0`, edition 2024.

The first workspace-test attempt encountered an unrelated Windows linker
`LNK1285` corrupt-PDB error in a generated target artifact. After that exact
generated artifact was quarantined, the required workspace test command passed.

There were no `Cargo.toml` or `Cargo.lock` changes and no dependency drift.

No production MCP source behavior changed.
