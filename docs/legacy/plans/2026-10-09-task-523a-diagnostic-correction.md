# Task 523A — Diagnostic correction and one human repo.list probe

Starting checkpoint: `80fc345b1ff63e7f7c787abf6c15b785bee5e6e0`.
Preserve all pre-existing reports and Task 523 private artifacts.

1. Review retained seven E0433 errors and original source fingerprints.
2. Prepare a new temporary patch using existing production dependencies;
   runtime passes typed diagnostic data to the existing Tool-side serializer.
   Keep capture opt-in, sanitized, bounded and correlated. No listing algorithm,
   authority, dependency, version or release changes.
3. Review source/dependencies, then run fmt check, targeted runtime release check,
   Desktop release check, and exactly one diagnostic Desktop build, in order.
   Stop on the first failed gate; preserve command, exit and compiler logs.
4. Verify one bounded fixture and evidence delivery before launching for one
   human-operated native llama.cpp request. Fixture success is not live acceptance.
5. Classify the actual correlated outcome, or READY_FOR_HUMAN_PROBE if unavailable.
6. Preserve diagnostic source/evidence privately; restore source fingerprints.
   Publish documentation only for a useful supported conclusion, with exact-head CI.

## Disposition — C: DIAGNOSTIC BUILD OR CAPTURE INFRASTRUCTURE BLOCKED

The new preparation command `& target/task523a/private/prepare.ps1` exited 1
before any production source was written. Its manifest reader used
`@(Get-Content ... | ConvertFrom-Json)`, treating the JSON array as one nested
collection. The collection comparison incorrectly raised
`Original fingerprint mismatch`. A subsequent read-only, enumerated inspection
confirmed all seven current source SHA-256 values equal both the retained
original manifest and backups. This is a Task 523A preparation infrastructure
defect, separate from Task 523's seven compiler E0433 errors.

No script correction or retry was performed. The proposed typed serializer
patch was not applied because the helper file had not been created.
Required fmt check, runtime release check, Desktop release check, full build,
fixture and human probe were all NOT RUN. There is no diagnostic executable
identity and no actual repo.list product evidence from this attempt.

Private evidence: `target/task523a/private/prepare.ps1`,
`disposition.json`, `source-verification.json`, `retained-inventory.json`.
All retained Task 523 material remains in place. No source restoration was
necessary; no tracked source, dependency, authority, ADR, version or release
change occurred. HEAD and origin/master remain the authoritative checkpoint.
This local disposition supplies no new product conclusion and is not published.
Next work requires a separately documented preparation correction attempt;
enumerate the manifest before comparison and preserve this first failure.
