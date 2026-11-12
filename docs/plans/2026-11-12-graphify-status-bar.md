# Graphify status-bar indicator

## Scope

Add a live Graphify icon to the bottom status bar. Show busy/ready/error state,
an accessible tooltip and a click-through to its existing status page. Preserve
the detailed page, automatic indexing, existing worktree changes and user config.

## Design

Extend the existing declarative UiViews contract with an opt-in ViewSpec flag
and one bounded status node per view (ADR 0078). Frontend-web owns placement,
icons and colors, so a plugin cannot hide built-in status or inject markup.
Graphify supplies its indicator through the same status-view process and
UiChanged events, including when its detail page is not open. Unmounted providers
disappear; render/connection failures cannot leave a stale green icon.

No kernel logic, model tools, policy changes or new crate dependencies. Review
ADR 0036, the pivot plan and ADR 0062 before changing the additive API contract.
The process plugin emits the new fields only when explicitly configured, so the
old status-page mode remains usable on an older host.

## Validation and deployment

Add deterministic API bounds/default tests, process-view tests, Graphify status
tests and a real-browser test for footer updates, detail click and provider
removal. Run relevant Rust/Node tests, formatting, targeted Clippy and diff
checks. Build CLI/desktop binaries, back up deployed files, and install when the
application is not running; preserve active sessions and report any deployment
limit. Do not commit or run paid/live-model evaluations.

## Result

Implemented the additive contract in `rah-api::ui`, discovery in frontend-web,
fixed graph/activity SVG icons and bounded footer rendering. Graphify's process
view opts in with `plugin.status_bar = true`. It supplies busy file progress,
ready/error/waiting/stopped/stale states and a tooltip. Clicking opens the
existing Plugins detail page. No kernel, service trait, process-method, tool
permission or dependency changes were made. ADR 0078 records the data extension.

Validation completed:

- 19 Node tests for managed indexing, status health, footer mapping and protocol.
- Nine `rah-api` UI tests, including legacy metadata and footer bounds.
- The real Graphify managed-workspace integration test passed with footer
  discovery and ready-node assertions (28.37 seconds, mock model, cached package).
- Real headless Edge footer test passed (4.11 seconds): pushed busy/ready/error,
  visible detail click, removal and no page errors. The screenshot was inspected
  at `target/graphify-ui/graphify-status-bar.png`.
- Web API: 34 passed, one synthetic performance test intentionally ignored;
  process-plugin and architecture suites: four passed each.
- Release CLI/desktop build, workspace check, formatting, targeted all-target
  Clippy, JavaScript syntax and diff checks passed.

Installed CLI/desktop binaries and `status-view.mjs` were backed up and updated.
The active `C:\Users\spider.tp\.rah\rah.toml` enables the footer flag, preserving
all other text/settings; installed binary/script hashes match the build/source.
Both staged and installed configuration checks accepted 23 plugins with a
one-run mock override; the saved model remains `llamacpp`.

Backup: `C:\Users\spider.tp\.rah\config-backups\20261112-165111-graphify-status-bar`.
No RAH app was running during installation. The feature loads at the next launch;
no user application was started or stopped. Current desktop-window inspection
remains unavailable as recorded in KNOWN_ISSUES.md. Existing unrelated work was
preserved and no commit was made. Full workspace tests were not run. Suggested
next check: open RAH normally and observe indexing for the selected workspace.
