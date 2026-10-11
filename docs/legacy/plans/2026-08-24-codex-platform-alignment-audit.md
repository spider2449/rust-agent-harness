# Task 075 — Codex Platform Alignment Audit

Date: 2026-08-24

## Scope

Research and documentation only. Audit the Task 074 RAH/Codex baseline against
OpenAI's current Codex platform article and official app-server/SDK documentation.
No production Rust, runtime, ToolRegistry, permission, profile-schema, baseline
version, persistence, approval-remapping, release-preparation, or baseline-script
implementation is authorized.

## Deliverable

`docs/RAH_CODEX_PLATFORM_ALIGNMENT_AUDIT.md` records evidence classes, the
app-server-primary recommendation, SDK/exec/internal-library comparison,
authority/approval and thread/session conclusions, persistence guidance, bridge
and sandbox implications, protocol-schema audit recommendation, three-tier
baseline policy, and the Task 076 scope recommendation.

## Validation gates

Run docs-only repository gates without changing dependencies. Confirm 11 workspace
packages remain at version `0.6.0`, edition 2024, and no dependency changes occur.

## Commit boundary

One documentation-only commit: `docs: audit Codex platform alignment`.

## Stop condition

Stop after Task 075. Task 076 is proposed only; it is not implemented here.
