# Compact personal plugin management

Date: 2026-11-11

Status: Complete

Replace the My plugins screen's detached per-plugin management buttons with
selectable table rows. The library shows names and status only. Import and
backup/export/restore have their own pages, reached through two shared actions.
Runtime, descriptions, paths, permissions and destructive operations stay in
the selected plugin's details. Preserve confirmation, backup and lifecycle rules.

Extend the existing declarative table with optional row actions, reusable by any
view provider. No plugin-specific rendering in the frontend. Count and validate
the added data against the existing view limits. Existing table payloads remain
valid. Document the service-contract addition in an ADR and process-plugin docs.

Validate API serialization and bounds, the builder's navigation and compact
index, and real-browser import, row selection, enable, configuration, page
actions and disable. Preserve all existing uncommitted work. Do not commit.

## Result and validation

- My plugins now contains a two-column selectable library and two shared
  utility entries. Descriptions, runtime and paths remain in plugin details.
  Successful import returns to the library. Neither sidebar nor Settings renders
  a separate management button for each plugin or utility forms on the index.
- ADR 0077 records the optional UI table `row_actions` contract. Existing table
  JSON is unchanged when the field is empty. No kernel or dependency changes;
  process service-call envelopes and protocol version remain unchanged.
- `cargo test -p rah-api -p rah-plugin-plugin-builder -p rah-plugin-telemetry-log -p rah-plugin-host-process --lib --no-fail-fast`: passed.
- `cargo test -p rah-cli --test web_browser --test web --test plugin_builder --test architecture --no-fail-fast`:
  56 passed, including all 15 browser-target tests with Microsoft Edge.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: passed.
- `cargo fmt --check`, JavaScript syntax and `git diff --check`: passed.
- Inspected `compact-my-plugins-sidebar.png` and `compact-personal-library.png`
  in the temporary screenshot directory. The browser test imports all three
  examples, asserts the index has one selectable row per plugin and no forms,
  opens the transfer page, returns to the library, checks the sidebar and
  completes enable/configure/action/disable through the existing confirmations.
- Existing uncommitted changes are preserved. No commit or push. The full
  workspace test suite was not run. Running installations need the updated host
  and embedded frontend for selectable rows.
