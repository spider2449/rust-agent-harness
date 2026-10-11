# Task 119 — ADR 0015 bounded model-provider network endpoint authority

## Scope

Starting from clean `master` at
`d63825f4f25325e1eb61844fbe20e445f1ba7c71` (Task 118 complete; exact-head
GitHub Actions CI #108 / `33128374221` successful), accept ADR 0015 before any
Desktop endpoint implementation.

## Decision recorded

ADR 0015 authorizes only a trusted human/Desktop host to select one structured
initial `llama_cpp` provider endpoint for one explicit connection: closed
`http|https`, IPv4/IPv6/DNS host, numeric port, and Rust-synthesized fixed
`/v1`. It records non-loopback HTTP as an explicit insecure host choice,
initial-endpoint rather than redirect/proxy-confinement authority, no
credentials or persistence, captured connection snapshots, no replay, and no
provider process lifecycle.

## Task 120 boundary

Task 120 may implement only the Desktop-private structured validation, closed
IPC, model-generation/reconnect behavior, adapter configuration, deterministic
tests, and two already-running-server live proofs specified by ADR 0015. It
must use `credential_environment_variable = None` and must not add generic
network, ToolRegistry, MCP, Git, browser, process, provider lifecycle, or
persistence authority.

## Validation

Documentation-only validation is `git diff --check`, changed-file inspection,
`git diff --stat`, `git diff`, and `git status --short`. No Cargo command is
required solely for this ADR/documentation task.
