# RAH v0.10 Desktop Host Configuration & Authority Roadmap

Date: 2026-08-27

## Decision

**Recommended v0.10 product center: Desktop Certified Codex Baseline
Discovery and Selection.**

RAH Desktop should reliably select the already-certified local
`codex-cli 0.149.0` baseline without relying on ambient `PATH` or external
environment setup. The Desktop Rust host, not JavaScript and never the model,
should resolve and re-verify the known baseline before connecting. This removes
the observed Unsupported Codex version failure without adding a model tool,
generic process authority, provider installation, or inference capability.

The bounded follow-on within the same v0.10 product track may be **Desktop
local llama.cpp endpoint configuration**, but only if it remains an explicit,
loopback-only connection setting for an already-running Responses-compatible
server. It must not select an executable or model, start a process, download a
model, or restore a connection on restart. If the baseline work alone consumes
the release budget, endpoint configuration is the recommended Task 118
research/implementation candidate, not a reason to widen Task 117.

**Explicitly defer RAH-owned llama.cpp process launch.** It is a separate
host-runtime-management authority, not merely a different endpoint field.

Task 118 supersedes the earlier port-only loopback proposal: llama.cpp can be
an already-running LAN or explicitly configured remote network service. Strict
loopback-only remains the narrow no-new-authority case, but non-loopback
selection determines where model input can leave the Desktop machine. It is a
distinct bounded model-provider outbound-network authority and implementation
must wait for ADR 0015 — Bounded Model Provider Network Endpoint Authority.
The anticipated form is a structured host-selected configuration (closed
scheme, explicit host, bounded port, fixed `/v1` path), not arbitrary URL text.

## ADR 0015 acceptance and Task 120 authority

**ADR 0015 — Bounded Model Provider Network Endpoint Authority is accepted.**
It grants only a trusted human/Desktop host selection of one structured initial
`llama_cpp` provider endpoint for one explicit connection: closed HTTP/HTTPS
scheme, validated IPv4/IPv6/DNS host, bounded numeric port, and Rust-synthesized
fixed `/v1` path. It is not generic network, ToolRegistry, MCP, Git, browser,
process, credential, redirect, proxy, discovery, or provider-lifecycle
authority.

**Task 120 is authorized only to implement that Desktop-private boundary.** It
must keep credentials `None`, use closed Rust validation and endpoint synthesis,
retain explicit reconnect/model-generation semantics, and add neither endpoint
persistence nor transport-confinement claims.

## Audited starting state

This roadmap starts from clean `master` at
`0bcc985e8c275f4d26814913947f0bf49dc650c2`.

- RAH v0.9 repository-authoring authority is complete; ADRs 0012, 0013, and
  0014 remain scoped to their distinct repository mutations.
- Desktop Task 115B cross-restart Resume is complete. Persisted transcript
  text is display/replay data only after an explicit user action and a fresh
  current host context; it restores no repository, provider, model, tool, or
  native thread authority.
- Actions Cleanup and Windows PowerShell 5.1 compatibility for
  `codex-baseline.ps1` are complete.
- The only certified Windows live Codex baseline is exactly
  `codex-cli 0.149.0`.
- Desktop currently chooses `RAH_CODEX_EXECUTABLE`, otherwise `codex`; the
  latter reaches hardened adapter PATH/npm discovery. Desktop model selection
  has provider/model state but llama.cpp always uses
  `http://127.0.0.1:8080/v1`.
- The baseline store already has a closed manifest and verification workflow:
  `%LOCALAPPDATA%\codex-baselines\0.149.0\codex.exe` plus `manifest.json`.
  Store-path appearance is not evidence: manifest, native executable form,
  platform/architecture, SHA-256, and reported version must be checked.

## Required distinctions

| Concern | Owner | Meaning | Must not imply |
| --- | --- | --- | --- |
| Model authority | Model through RAH tool path | A request the model may make | Host permission or process selection |
| Host configuration | Human/Desktop Rust host | A current desired provider/baseline/endpoint | A model-visible capability |
| Durable configuration | Desktop-private stored preference | A value available after restart | A live connection or current authorization |
| Active authority | Freshly verified host objects for this run | The executable, registry, repository, and provider actually in use | Validity merely because an old value was persisted |
| Process management | Host lifecycle owner | Start, monitor, stop, and classify an external process | OS sandboxing or inference implemented by RAH |

These layers are intentionally non-equivalent. A persisted selected baseline
identity can improve restart UX, but a fresh connection must still resolve and
verify it. A persisted endpoint does not prove a server is present, local,
compatible, or authorized to receive a request. No configuration item is a
Tool, profile grant, or permission decision.

## Candidate decision matrix

Scores: value/testability/live feasibility are 1 (low) to 5 (high); risk and
complexity are 1 (low) to 5 (high). “New host authority” means a material new
active capability, not ordinary host-side validation of an already-approved
choice.

| Candidate | Value | New model authority | New host authority | Risk | Complexity | Deterministic | Windows live | Architecture / dependency / ADR | Persistence | v0.10 |
| --- | ---: | --- | --- | ---: | ---: | ---: | ---: | --- | --- | --- |
| A. Explicit certified baseline selection | 5 | None | No; chooses verified existing adapter input | 1 | 2 | 5 | 5 | Desktop-private; no dependency; no ADR | Persist identity only, re-verify | **Adopt** |
| B. Automatic fixed-version baseline discovery | 5 | None | No, if strict verification precedes use | 1 | 2 | 5 | 5 | Same as A; no ADR | Prefer source/status, not path | **Adopt with A** |
| C. Local llama.cpp endpoint configuration | 4 | None | No process authority; bounded outbound connection target policy is host configuration | 2 | 3 | 4 | 4 | Adapter/Desktop-private; likely no ADR if loopback-only/no credentials | Persist normalized endpoint only; reconnect explicit | Conditional follow-on |
| D. llama.cpp process launcher | 4 | None | **Yes: provider process lifecycle and file identity authority** | 5 | 5 | 2 | 3 | New bounded design, likely ADR; possible Windows APIs/dependency | Paths/identities need new durable policy | Defer |
| E. Persisted host configuration | 4 | None | No, only if restore never activates it | 3 | 3 | 4 | 4 | Desktop-private schema; no ADR for limited preferences | Identity/provider/model/loopback endpoint only | Pair with A/C |
| F. Conversation export/import | 3 | None | No | 2 | 3 | 4 | 4 | Desktop-private; no ADR if completed text only | Export/import remains data, never authority | Defer |
| G. Model-selected Git staging | 3 | Yes: path/index target selection | Yes: index mutation | 4 | 4 | 3 | 3 | Successor/new ADR; no generic Git argv | No authority replay | Defer |
| H. Commit/history/ref authority | 3 | Yes | Yes: durable Git/history mutation | 5 | 5 | 2 | 2 | New ADR(s), complex hooks/signing/refs | Never restore active Git authority | Defer |
| I. Network MCP / Streamable HTTP | 3 | Potential remote tools | Yes: endpoint/auth/network lifecycle | 5 | 5 | 2 | 2 | New ADR; transport/security dependencies likely | Credentials and endpoint policy are sensitive | Defer |
| J. PluginManager/lifecycle | 2 | Potential installed tools | Yes: install/update/start/remove | 5 | 5 | 2 | 2 | New ADR; fixed Process Plugin is insufficient | Provenance/update state required | Defer |
| K. Trusted Profile reload | 2 | Indirectly changes visible tools | Yes: replaces active authority set | 5 | 5 | 2 | 2 | Conflicts with ADR 0011 static lifetime; new ADR | Persisted profile cannot auto-activate | Defer |
| L. Delete/rename/directory creation | 3 | Yes: repository paths | Yes: persistent filesystem mutation | 4-5 | 4-5 | 2-3 | 3 | Separate successor/new ADR(s) | No replay/recovery grant | Defer |

## Codex baseline recommendation

### Answers

1. **Should Desktop keep requiring `RAH_CODEX_EXECUTABLE`?** No. Retain it as
   an explicit host/operator override for compatibility, live gates, and
   diagnosis; do not require it for ordinary Desktop use.
2. **Should Desktop discover the exact supported store baseline?** Yes. It
   should look only for the adapter's exact `SUPPORTED_CODEX_VERSION` in the
   existing baseline store and apply the existing verification rules before
   use.
3. **Should verified baseline selection precede PATH?** Yes.
4. **Should PATH remain a fallback?** Yes, as last-resort compatibility only.
   Adapter version/schema checks still reject an incompatible result; Desktop
   should present a sanitized source/status, not claim certification.
5. **Should JavaScript receive the absolute executable path?** No. Expose a
   closed presentation such as `source: override|certified_baseline|path` and
   `version/status`; keep raw paths Rust-private.

### Safe precedence and activation

```text
explicit RAH_CODEX_EXECUTABLE host override
  -> exact supported baseline-store candidate, freshly verified
  -> existing PATH/npm compatibility discovery
  -> sanitized failure
```

The explicit override preserves a deliberate operator choice; it must still
pass the adapter's native executable and exact-version checks. Store discovery
must use a Rust-side verifier equivalent to `codex-baseline.ps1 verify`, not
trust a directory name, manifest alone, or a matching path. A selected
baseline passes an absolute verified executable to `CodexRuntime`, so PATH is
not consulted for that connection. The store must never be searched for a
newer “best” version and a candidate baseline must never be promoted
automatically.

This is host configuration/presentation plus fresh validation, not a new
model, ToolRegistry, permission, trusted-profile, or generic process boundary.
It reuses an existing explicit adapter executable input. No new ADR is needed
provided the implementation remains Desktop-private and does not turn the
baseline store into generic executable selection or installation authority.

## llama.cpp recommendation

### Endpoint configuration for v0.10

Endpoint-only configuration is sufficient and more valuable than process
ownership for the first local-provider step. The recommended initial contract
is:

- an explicit host-entered `http://127.0.0.1:<port>/v1` endpoint only;
- canonical URL parsing and exact loopback host validation (no hostname,
  LAN, IPv6-any, proxy, redirect, or automatic discovery);
- no credentials, headers, environment-variable names, custom provider URL,
  executable path, model path, GPU/backend flag, context size, server probing,
  process launch, download, or installation;
- endpoint/model changes make the current configuration reconnect-required;
  connecting is explicit and validates current configuration afresh;
- frontend receives a sanitized selected endpoint status, never host file
  paths or secrets.

The existing `127.0.0.1:8080/v1` preset remains a useful default, but not an
adequate configuration capability where local servers move. The configured
endpoint is a connection target, not proof of a llama.cpp binary, model,
server identity, server state, compatibility beyond the existing restricted
Responses contract, or permission to perform arbitrary network access.

For that narrowly local-only configuration, C requires no new model authority
and no process-management authority. It should require no ADR if it is a
private Desktop-to-existing-adapter configuration change with a closed
loopback-only validator and no new network/provider installation policy. If
the scope expands to non-loopback URLs, credentials, redirects, proxy handling,
or provider discovery, stop: that is network authority and needs fresh ADR
research rather than an incremental form field.

### Why process launch is different

RAH does not need to know the location of `llama-server.exe` or a GGUF for
v0.10 endpoint configuration. It would need both only for a future launcher,
where they become host-selected resources requiring identity, replacement, and
lifecycle rules. Starting an external inference-provider process does not make
RAH an inference engine—RAH would not load weights, implement kernels, or run
inference itself—but it still grants a valuable and risky new host authority:
the ability to execute and manage a local provider with selected resources.

A future launcher must be a separately researched bounded
host-runtime-management authority, not `shell.exec` and not a generic profile
executable/argv schema. Its design must answer all of these before coding:

- operator-selected native executable validation, canonical identity, expected
  version/hash policy, and stale/replaced-file refusal;
- operator-selected GGUF regular-file identity, no model-supplied path, clear
  TOCTOU limitations, and a decision whether a path, fingerprint, or neither
  persists;
- closed/allowlisted argv generated by the host, fixed isolated cwd and cleared
  environment, explicit fixed limits for GPU/backend and context arguments;
- bounded stdout/stderr capture, readiness probe, timeout, cancellation,
  disconnect, shutdown, crash taxonomy, restart policy, and no automatic replay
  after uncertain startup/effect;
- Windows Job Object ownership and child cleanup as supervision only, never an
  OS sandbox claim; and
- no automatic provider/model installation, download, update, executable
  replacement, or generic process authority.

Existing process-supervision primitives inform that work, but are not by
themselves sufficient. MCP/Process Plugin launching proves a fixed adapter
process pattern; it does not authorize a model-serving executable plus GGUF,
provider readiness, restart, persistent resource identities, or GPU launch
policy. D therefore needs a new ADR before implementation.

## Persistence recommendation

Persisted configuration may restore UX, never authority. A later Desktop-private
preference record may contain only bounded, schema-validated values such as:

- selected Codex source identity (`certified_baseline` and fixed supported
  version, or an override-selected marker without its raw path);
- model provider kind and bounded model name; and
- normalized loopback llama.cpp endpoint.

On startup this record must create only inactive desired state. It must not
auto-connect, start a provider, choose a repository, recreate a ToolRegistry,
restore permissions/profile authority, apply an environment override, or
resurrect credentials. Connection must freshly resolve/verify the baseline and
current endpoint after explicit user action. An unavailable baseline or server
is a sanitized inactive/error state, not a fallback to an old raw path.

Initially reject persistence of absolute executable paths, GGUF paths,
credentials/tokens/headers, arbitrary provider URLs, process argv/environment,
GPU flags, context size, server state, native thread IDs, repository identity,
registry inventory, or authority snapshots. These either expose sensitive host
topology or would misleadingly imply that an old durable value is present-tense
authority.

Conversation export/import is lower value than fixing a current connection
failure. If later pursued, it should export/import completed visible
conversation data through a deliberate user action and keep the same rule:
imported data restores neither authority nor automatic replay/connection.

## ADR conclusion

- **A/B:** no new ADR when strictly Desktop-private, Rust-owned, exact-version
  discovery/verification feeds the existing explicit `CodexRuntime` executable
  seam, and JavaScript receives only a redacted status.
- **C:** no new ADR for a closed, loopback-only, no-credential, already-running
  endpoint setting that uses the current provider configuration seam and never
  launches a process. Broadened endpoint/network behavior requires ADR research.
- **E:** no ADR for inactive, private preference persistence that restores no
  authority. An ADR is required if durable configuration can automatically
  activate/replace authority.
- **D:** a new ADR is required before any launcher implementation. It changes
  the security model by introducing bounded external inference-provider
  lifecycle authority. ADR 0011 cannot be broadened silently: it expressly
  rejects generic subprocess schemas, installation, and hot reload.
- **G-L:** retain the existing deferrals. In particular, static Trusted Profile
  lifetime in ADR 0011 remains intact, and ADRs 0012-0014 are not authority for
  directory, delete/rename, staging, commit, or ref work.

## Proposed sequence

1. **Task 117 — Desktop Certified Codex Baseline Discovery and Selection.**
   Implement only the A/B closed Rust-host resolver and presentation, with
   deterministic isolated-store/override/PATH precedence tests and a certified
   Windows live connection. No persistence, endpoint form, launcher, or ADR.
2. **Task 118 — Desktop Local Provider Endpoint Configuration Research.**
   Confirm loopback URL normalization, adapter seam, UI closure, persistence
   schema, deterministic test design, and live already-running-server evidence
   before implementation.
3. **Later separate research — Bounded llama.cpp Provider Lifecycle Authority.**
   Produce the required new ADR proposal and lifecycle/failure contract before
   any executable/model selection or launch code.

## Explicit deferrals

- RAH-owned llama.cpp launch/supervision/restart and all executable/GGUF path
  persistence.
- Model and provider installation, model download, automatic endpoint discovery,
  non-loopback/network providers, credentials, arbitrary URLs, and generic
  process or shell authority.
- Model-selected Git staging; commit/history/ref mutation; network MCP;
  PluginManager/install/update/remove; trusted-profile reload; repository
  delete/rename/directory creation; and conversation import/export.

The resulting v0.10 remains provider-neutral at RAH public boundaries:
Desktop uses a Codex-adapter-local baseline resolver and existing neutral model
selection path. It introduces no dependencies, no Cargo changes, no new model
authority, and no new active host authority.
