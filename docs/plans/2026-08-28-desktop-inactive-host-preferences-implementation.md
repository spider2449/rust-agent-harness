# Desktop inactive host preferences implementation

Task 122 implements the closed v1 `desktop-preferences.json` record in the
Desktop local-data directory. It restores only inactive desired model state;
startup does not resolve Codex, construct a runtime or registry, or probe a
provider.

The v1 record is closed and canonical: `inherit`, `openai`, `ollama`,
`lm_studio`, and loopback-IP-only `llama_cpp`. Non-loopback llama.cpp selections
remain valid in-process Task 120 selections but are deliberately not retained.
Writes use an exact private temporary-file family and the existing native
ReplaceFileW / narrow MoveFileExW fallback pattern. Restore and save failures
are exposed through a separate sanitized preferences warning domain.

Explicit Apply changes desired state before attempting persistence. Reset writes
the canonical inherit record, does not clear conversations or disconnect a
runtime, and uses the same chat-idle safety gate.

Deterministic tests cover schema parsing, model validation, restore fallback,
atomic write behavior, non-loopback non-retention, apply ordering, and reset.
Windows live acceptance is recorded after the release Desktop checks complete.

Task 122's startup matrix uses private test-only counters at the direct activation
seams for Codex resolution, runtime construction, llama.cpp readiness probing,
ToolRegistry construction, repository composition, and explicit conversation
Resume. Codex app-server launch is structurally unreachable until runtime
construction; provider requests are structurally unreachable until the explicit
readiness probe; trusted-profile composition/activation, MCP spawn, and Process
Plugin spawn are structurally unreachable because Desktop startup constructs none
of those components and the Desktop registry contains only host-owned built-ins.
Transcript display restoration remains independently permitted, while Resume is
only activated by its explicit command.

Applying a persistable selection structurally identical to the current durable
canonical bytes performs no preference rewrite and never increments the model
generation. Any distinct persistable Apply or Reset performs at most one logical
persistence transaction. A non-loopback llama.cpp selection is rejected before
any preference filesystem mutation, but remains a valid current-process model
selection.

Focused implementation validation currently runs 88 Desktop tests. The Windows
release live-acceptance checklist remains an operator gate and is not claimed by
this document until it is performed against the exact release build.
