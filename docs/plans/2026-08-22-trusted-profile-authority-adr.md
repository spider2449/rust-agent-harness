# Task 036: Trusted host capability profile authority ADR

## Scope

Record the already implemented trusted static capability profile as RAH's
host-owned authority-composition boundary. This task changes documentation only.

## Plan

1. Confirm ADR numbering and review ADRs 0001 through 0010 plus the current
   architecture, security, README, and trusted-profile implementation evidence.
2. Add accepted ADR 0011 that defines the trusted host profile's source,
   composition, validation, admission, lifecycle, inspection, and fail-closed
   invariants, with implemented and deferred scope stated separately.
3. Add narrow references from architecture, security, and README; remove the
   obsolete claim that the unused ADR 0011 is reserved for worktree mutation.
4. Verify documentation-only scope, whitespace, workspace compilation, and the
   final Git diff before committing the coherent documentation change.

## Acceptance criteria

- ADR 0011 accurately describes implemented built-in and local stdio MCP
  profile composition without claiming Process Plugin composition.
- The ADR preserves existing runtime, Tool/ToolRegistry, execute, repository
  mutation, and provider-adapter authority boundaries.
- Operator-facing documentation distinguishes static validation from effective
  validation and points to the accepted ADR.
- No Rust, Cargo, protocol, or provider implementation changes are present.
