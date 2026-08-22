# RAH v0.4 scope and authority roadmap

Status: **PLANNED — research recommendation only**
Date: 2026-08-22
Baseline: RAH v0.3.0, Rust edition 2024, `codex-cli 0.148.0`

This document recommends v0.4 scope. It does not grant authority, change a
public contract, revise an accepted ADR, or claim an implementation exists.
Status words are deliberate: **VERIFIED** means supported by the newest recorded
deterministic and, where named, opt-in live evidence; **IMPLEMENTED** means
present in the released source baseline; **PLANNED** is a proposed v0.4 action;
and **DEFERRED** is expressly outside v0.4.

## 1. Current v0.3 boundary

**VERIFIED / IMPLEMENTED.** RAH v0.3 is a provider-neutral harness, not an
inference engine. Public boundaries remain RAH-owned: `AgentRuntime`,
`ModelBackend`, `Tool`, `ToolRegistry`, `AgentEvent`, `SessionStore`, and
`Sandbox`; `rah-protocol` remains dependency-bottom. Codex is an optional,
version-pinned adapter, and Codex wire types remain private to
`rah-runtime-codex`.

**VERIFIED / IMPLEMENTED.** The enabled product path is:

```text
Model output -> parsed ToolCall -> ToolRegistry -> host permission/policy
             -> sandbox/executor -> Tool -> ToolOutput
```

The verified v0.3 components are the Generic Tool Bridge, `fs.read`, MCP
adapter, Process Plugin adapter, hardened `HostExecutionPolicy`,
`RepositoryMutationPolicy`, `host.cargo.version`, `host.git.status`,
`host.git.stage`, and `host.git.unstage`. `process.test.echo` and the
repository-mutation fixture are validation infrastructure, not public host
capabilities; `host.fixture.echo` does not exist.

**VERIFIED / IMPLEMENTED.** Repository mutation ends at the index:

```text
host.git.stage:   worktree -> index
host.git.unstage: HEAD     -> index
```

Neither command writes worktree bytes, creates objects, moves refs, or uses
network Git. Host-owned configuration selects the executable, repository,
symbolic target, argv, cwd, environment, and timeout. Model input supplies none
of those values.

**VERIFIED.** Normal validation is deterministic and offline. Opt-in live Codex
examples require explicitly configured local prerequisites and the exact
`codex-cli 0.148.0` baseline. A Codex upgrade is a separate compatibility task:
it must revalidate executable version, app-server schema/fixtures, translation,
and live behavior; it is not v0.4 scope.

**DEFERRED.** Generic live-model shell/process authority, arbitrary executable
selection, arbitrary filesystem mutation, worktree restore, commit/ref/history,
network Git and credentials, interactive approvals, a general plugin manager,
automatic plugin restart, SQLite persistence, TUI/web UI, RAG, and multi-agent
orchestration remain excluded.

## 2. v0.4 decision criteria and goals

**PLANNED.** v0.4 should make the existing capability foundation usable by a
trusted host without granting models more mutation authority. The smallest
coherent outcome is a host-configured, inspectable composition of the already
implemented runtime and capabilities, with fail-closed authorization preserved.

The release must preserve these invariants:

- A model request is never authorization; missing external permission fails
  closed.
- `Tool` and `ToolRegistry` remain the common built-in/MCP/process-plugin
  extension boundary.
- No model-selected executable, argv, cwd, environment, repository, or
  capability registration.
- Capability execution remains direct program-plus-argument execution without
  shell interpretation and with minimized environment.
- Repository targets remain host-owned; `RepositoryMutationPolicy` still gates
  the index-only tools.
- Timeout, cancellation, disconnect, or process loss do not imply rollback;
  uncertain effects are never replayed.

## 3. Candidate directions

### A. Destructive worktree authority: `host.git.restore-worktree`

**PLANNED candidate; DEFERRED from v0.4.** This would replace one host-owned
tracked file's worktree bytes from a fixed Git source. It is a new destructive
authority class, not a variant of stage/unstage or ordinary Execute.

The v0.3 research establishes minimum conditions before implementation:

- a separate ADR 0011; host-owned, single-use destructive authorization; and
  stale-state refusal immediately before spawn;
- durable, verified raw-byte preimage backup outside the repository; bounded
  retention; a host-only audit; no model-visible backup surface;
- explicit compare-before-recover authorization, never automatic rollback;
- known-success/known-failure/partial-or-policy-violation/uncertain result
  semantics; no retry or replay after spawn;
- one tracked, existing regular file from `HEAD`, with literal pathspecs and
  host-owned source/target only;
- initial refusal of staged state, deletion/creation, mode changes, gitlinks,
  submodules, sparse checkout, linked worktrees, non-regular files, hard links,
  symlinks/reparse points, nested repositories, and unsupported aliases; and
- rejection (or a separately designed hermetic materialization strategy) for
  attributes/configuration that can transform bytes or invoke filters:
  `text`, `eol`, `ident`, `working-tree-encoding`, and filter paths.

Windows makes this disproportionately costly: reparse points, junctions, UNC
and verbatim paths, drive/case aliases, ADS, hard-link count reliability,
sharing violations, read-only/ACL interactions, antivirus/indexer races, and
post-spawn Job Object assignment all need live evidence. Unix likewise needs
symlink, hard-link, permissions, process-group, filter, and concurrent-writer
coverage. Neither platform can turn an in-process repository lease into
cross-process exclusion.

This capability has real user value when a trusted human explicitly asks to
discard one known edit. But the host must supply consent, recovery durability,
and a supported filesystem/Git subset before the model gets any such request
path. It would consume most of a release on a narrow recovery problem while
still excluding common repositories. **Recommendation: DEFER. Do not draft ADR
0011 in v0.4 planning.**

### B. History/ref authority: a future commit capability

**PLANNED candidate; DEFERRED beyond v0.4.** A commit is not worktree authority
and must not be bundled with it. It creates Git objects; can update index state;
moves `HEAD` and refs; writes reflogs; selects author/committer identity and
timestamps; and can involve templates, editors, signing keys/agents, hooks,
ambient configuration, and concurrent writers. Recovery is history-aware, not
a byte-backup problem: a retry may create a different object/parent or duplicate
a recorded effect, while ref movement can race another Git client.

An eventual design needs a separate history/ref authority model and ADR before
implementation. It must make host choices for repository, ref, parent,
identity, message provenance, signing policy, templates/editor behavior,
hooks, configuration isolation, locking, audit/reflog evidence, and
postcondition/recovery semantics. It must reject or consciously govern local
`core.hooksPath`, all commit hooks, signing helpers, credential helpers, and
all network remote operations. No public RAH type currently needs to leak Git
types, but the host-facing authorization contract would be substantial.

Deterministic temporary repositories can prove selected local scenarios, but
live validation must cover real hook/signing/editor suppression and concurrent
mutation on Windows and Unix. Windows adds credential-manager, GPG/SSH-agent,
file-lock, and executable association concerns; Unix adds hook executable-bit,
signal/process-group, and signing-agent behavior. The complexity and residual
ambient authority exceed v0.4's value. **Recommendation: DEFER.**

### C. Security/process/repository hardening

**PLANNED candidate; partly a v0.4 supporting workstream.** This direction adds
little new user-facing authority and can reduce known risk, but only concrete
gaps should enter the release.

Concrete gaps worth scoping behind the selected theme are: host configuration
validation before registration; canonical executable/repository identity
revalidation at use; capability-specific Git configuration isolation; durable,
redacted audit records for currently authorized mutations; and clearer
timeout/cancel/disconnect/process-loss outcome handling. Cross-process
repository coordination is only a best-effort advisory mechanism unless an OS
primitive plus its failure semantics is proven; it must not be advertised as
exclusive locking. Stronger executable identity and TOCTOU resistance similarly
reduce, but cannot eliminate, the final check-to-spawn race.

**DEFERRED.** A general Windows containment subsystem, a claim of OS sandboxing,
or broad Unix live validation is not a small hardening patch. Windows process
containment needs job assignment, breakaway/descendant behavior, handle rights,
and post-spawn-race evidence. Unix needs process-group/session and signal
behavior validated on actual supported hosts. Those efforts are prerequisites
for stronger guarantees, not proof that existing policy is a sandbox.

### D. Higher-level runtime/product capability

**PLANNED candidate; RECOMMENDED for v0.4.** Deliver a *trusted host capability
profile*: a configuration-loaded, validated, inspectable composition of current
RAH capabilities. A profile permits a host to select the runtime mode and
register named, predeclared capabilities such as `fs.read`, `host.cargo.version`,
`host.git.status`, `host.git.stage`, and `host.git.unstage`, subject to all
existing constructors and policies. It gives users a usable application boundary
without requiring application code to hand-register every tool.

The profile is host input, not model input and not a plugin package. Its schema
must be strict, versioned, and default-deny: unknown fields/capability kinds,
duplicate names, missing permission assignments, invalid paths, relative or
uncanonical executables/repositories, and unsupported platform fields fail
before a runtime starts. It must not contain arbitrary command templates,
unbounded environment inheritance, model-provided registration, remote plugin
installation, or secrets in tool output. Sensitive configuration diagnostics
remain host-side and redacted.

The initial product surface can remain a CLI evolution: validate a profile,
print a redacted capability inventory, and run the already-supported restricted
or generic-bridge runtime only when the trusted host explicitly chooses it.
Live provider selection, credential storage, session persistence, desktop-host
integration, automatic plugin lifecycle, and approval UX are separate product
areas. They should not be bundled merely because a configuration file exists.

This has high user value, no increase in model authority, no new provider SDK
dependency, and deterministic fixture testability. It uses existing RAH-owned
types and constructors; no Codex types cross the adapter boundary. Windows and
Unix differences are contained to profile-path validation and capability
availability, both of which can fail closed with platform-specific diagnostics.

## 4. Authority-delta comparison

| Direction | User/product usefulness | New authority | Contract/architecture effect | Test/live feasibility | v0.4 decision |
| --- | --- | --- | --- | --- | --- |
| A Worktree restore | Moderate, but only for an explicit discard workflow | Destructive replacement/removal of worktree bytes and recovery data handling | Private destructive policy plus host consent/recovery; ADR 0011 | Deterministic subset possible; broad Windows/Unix live matrix required | **DEFERRED** |
| B Commit/ref | Potentially high, but workflow-dependent | Object creation, index effects, ref/HEAD/reflog movement, identity/signing/hooks | New history/ref authority model; separate ADR | Fixtures are incomplete evidence; signing/hooks/concurrency need live tests | **DEFERRED** |
| C Hardening | Indirect but important | Ideally none; may strengthen existing host controls | Keep private; avoid calling advisory controls isolation | Deterministic gaps and targeted live tests feasible | **PLANNED support only** |
| D Trusted capability profile | High: current foundation becomes host-operable | None to the model; host config selects only already-authorized capabilities | Host-facing config/composition layer, not a stable core-type redesign | Strong deterministic coverage; opt-in pinned-Codex smoke only | **RECOMMENDED** |

## 5. Detailed required comparison

| Criterion | A Worktree restore | B Commit/ref | C Hardening | D Trusted capability profile |
| --- | --- | --- | --- | --- |
| Security/technical debt | High: destructive recovery and residual races | Very high: ambient Git identity/hook/signing/ref debt | Low only when fixing named gaps; high if it promises containment | Low: strict parsing and redaction are the main new attack surface |
| Complexity/prerequisites | High: ADR 0011, durable recovery, filesystem/Git subset, live matrix | Very high: separate authority design, hooks/signing/config/ref recovery | Medium: exact gap inventory and platform evidence | Medium: configuration design, validation, inventory, fixtures |
| Windows implications | Reparse/UNC/ADS/hard links/sharing/ACL/AV | locks, hooks, editor/signing/credential helpers | Job Object and identity semantics remain best effort | canonical paths; deny unsupported fields and unavailable capabilities |
| Unix/cross-platform implications | links/modes/filters/process groups/concurrency | hooks, identities, agents, file modes, concurrent refs | process groups and live signal behavior | canonical paths, executable availability, uniform strict schema |
| Explicit non-goals | deletion, sparse/submodules, filters, rollback/retry | push/fetch, merge/rebase, arbitrary messages/hooks/signing | OS sandbox, universal cross-process lock | plugin manager, remote installs, secrets, provider routing, persistence, approvals |

## 6. Recommended v0.4 theme and milestone boundary

**PLANNED recommendation: “Trusted host composition, with no new model
authority.”**

The proposed v0.4 boundary is:

```text
trusted host profile
  -> strict validation and canonicalization
  -> existing capability constructors and permission policy
  -> ToolRegistry inventory
  -> existing restricted/generic runtime selection
  -> existing Tool execution boundaries
```

It may add redacted validation and audit status for configured capabilities, but
may not bypass constructors, permission checks, `HostExecutionPolicy`, or
`RepositoryMutationPolicy`. The profile can enable only an explicit allowlist of
the v0.3 capabilities; capability instances remain host-constructed and
immutable after startup. The model cannot alter, reload, add, or select profile
contents during a run.

**PLANNED acceptance shape.** A deterministic suite should construct valid and
invalid profiles in temporary directories, prove default denial and exact
inventory, prove that each capability retains its existing fixed authority, and
prove sensitive values are absent from diagnostics/events. Existing stage and
unstage tests must still prove no worktree/ref mutation. An opt-in live Codex
smoke may be retained only for the exact `0.148.0` bridge and explicitly enabled
profile; it cannot become a normal test prerequisite.

## 7. ADR implications

**DEFERRED.** ADR 0011 is required before destructive worktree authority, but
the recommendation does not add that authority. This task therefore does not
draft ADR 0011.

**PLANNED.** No new ADR is required to record this research recommendation.
Before implementation, perform a narrowly scoped configuration-authority design
task. If it finds that host profiles alter a stable public boundary, introduce a
new ADR under the next available number; it must define trust source, schema
versioning, secret handling, reload/lifecycle semantics, and fail-closed
behavior. Do not repurpose ADR 0011 for configuration.

## 8. Explicit v0.4 non-goals

**DEFERRED.** v0.4 does not include `host.git.restore-worktree`, any destructive
worktree write, Git commit/ref/history/object/reflog authority, reset/clean/
checkout/switch/stash, merge/rebase, remote/network Git, credentials, arbitrary
process execution, model-selected process parameters, OS sandbox claims,
interactive model approvals, general plugin lifecycle/installation, automatic
restart, session persistence, desktop integration, provider routing, Codex CLI
upgrade, TUI/web UI, RAG, long-term memory, or multi-agent orchestration.

## 9. Ordered candidate tasks after this research

1. **PLANNED — Task 027: Trusted capability profile design research.** Define
   the minimal host configuration format, trust source, strict parsing rules,
   capability allowlist, redaction, platform behavior, and whether an ADR is
   required. No production code.
2. **PLANNED — Task 028: Profile implementation and deterministic tests.** Only
   after Task 027 accepts a design. Compose current tools through their existing
   constructors; add no new authority or dependencies without explicit review.
3. **PLANNED — Task 029: CLI profile validation/inventory.** Expose redacted
   validation and inventory; retain the deterministic demo and no implicit live
   provider behavior.
4. **PLANNED — Task 030: focused security evidence.** Test profile fail-closed
   behavior, current mutation invariants, and the selected concrete hardening
   gaps; run a limited pinned-Codex live smoke only if locally configured.
5. **DEFERRED — post-v0.4 worktree authority research refresh.** Revisit only
   when a product owner supplies a destructive-consent and recovery requirement;
   then assess ADR 0011 before code.
6. **DEFERRED — post-v0.4 history/ref authority research.** Keep commit as an
   independent authority program after worktree authority is separately decided.

## 10. Immediate next task

**Recommendation: begin Task 027, trusted capability profile design research.**
It supplies a real host-operability gain, exercises the current `ToolRegistry`
extension architecture, and adds no model-visible mutation authority. It is the
smallest coherent increment with deterministic, cross-platform-first validation
and leaves ADR 0011, destructive worktree replacement, and commit/ref authority
properly deferred.

## Evidence used

**VERIFIED source baseline.** `docs/RAH_V0.3_RELEASE_GATE.md`,
`docs/SECURITY.md`, `CHANGELOG.md`, ADRs 0001–0010, and
`docs/RAH_V0.3_WORKTREE_MUTATION_RESEARCH.md` are the source for existing
claims and the destructive-worktree prerequisite list. This roadmap makes no
claim that research-only proposals are implemented.
