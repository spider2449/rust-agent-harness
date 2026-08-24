# Multi-replacement repository patch bridge validation

Date: 2026-08-24

## Baseline and scope

Task 073 validates Task 072 commit `024d460e992e31a682ab1b1583b60fb939e220f4`
through the existing trusted static profile, effective composer, fresh
`ToolRegistry`, and Generic Tool Bridge. It adds deterministic tests only.
There is no production trusted-profile or bridge change, profile version remains
`1`, and the canonical capability remains `repo.patch` with `Execute`
permission.

## Evidence

The bridge advertises the composed `ToolDefinition` without a copied schema.
The definition has two closed `oneOf` forms: legacy single replacement and a
`replacements` form with one through sixteen entries. The deterministic alias
for a profile containing only `repo.patch` is `rah_tool_0`; it remains private
while canonical lifecycle identity remains `repo.patch`.

The tests use a real tracked UTF-8 repository fixture. A single dynamic bridge
call applies three independent replacements, preserves a sentinel and unrelated
file, and verifies postimage, HEAD, refs, index bytes, and no staging. The same
`threadId + turnId + callId` delivery is deduplicated, while a new call ID is
executed as a distinct legacy-form call. `None`, `Read`, and `Write` permission
grants are denied before tool entry, while `Execute` is allowed.

The fail-closed matrix covers empty and seventeen-item lists, mixed forms,
duplicate and overlapping targets, generated-match dependencies, repeated
source matches, and stale preconditions. Existing bridge tests retain canonical
name and unknown-alias rejection, cancellation before entry, cancellation after
possible commit, disconnect handling, output redaction, and restricted
Codex-owned capabilities.

## Uncertainty boundary

Task 072's post-commit fault seam is private to `rah-tools` unit compilation and
is intentionally not exposed through a production API. Task 072 directly proves
the mutation uncertainty taxonomy. The existing composed bridge test wraps the
real tool only after it returns, reports an uncertain result, and proves duplicate
call identity does not replay the committed mutation. This gives layered
repository and bridge no-replay coverage without weakening encapsulation.

## Limitations

This task uses the fake app-server transport only. It does not perform live
Codex validation, alter authority, add a capability, or prepare a release.

## Validation

Run the focused runtime-codex, tools, and CLI test suites plus workspace format,
check, test, clippy, diff, and metadata gates. The subsequent task is live Codex
validation of one multi-replacement `repo.patch` call with repository observers
and the same restricted Codex-owned capability configuration.
