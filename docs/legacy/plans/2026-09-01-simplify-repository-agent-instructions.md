# Task 156 — Simplify Repository Agent Instructions

## Starting state

- Starting SHA: `e80145f450327943adde18150d978de848179ccc`.
- The root `AGENTS.md` had accumulated version-specific implementation and
  release-history instructions that no longer represented durable repository
  guidance.

## Change

- Removed stale implementation-plan framing, phase-specific scope and
  autonomous-execution rules, historical plugin wording, release/task evidence,
  and low-value duplicated style detail.
- Retained durable project identity, provider-neutral public boundaries,
  Tool/ToolRegistry extension rules, host-owned authority, repository-context
  safety, worktree discipline, security precision, validation, and closure
  guidance.
- This documentation-only task changes no architecture, authority, product
  capability, release identity, dependency, or production code.

## Size record

- `AGENTS.md` before: 616 lines, 14,577 bytes.
- `AGENTS.md` after: 191 lines, 7,848 bytes.
- Reduction: 425 lines (69.0%) and 6,729 bytes (46.2%).

## Validation and closure

- Run documentation and workspace integrity validation specified by Task 156.
- Create one documentation commit, push it, and require a new successful
  exact-head CI run before recording closure.
- Commit and CI result: pending closure.
