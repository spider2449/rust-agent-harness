# Task 042: Codex baseline and live release gate

## Decision

Select `codex-cli 0.149.0` as the one exact v0.4 Codex baseline. The established
local native executable reports 0.149.0 and no trusted native 0.148.0 executable
is available for the release gate.

## Compatibility evidence

The 0.149.0 `app-server generate-json-schema --experimental` output retains every
captured required field for initialize, thread start/resume, turn start/interrupt,
agent-message deltas, terminal completion, and Dynamic Tool Bridge calls/results.
The observed contract change is additive schema expansion only. The restricted
thread configuration remains `approvalPolicy: never`, read-only sandbox, empty
Codex MCP configuration, and disabled shell, unified execution, web, image, and
apps.

## Scope

Update the exact adapter pin and captured schema contract without widening version
support or changing RAH public APIs. The remaining gate is opt-in trusted-profile
composition through the Generic Codex Tool Bridge.
