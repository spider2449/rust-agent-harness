# History search performance

Measure authenticated HTTP history search with deterministic synthetic local JSONL logs. Cover missing queries and a match in the oldest session, so the full corpus must be scanned. Preserve prior uncommitted search changes. Optimize only measured bottlenecks, retain correctness and avoid new dependencies or service contracts. Validate with regression tests and repeat the same benchmark. No live model, network service, or user history is needed.

## Measurements

Local Windows debug/test builds, synthetic data only. These are individual observations, not release performance guarantees or statistically controlled comparisons.

| Workload | Before | After |
| --- | --- | --- |
| 500 sessions / 40,000 messages / 26,328,376 bytes, first missing query | 2.225 s | 1.569 s |
| Same corpus, oldest-session match | 727 ms | 581 ms |
| Same corpus, repeated missing query | 693 ms | 592 ms |
| Five matches near the end of a 1,114,304-byte message, 192-character query | 22.193 s | 489 ms |

The long-message benchmark exposed repeated allocation and lowercase conversion at every candidate position. The implementation now finds the match once in the lowercase text and maps its byte offset back to original characters. Added regressions for a match inside a lowercase expansion and contextual Greek final sigma. The isolated long-message case improved about 45 times; full-history timings also include file I/O, metadata and JSON parsing, and are affected by filesystem caches.

## Reproduction

- `cargo test -p rah-cli --test web history_search_large_history_latency -- --ignored --nocapture`
- `cargo test -p rah-plugin-frontend-web long_message_search_latency -- --ignored --nocapture`

The corpus has deterministic content and timestamps; unique session/turn identifiers isolate the fixture. Both benchmarks assert correctness but deliberately have no timing threshold in the normal suite. Temporary history files are removed when the test fixture is dropped.

## Scope and remaining limits

Changed frontend-web's excerpt matching, its Unicode regressions and opt-in timing test, plus an HTTP benchmark in `crates/rah-cli/tests/web.rs`. No dependency, ADR, kernel, plugin protocol or service contract changes. No persistent index or cross-request cache: freshness continues to follow the existing session service. First-request scans still take over a second on this fixture in debug mode; the fixture does not clear operating-system file caches. Suggested next task: collect release-mode timings and representative corpus sizes before designing an index.

## Validation

Passed `cargo test -p rah-plugin-frontend-web -- --include-ignored --nocapture` (32 tests), `cargo test -p rah-cli --test web history_search -- --include-ignored --nocapture` (2 tests), the session lifecycle browser test with local Chrome via `RAH_TEST_BROWSER`, `cargo clippy -p rah-plugin-frontend-web --all-targets -- -D warnings`, `cargo fmt --check`, `node --check plugins/frontend-web/assets/app.js` and `git diff --check`. The full workspace suite was not run for this localized change. Changes remain uncommitted and the running application has not been replaced.

## Release validation

Measured the same optimized implementation and 500-session corpus in release mode on 2026-11-12. Three runs, each with a fresh synthetic fixture:

| HTTP query | Run 1 | Run 2 | Run 3 | Median |
| --- | --- | --- | --- | --- |
| First missing query | 402.383 ms | 380.418 ms | 384.720 ms | 384.720 ms |
| Oldest-session match | 173.826 ms | 152.787 ms | 165.842 ms | 165.842 ms |
| Repeated missing query | 182.023 ms | 151.862 ms | 168.966 ms | 168.966 ms |

These are end-to-end HTTP observations. Corpus generation immediately precedes the requests, so the first request is not an OS cold-cache measurement. The first run also ran the HTTP correctness regression. Compilation is excluded from all timings; concurrent local work and filesystem caching can affect results. The existing UI debounce adds 250 ms before issuing a query.

Commands: `cargo test --release -p rah-cli --test web history_search -- --include-ignored --nocapture`, followed by two direct invocations of the generated release integration-test executable filtered to `history_search_large_history_latency --ignored --nocapture`.

At this tested scale, full scans finish within about 0.15-0.40 seconds in release mode. Defer a persistent index until representative larger corpora or actual interactive latency justify the storage, invalidation and migration complexity. This is a workload-specific decision, not a latency guarantee.

Release plugin validation also passed: `cargo test --release -p rah-plugin-frontend-web -- --include-ignored --nocapture` (32 tests). Five end-of-message matches in the 1,114,304-byte fixture took 52.497 ms total. HTTP correctness plus all three corpus runs passed. Only this measurement document changed during the release-validation step; prior implementation changes remain uncommitted. No ADR or plugin-contract impact, and no running installation was replaced.
