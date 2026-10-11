# Task 163A — Diagnose and fix Desktop live `repo.delete-file` availability

## Scope

Trace the real Windows Desktop selected-repository, connect, reconnect, and
Codex Generic Tool Bridge construction path for `repo.delete-file`. Fix only
the proven divergence from the Task 161/162 host-owned composition path.

## Gates

- Preserve ADR 0011 and ADR 0017 authority boundaries.
- Keep missing deletion authority fail-closed and avoid frontend/profile/model
  authority creation or generic filesystem/Git/shell access.
- Add deterministic live-like construction coverage for authorized,
  unauthorized, reconnect/generation, no widening, and duplicate registration.
- Update the Task 163 evidence document with the diagnosis and remaining live
  requirement.
- Run the requested workspace validation, commit and push one coherent fix,
  verify exact-head CI, then rerun the real Windows live validation.

## Completion boundary

Deterministic tests do not complete Task 163. Completion still requires the
real Windows live lifecycle and filesystem/Git evidence recorded by the Task
163 gate.
