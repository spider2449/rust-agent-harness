# RAH v0.16.0 Release Gate

**RELEASE CANDIDATE PREPARED — NOT RELEASED**

## 1. Release identity

- Release candidate: `RAH v0.16.0`.
- Milestone: Host-owned Effective Authority Review UX.
- Task 197 audit: `aeb9df521522817e62edca661417b02be095a3fe` — PASS.
- Task 197 exact-head CI: run `33847133836` — PASS.
- Task 198 release-preparation commit: to be recorded after commit.
- Exact-head CI for Task 198: required and must PASS.
- No `v0.16.0` tag exists during Task 198.
- No GitHub Release exists during Task 198.

## 2. Immutable prior release

- v0.15.0 release commit: `6b66a357cacea4b1fcf21131cbc9e72fab90d59c`.
- Annotated tag: `v0.15.0`.
- Tag object: `6ca031e66972b5e04dcade6766d6156a9c3e1a9b`.
- These identities must remain unchanged.

## 3. Required evidence chain

| Task | Commit / evidence | Result |
| --- | --- | --- |
| 191 | `fe6cf094f3d5bb7335bbf3eb093140648c07b652` | Scope selected; no new model-accessible side-effect authority. |
| 192 | `279fd1aee9b0465a5782c4ca5c4a9c33e580c2df` | Effective Authority UX contract; no new ADR. |
| 193 | `847c9c12ac04ad8e01dfdfb08813a13093db379b` | Sanitized backend snapshot and read-only command. |
| 194 | `a784727055272d2b253e82aeb713c043cff7ba1a` | Desktop review UX. |
| 195 | `2741719a1034550f92c952acf81acb0f446ac229`; CI `33835056900` | Cross-layer hardening — PASS. |
| 196/196A | Initial attempt INCONCLUSIVE; evidence `6b7aabcceb28245b6780cd560d6ddafe1c4c1ea7`; CI `33845690507` | Human-assisted Windows live validation — PASS. |
| 197 | `aeb9df521522817e62edca661417b02be095a3fe`; CI `33847133836` | Milestone audit — PASS. |

Task 196 was initially INCONCLUSIVE because Desktop-control capability was
unavailable. Task 196A supplied human-assisted GUI evidence; the chronology is
preserved and the final Windows live certification is PASS.

## 4. Product and authority contract

The panel reviews the current host-composed inventory. Configured, effective,
runtime-advertised, and individual request authorization are distinct. A
visible or advertised Tool does not guarantee request success. Requests still
pass ToolRegistry lookup, PermissionLevel gates, host policy,
repository/workspace constraints, generation/preconditions, and one-shot
reviewed-commit authorization where applicable. Execute does not grant
repository write authority.

The backend sanitizes before frontend rendering. Public Tool names are shown;
private `rah_tool_N` aliases are not. Currentness is generation-aware, and
stale/reconnect-required inventory is not represented as Current. Refresh
Authority is read-only and has no reconnect, Tool execution, chat,
repository, lifecycle, or authority side effects. No live JSONL evidence was
present; the exact live wording is: “No chat prompt was sent and no Tool
activity was visually observed; repository postconditions remained unchanged.”

The live certified Desktop inventory contained 13 public Tools, including
`echo` and the first-party repository/read Tools. `GitStageTool` and
`GitUnstageTool` remain host-owned Repository UI actions, not model-visible
ToolRegistry entries. Reviewed commit may be `Authorization revoked` while
`repo.commit` is effective/advertised; these are separate states.

## 5. Explicit non-goals and scope limits

No new repository mutation authority, dynamic permission granting, authority
toggles, profile hot reload, provider lifecycle management, network MCP,
authority persistence, generic shell/process/filesystem/Git/branch/ref
authority, OS sandboxing, network isolation, rollback guarantee, or external-
provider live-certified review UX is claimed. MCP and Process Plugin remain
under existing Tool-provider architecture but are not currently reachable in
this Desktop composition path.

## 6. Platform and live evidence

- Windows Effective Authority live certification: PASS.
- Linux live certification: not established.
- Certified baseline: `codex-cli 0.149.0`.
- `codex.exe` SHA-256: `14b7e6b2356e82d1d9275579eaa588757b4e0a501b65dcc19fccdf77bd83dc00`.
- `codex-code-mode-host.exe` SHA-256: `3c6726ab12b8de7c0bccecf4551af686d9dbe1b9fcdaee90bd66f60837943ac2`.

## 7. Workspace and dependency gate

- 12 workspace packages.
- All package versions: `0.16.0`.
- Rust edition: `2024`.
- No dependency additions, removals, source changes, or version drift.
- Accepted ADRs remain 0001–0019; no new ADR was required.
- Cargo.lock changes are limited to normal RAH workspace package versions.

## 8. Required validation

Task 198 must record PASS for the full release suite: Cargo format/check/test/
clippy, `git diff --check`, Cargo metadata audit, frontend Node checks and
`status_authority_test.js`, and `cargo build -p rah-desktop --release`.

The eventual release tag must point exactly to the Task 198 release-preparation
commit and may be created only by Task 199 after exact-head CI PASS.
