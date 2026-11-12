# Visible Graphify indexing status

## Scope

Show automatic graph generation and update state in RAH, using the existing
process-plugin `ui-views` contract and standard view nodes. No kernel, frontend,
service-contract or installed binary change is needed. Keep the query tool and
its read assignment unchanged and preserve the user's running application.

## Design

- The managed MCP launcher publishes an atomic, bounded status document with
  workspace, state/stage, source-file progress, node/edge counts, start/completion
  timestamps, last error, publisher identity and heartbeat.
- Status has both a workspace copy and a host-process-scoped runtime copy. The
  latter lets a small Graphify status-view process follow workspace switches
  without confusing simultaneous RAH instances. Both are Graphify integration
  artifacts; UI is contributed through the existing UiViews service.
- Initialize MCP against the prior graph or a clearly marked empty placeholder
  before building. Model tool calls still wait for a successfully refreshed
  graph. This lets RAH's UI open while the initial build is running.
- A contributed Graphify page reads status only, sends `ui/changed` when display
  data changes, and shows waiting/building/ready/error/stopped/unresponsive
  state, progress, counts, elapsed time and last successful update. A stale or
  dead publisher is never displayed as actively watching.
- No additional model-visible status tool, automatic graph-first prompting,
  background daemon or inference calls. The view process ends with its scope.

## Validation and installation

Use deterministic Node tests for status transitions, progress parsing, heartbeat
health, bounded/untrusted status data and the status-view protocol. Extend the
ignored native RAH integration test to verify UiViews and notifications during
real generation, failure recovery and workspace switching. Keep normal tests
offline. Run formatting, targeted Clippy and diff checks.

Back up configuration and installed scripts, deploy both scripts, and validate
an isolated installed-RAH profile. Persist the contributed view and status-path
settings. Apply through the running application's existing plugin API when
available without interrupting an active turn; otherwise leave changes ready
for the next launch and clearly report that limit. No commit is authorized.

## Completed implementation and validation

The launcher now publishes version-1 status documents through `status.mjs`.
The workspace copy is `.rah/graphify-out/status.json`; host-scoped copies use
`graphify-<RAH pid>.json` under the configured runtime directory. Progress comes
from Graphify's actual source-discovery and AST parsing output. Parsing all files
does not mark a graph ready: relationship resolution and atomic publication
must finish first. Last-success timestamps/counts survive a failed rebuild.

`status-view.mjs` contributes **Plugins > Plugin pages > Graphify** through
process protocol 4 and `ui-views`. It has no tools, actions or host-service
requirements. The standard view nodes show state, stage, file counts, graph
counts, elapsed/build time, last indexed time and the last error. It polls once
per second and emits `ui/changed` only when displayed values change. Heartbeats
run every five seconds; dead publishers or a heartbeat over fifteen seconds old
are never described as actively watching. Status input is limited to 16 KiB.

MCP initialization no longer waits for the first extraction. It uses a previous
graph or an explicit placeholder while background generation starts; every
model tool call still waits for a refreshed graph and cannot answer from that
placeholder. This allows the frontend and status page to become available while
indexing is still running.

Validation actually executed:

- `node --test examples/graphify/managed-mcp.test.mjs examples/graphify/status.test.mjs`:
  18 passed, including building/ready/error transitions, actual progress parsing,
  placeholder gating, previous-success retention, dead/stale publisher health,
  malformed/oversized status input, bounded text and the process-view protocol's
  automatic notifications and shutdown.
- `cargo test -p rah-cli --test graphify_evaluation graphify_managed_workspace_updates -- --ignored --exact --nocapture`:
  passed in 17.92 s with real Graphify and a mock model. It verifies UI service
  registration, the frontend's authenticated `/api/ui` page listing and rendered
  status endpoint, real file progress and ready timestamps, error display,
  workspace switches, notifications and stopped-publisher display. No live-model
  inference was used.
- `cargo fmt --check`, targeted Clippy with `-D warnings`, Node syntax checking
  and `git diff --check` passed. No kernel, frontend code, service contract,
  protocol version, ADR or crate dependency was changed.

## Installed state

All three deployed modules match their verified sources under
`C:\Users\spider.tp\.rah\plugins\graphify\`. The active configuration now has
`RAH_GRAPHIFY_STATUS_DIR = C:\Users\spider.tp\.rah\runtime\graphify` on the existing
MCP instance and a read-only process-view instance named `graphify-status` with
the same status directory. Existing query names/permissions, model, root and
other plugin settings were preserved. The installed RAH accepted the final
23-plugin configuration with a one-run mock-model override.

Backup directory:
`C:\Users\spider.tp\.rah\config-backups\20261112-160907-graphify-status`.
It contains the original `rah.toml.before` and the prior launcher. Reversing only
the status-view mount and new MCP environment entry reproduced the original
configuration exactly. The user application was left running; the new mount and
environment entry load on its next launch/restart. No commit was created.

## Visual-verification limit

Direct inspection of the existing desktop window was unavailable: the Windows
Computer Use helper returned a missing native pipe (OS error 2), and browser
inventory exposed no connected surfaces. Automatic approval review also rejected
an attempted isolated installed-RAH background start/stop command with only
`blocked by policy`; that command did not execute. Validation therefore used
the safer existing in-process RAH integration test and authenticated frontend
API, without starting/stopping any user desktop window. Current-window pixel
verification was not claimed. See KNOWN_ISSUES.md for the remaining environment
limit.
