# Task 510B2F — certification test Common Controls v6 manifest

Starting HEAD: `436470337269546a1209d0b9803c2b8aa9e1f6b4`. Evidence is under
`F:/temp/task510b2f-evidence`; the fresh target was
`F:/temp/rah-task510b2f-target`. Preserved Task 510B–510B2E WIP remains dirty.

## Correction

Task 510B2E established L3: the certification integration executable imported
`comctl32!TaskDialogIndirect` but had no embedded manifest, while the normal
Desktop sibling had Tauri's Common Controls v6 resource. Tauri build 2.6.3
generates `OUT_DIR/resource.lib` through tauri-winres/embed-resource and emits
`cargo:rustc-link-arg-bins`; the `[[test]]` target is a Cargo test target and
therefore missed that binary-only directive.

M1 was selected. `crates/rah-desktop/build.rs` now reuses the generated
`resource.lib` and emits `cargo:rustc-link-arg-tests` only for Windows/MSVC when
the explicit `certification-harness` feature is enabled. It asserts the resource
exists. No new dependency, separate manifest, production resource change,
runtime behavior, admission policy, or authority was introduced. The first
`git diff -- crates/rah-desktop` guard matched Windows/test-target linkage only.
M2 and M3 were unnecessary.

## Evidence

The fresh Cargo build passed (`certification-build.log`) and produced
`codex_certification-d52d592a41ace069.exe`. Windows SDK `mt.exe` extracted
resource #1 successfully. Its dependency exactly matched the known-good Desktop
sibling (`manifest-equivalence.log`):

```xml
<assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*" />
```

Direct launch succeeded and listed 380 tests. The presentation regression passed
`1 passed / 0 failed / 379 filtered` and no loader error occurred. Typed
verification passed `3 passed / 0 failed / 103 filtered`; exact MissingArtifact,
HashMismatch, and VersionMismatch evidence remained process-local and public
diagnostics were sanitized. Adapter and Desktop all-features checks passed.
The default graph disabled certification and the explicit graph enabled it.

The exact installed candidate controls passed: correct path/hash/version were
measured, wrong path/hash/version failed in the intended typed stages, and normal
production rejected 0.160.0 before app-server startup. The candidate pre/post
SHA was unchanged:
`fdda5fa3cf3fb3d000b876720742857676293e4315e4b045fae6f8bd7e866d1d`.

The candidate smoke reached the real `rah-runtime-codex`, app-server handshake,
neutral runtime, and Desktop backend composition. It asserted clean disconnect,
runtime shutdown, unchanged pre/post SHA, and inference/Tools `0/0`. It then
failed at `crates/rah-desktop/src/certification_tests.rs:214:35` while removing
the temporary workspace: Windows error 32, “file is being used by another
process.” Exact result: `0 passed / 1 failed / 379 filtered`, exit 101. No retry
or cleanup mutation was performed. Source hashes were frozen and unchanged.

## Stop and classification

Full deterministic gates, canonical Desktop counts, Tauri `47/47/47/47/47`,
metadata, and HostExplicit `11` were not rerun after this first stable failure.
Cargo.lock has no diff; no commit, push, CI, tag, release, version bump, admission
change, or preferred-baseline change occurred. ADR-B applies under existing ADR
0030. Codex 0.160.0 is not certified and Task 510B is not resumed.

**D — LOADER IS FIXED BUT PRESENTATION/SEAM VALIDATION FAILS.**

The next authorized task is bounded diagnosis of the exact cleanup lock owner;
preserve this evidence and do not resume Task 510B automatically.

Task 510B2G continues this diagnosis in
`docs/plans/2026-10-04-task-510b2g-certification-cleanup-lock-owner-diagnosis.md`.
