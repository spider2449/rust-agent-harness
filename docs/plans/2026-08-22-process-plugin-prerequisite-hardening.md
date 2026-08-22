# Task 037 - Process Plugin prerequisite hardening

## Scope

Harden `rah-tools-plugin` before any future trusted-profile composition. This
task does not add Process Plugin entries to the trusted capability profile.

## Design

- Capture a host-selected absolute executable identity before setup and
  revalidate it immediately before spawn. Windows accepts native `.exe` only;
  direct symlinks/reparse points, scripts, directories, and non-regular files
  fail closed. Unix requires an executable regular non-link file.
- Keep the check adapter-local. MCP has equivalent private semantics, but a
  shared helper would create an unnecessary generic process-security surface.
- Retain configured plugin ID as the identity namespace. The handshake ID and
  version must exactly match it.
- Add a host-facing `with_expected_tool` contract binding remote name, object
  schema, and explicit permission. Discovery must equal the configured set;
  normalized JSON schema equality is exact and recursively key-order neutral.
- Build proxies only after full validation. Any admission failure stops and
  reaps the child and returns no adapter/tool set.

## Validation

Run deterministic fixture modes for missing, extra, duplicate, invalid, and
malformed discovery; schema drifts; timeout and child exit. Retain lifecycle,
cancellation, bounded diagnostics, cwd, and environment tests. Run the full
workspace formatting, check, tests, clippy, diff, and metadata gates.

## Residual limitation

Revalidation narrows but cannot eliminate replacement TOCTOU between the last
filesystem check and process spawn. Process supervision is not OS sandboxing.
