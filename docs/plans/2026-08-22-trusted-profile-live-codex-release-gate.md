# Task 043: Trusted-Profile Live Codex Release Gate

## Goal

Provide one opt-in, end-to-end validation of the ADR 0011 trusted-profile path
through the Generic Codex Tool Bridge using exactly `codex-cli 0.149.0`.

## Design

`rah-cli` now exposes its existing host-side `profile_composition` module through
its library target. The CLI remains its caller. This is implementation placement,
not a new provider manager or a model-facing API: the composer continues to own
provider construction, exact admission, atomic publication, and shutdown.

The live example creates an absolute temporary profile file and a temporary copy
of the native `rah-plugin-echo` fixture. It loads that path via
`TrustedStaticProfile::load`, invokes the same `compose` function as
`rah profile validate-effective`, passes the resulting fresh registry to
`CodexRuntime::connect_tool_bridge`, and makes one `plugin.test.echo` call with
`PermissionLevel::None`.

The fixture lifecycle marker is validation-only fixture instrumentation. It
records `spawn`, one `call`, `shutdown`, and `exit`, so the example can reject
replay and verify provider cleanup without exposing a path, profile body,
environment, or stderr in normal output.

## Acceptance checks

- Static source/schema validation succeeds from an explicit absolute path.
- Effective composition yields exactly one validated Process Plugin-backed tool
  in a fresh registry with a redacted inventory and owned lifecycle.
- The bridge uses its private `rah_tool_0` alias and admits only `None`.
- The run observes exactly one each of ToolRequested, ToolStarted, and
  ToolFinished, then Codex continuation and terminal Completed.
- The fixture records exactly one provider call and clean child shutdown.
- Normal workspace tests remain offline and do not invoke this example.

## Result

Passed on 2026-08-22 with `codex-cli 0.149.0`. The Process Plugin profile
admitted only `plugin.test.echo` with `PermissionLevel::None`; its private
Codex alias was `rah_tool_0`. The live turn observed exactly one ToolRequested,
ToolStarted, ToolFinished, and fixture provider call, followed by ModelDelta
continuation and terminal Completed with the required marker. The Codex
app-server and fixture child were both reaped.
