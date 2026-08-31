# Task 149 - Desktop `repo.commit` integration

## Scope

Register the existing, host-composed `RepositoryCommitTool` in Desktop's
existing Generic Tool Bridge only when the selected repository and explicit
commit identity produce the paired tool/control. Retain the control only in
Rust, preserve the Execute gate, and surface only the bounded commit
disposition plus a verified commit OID in Desktop activity.

## Constraints

The model supplies only the existing `message` input. A Desktop click authorizes
one opaque reviewed snapshot but does not commit. Registration, conversation
history, frontend state, and a Tool call never create authority. A result of
`uncertain` is displayed without retry or replay. This task changes neither
the Generic Tool Bridge nor any RAH protocol/public Tool contract.

## Verification

Add deterministic Desktop tests proving the registry receives the exact paired
commit tool only under the host-owned capability conditions, the redacted
result parser accepts only known dispositions, and a commit result requests a
repository refresh. Run focused Desktop and `rah-tools` tests, formatting,
frontend syntax, diff checks, and relevant Clippy.
