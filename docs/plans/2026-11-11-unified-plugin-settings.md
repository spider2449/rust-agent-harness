# Unified plugin settings

Date: 2026-11-11

Status: Complete

## Scope

Provide one plugin settings workspace, available in Settings and in the existing
Plugins panel. The workspace has a searchable plugin list and a selected plugin's
status, configuration and lifecycle actions. Personal and built-in plugins use
the same editor. Contributed management pages, including My plugins, are
available in the workspace through the existing `ui-views` service.

Use the catalog's existing configuration examples and key descriptions to build
typed controls. Missing fields remain unset until explicitly enabled. Nested or
unknown values remain editable as JSON, and a complete advanced JSON editor
remains available. Examples are references, never automatically applied defaults.
The backend remains responsible for configuration validation and persistence.

Keep the existing task-oriented Settings shortcuts and layout panel identifiers
compatible. Reuse the same workspace DOM and view action renderer rather than
creating a second configuration store or lifecycle implementation. ADR 0036,
ADR 0062 and ADR 0073 still govern trust and management actions. No kernel,
service contract, protocol or dependency changes are needed.

## Work

1. Add the shared typed editor, preserving omitted and unknown values and JSON
   round trips, with errors beside the editor.
2. Add searchable master/detail plugin management and a Settings entry. Expose
   contributed pages there without giving the model any new action.
3. Exercise field editing, raw JSON, failed validation, unknown values, omitted
   defaults, selection, search and contributed pages in the real-browser tests.
4. Run formatting, relevant Rust and browser tests, and inspect screenshots and
   the final diff. Do not commit or push.

The requested personal plugin examples live in `examples/personal-plugins`:
`text-lab` (a configurable text tool), `scratch-note` (persistent scoped storage)
and `tally-board` (a contributed page with actions). `bundle.mjs` builds an
importable bundle from their source files without running or mounting them.

## Acceptance

- Every configurable installed plugin has the same editor and action placement.
- Built-in and personal plugins share the editor; saved but disabled personal
  plugins remain manageable from the contributed My plugins page.
- Switching between fields and JSON preserves the configuration exactly.
- Saving a field never silently writes an example or drops an unknown key.
- Invalid data leaves the editor open and reports the failure locally.
- Management without the Plugins panel in the layout is available in Settings.
- Existing enable confirmations, policy checks and backup behavior remain owned
  by their existing providers.

## Validation

- `cargo fmt --check`: passed.
- `cargo check --workspace`: passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: passed.
- `cargo test -p rah-plugin-frontend-web -p rah-plugin-plugin-builder --lib --no-fail-fast`:
  53 passed.
- `cargo test -p rah-cli --test web_browser --test web --test plugin_builder --test architecture --no-fail-fast`:
  56 passed, with `RAH_TEST_BROWSER` pointing at Microsoft Edge. The browser
  target ran all 15 tests, including both new scenarios.
- JavaScript syntax checks and `git diff --check`: passed.
- Inspected desktop and narrow-screen screenshots. The list and the selected
  plugin content scroll independently; My plugins is above the installed list.
  A browser assertion requires the sample board's count to stay in the viewport
  even with a long installed-plugin list.
- The sample test imports all three plugins disabled, enables each through the
  existing confirmation, edits nested process configuration, invokes tools through
  `ToolInvoker`, checks note persistence across restart, runs a page action and
  disables a personal plugin. All data is in temporary test directories.

No kernel, service-contract, protocol, ADR or dependency changes. The existing
catalog describes types through examples rather than a complete configuration
schema; complex values use JSON and the plugin remains the validation authority.
The full workspace test suite was not run. No commit or push was made.
