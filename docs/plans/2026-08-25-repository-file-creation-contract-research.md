# Task 084 — Repository File Creation Contract and ADR Research

Status: Complete (research/design only)
Date: 2026-08-25

## Outcome

Task 084 selects `repo.create-file`: one host-authorized, create-only,
repository-bound UTF-8 file per call. The complete contract, platform hardening,
failure semantics, profile/bridge fit, deterministic matrix, and certified live
gate are in `docs/RAH_REPOSITORY_FILE_CREATION_CONTRACT.md`. ADR 0013 is
Proposed rather than Accepted; this is a new authority class, not an ADR 0012
extension.

## Task 085 implementation sequence

1. Obtain acceptance for ADR 0013 without changing its authority boundary.
2. Implement the narrow `rah-tools` policy/tool and platform-native create-new
   hardening, with no new dependency unless implementation evidence requires it.
3. Add deterministic Windows/Ubuntu, fault-seam, Git invariant, and security
   tests; run the full workspace gate.
4. Keep trusted-profile/effective composition and bridge/live expansion out of
   Task 085 unless separately authorized after the core contract is proven.

## Non-goals retained

No Rust/Cargo changes, profile schema/version change, Generic Tool Bridge
change, Codex baseline change, release preparation, version bump, or file
creation implementation is part of Task 084.
