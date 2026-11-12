# Session history search

## Scope
Extend the Web UI session search beyond metadata to user and assistant messages using the existing SessionLog service. Return matching snippets and record positions, and open results at the matching message. Keep all session data local and read-only; no kernel, service contract or protocol changes.

## Acceptance and validation
- Case-insensitive full-history matching, including Chinese text.
- Display a matching snippet and navigate to its record.
- Debounce requests and reject stale responses; report failures.
- Preserve metadata filtering and existing folder grouping.
- Run frontend plugin tests, formatting and diff checks.

## Limits
Scan session history on demand without a persistent index. Return up to 100 matching sessions, newest first, with the first matching message per session. Tool payload search and semantic search are outside this task.

## Result
Implemented in frontend-web with the existing SessionLog contract. Added the authenticated read-only search endpoint, debounced and cancellable UI requests, snippets and navigation to the first matching record. No ADR, kernel, protocol or service-contract changes.

Validation passed:
- `cargo test -p rah-plugin-frontend-web` (30 tests).
- `cargo test -p rah-cli --test web history_search_finds_later_messages_and_record_positions`.
- `RAH_TEST_BROWSER` set to local Chrome: `cargo test -p rah-cli --test web_browser sessions_are_named_renamed_branched_exported_and_deleted_in_the_browser`.
- `cargo fmt --check`, JavaScript syntax check and `git diff --check`.

Suggested follow-up: measure search latency against a large local history before introducing a persistent index.
