# Task 504C — Intermittent-state audit and validation-resume gate

**G1 — RESUME TASK 504 VALIDATION.**

**A — TASK 504 VALIDATION MAY RESUME WITHOUT PRODUCT CORRECTION.**

Starting HEAD: `3ab73e32d33b37acb8027d7ad4b4c7b8e078d077`, matching the
authoritative checkpoint. Task 504 migration remains dirty and uncommitted;
there is no Task 504 commit or CI. This audit does not resume its validation.
The three historical Phase A failures remain evidence with exact cause unknown.
They are not classified as false, fixed, infrastructure failure or migration defect.

## Evidence and scope

Task 504B recorded Matrix A historical command 8/8 PASS, Matrix B default
parallelism 8/8 PASS and Matrix C serial 4/4 PASS. Each Codex library invocation
had 102 passed, 0 failed, 1 ignored. Each historical test passed in all 20 runs.
No natural failure reproduced. These instrumented runs are bounded evidence,
not a proof of permanent stability or an explanation of the historical failures.
See [Task 504B](2026-10-03-task-504b-natural-failure-evidence-capture.md).

Read README, architecture/guardrails, relevant security composition/observation
sections, ADR 0019 and ADR 0033. Audited only fixtures and state reachable from
the three legacy tests. No instrumentation or source correction was added.
No executable inventory, full workspace gate, Desktop gate or production gate ran.

Evidence: `F:/temp/rah-task504c-evidence`. Current hashes of all seven Task 504B
diagnostic-touched files match its pre-instrumentation/restoration manifest.
`source-before.json` and `source-after.json` additionally verify all seven dirty
tracked source files, untracked runtime_composition.rs and diagnostic-touched
files remained byte-identical during this task (15 distinct files).

## Fixture map

All references below are repository-relative. Test entrypoints are in
`crates/rah-runtime-codex/src/bridge_tests.rs` at lines 1126, 1187 and 2003.

`TestDirectory::new` (line 128) creates
`temp_dir()/rah-codex-fs-read-{pid}-{Unix-nanoseconds}-{atomic-sequence}` with
exclusive `create_dir`. `NEXT_TEST_DIR.fetch_add(Relaxed)` guarantees distinct
sequences within the process; Relaxed suffices for uniqueness. PID/time separate
normal process runs. A hypothetical external collision would fail construction,
not silently reuse a directory. No fixed root is reused.

| Test | Repository and contents | Registry/runtime/observer/counter |
| --- | --- | --- |
| repository_search_dispatches_through_the_generic_bridge | Unique base/repository; tracked bridge-search.txt containing BRIDGE_SEARCH_SENTINEL; local .git/config email bridge@example.invalid/name RAH bridge test; committed search fixture | Fresh ToolRegistry with one RepositorySearchTool and its own RepositoryObserver; fresh fake transport and CodexRuntime; local collected events; no execution counter |
| repository_list_dispatches_through_the_generic_bridge | Different unique base/repository; tracked README.md containing BRIDGE_LIST_SENTINEL and src/lib.rs containing source; same local identity strings; committed list fixture | Fresh ToolRegistry with one RepositoryListTool and its own RepositoryObserver; fresh fake transport and CodexRuntime; local collected events; no execution counter |
| host_composed_repo_create_directory_uses_generic_bridge_once | Unique base/create-directory-success; existing parent; anchor.txt committed; trusted-profile.json (task184-create-directory-bridge, empty resources/capabilities); initially absent existing/new-directory; local identity rah-test@example.invalid/RAH create-directory bridge test | First local composition without authority is shut down; second local composition receives explicit root-bound authority; fresh counting registry wraps only that composed Tool; fresh Arc<AtomicUsize>(0), transport and legacy runtime |

Repeated repository labels, profile IDs, filenames, private-thread/private-turn,
rah_tool_0 and JSON request IDs are names inside independent roots, registries
or connections. They are not global lookup keys. No other concurrent test is
given these fixtures' root paths or authority handles.

## Reachable helpers and process state

- bridge_tests: TestDirectory, RepositoryCreateDirectoryFixture::{new,compose},
  git_executable, git, counting_composed_registry/CountingComposedTool,
  connected_bridge, start_bridge, sample_request, bridge_call/tool_request,
  dynamic_item, finish_turn, response_json and tool_event_count. Git discovery
  runs where.exe git.exe (which on Unix) and canonicalizes the first result on
  each call; no static discovery cache. Fixture Git uses fixed argv and explicit
  child cwd, with inherited environment. Git config writes are repository-local.
- test_support::fake_transport creates separate channel pairs and stopped atomic
  per call. Runtime construction creates separate AppServerConnection channels,
  event broadcast, router ownership, session map, bridge call/dedupe map and tasks.
  No shared process-wide runtime or observer/event registration exists here.
- rah-cli reexports rah-profile-composition. Its compose functions build fresh
  ToolRegistry objects; the empty directory profile launches no external provider.
  TrustedStaticProfile loading and explicit directory authority are instance-owned.
- RepositorySearchTool/RepositoryListTool own RepositoryObserver instances;
  RepositoryIdentity/RepositoryGitLayout and nested boundaries bind their roots.
  Observation uses fixed cwd, bounded child policies and fresh local result maps.
  No global repository membership registry or mutable process Tool registry is used.
- RepositoryObserver and directory authority use git_stage::repository_lease
  (git_stage.rs:625): OnceLock initializes a Mutex-protected root-keyed map of
  Weak async mutex handles. Windows keys lowercase the canonical path. Different
  unique roots have different keys; same-root policies intentionally serialize.
  Dead weak entries cannot restore fixture contents or a prior live lease.
  The separate repository_mutation.rs lease table is not the table used here.
- Tool Git environment is constructed as a local map: GIT_CONFIG_NOSYSTEM=1,
  GIT_CONFIG_GLOBAL=NUL on Windows, GIT_CONFIG_COUNT=3; config entries disable
  core.fsmonitor/core.untrackedCache and admit only the selected safe.directory;
  GIT_OPTIONAL_LOCKS=0 and GIT_TERMINAL_PROMPT=0. Supervised children clear their
  environment and receive this map. No parent-process environment is mutated.
- temp_dir and Git discovery/setup read ambient temp/PATH/Git environment and
  setup inherits user/system Git configuration. These are external inputs, not
  demonstrated inter-test writers. Searches across Codex/runtime source found
  no set_var/remove_var/set_current_dir mutation. No reachable helper changes
  process cwd, installs global tracing, or depends on prior initialization.

## Parallel/serial resource classification

| Resource | Class | Isolation/synchronization finding |
| --- | --- | --- |
| Fixture bases, repositories, .git/index/HEAD/config, files, parent/target, profile | P0 | Unique base owns every path; same relative names do not alias |
| NEXT_TEST_DIR | P1 | Shared atomic allocator; no sequence reset or observable fixture dependency |
| git_stage LEASES OnceLock/map | P1 | One-time initialization plus explicit mutex; root-keyed async leases; no authority or membership registration |
| Registries, profile/authority objects, repository identities/layouts/boundaries | P0 | Fresh and scoped to fixture; no process-wide registry mutation |
| Runtime/connection/session/call maps, channels, observer result maps/events | P0 | Per runtime or invocation; locks coordinate only instance-local work |
| Directory execution counter and fake stopped flag | P0 | Newly allocated atomic per test/connection, never shared across tests |
| Child cwd/environment maps | P0 | Explicit fixture cwd and local policy maps; no parent cwd/env writes |
| Shared Git executable, ambient temp/PATH/config, OS scheduling | P0 for test-owned mutable state | Shared external read inputs; no reachable test mutates them; no P2/P3 defect demonstrated |
| Drop cleanup and native parent handles | P0 | Owned unique base; native handles are invocation-owned; normal runtime shutdown precedes cleanup |

No P2 shared potentially interfering resource or P3 cleanup/order-sensitive
precondition defect is proven. Shared external resources and finite observer
deadlines could affect execution under load, but no measured failure establishes
that explanation. They are not assigned as the cause of these historical failures.

## Search/list interference audit

Their repositories are distinct even though both use the label repository.
The setup writes finish before Tool construction/dispatch; the tests subsequently
observe rather than mutate those contents. Another test's write/delete/Drop has
no path to either unique root. There is no global repository registration,
ToolRegistry mutation or shared observer registration. Their lease map shares
synchronization infrastructure, not fixture contents. Thus co-failure supplies
no independent shared-root evidence.

## Directory preconditions before native creation

`repository_create_directory.rs` policy construction captures canonical root,
root FileIdentity, canonical Git path and executable FileIdentity,
RepositoryGitLayout, nested boundary and root lease. Capture and recapture run
under that lease:

| Input/predicate | Exact source behavior | Concurrent-test reachability |
| --- | --- | --- |
| Target | Parse single logical path existing/new-directory; join bound root; UTF-8 final name; target metadata must be absent | P0, unique root; no other test receives it |
| Parent | Validate existing ordinary in-root directory, reject reparse ancestry/nested boundary; capture parent FileIdentity; open NativeParent relative to root | P0, existing belongs only to fixture |
| Repository/executable | repository_ok checks reparse ancestry, root canonical equality, root/Git same-object identities and layout.validate_git | Root/layout P0; executable shared read input with no test writer |
| Captured Git state | Exact optional index bytes and HEAD bytes from fixture layout | P0, no concurrent test operates on this .git |
| Immediate revalidation | Repeat capture; require identical target path, same parent object and equal GitSnapshot, including absence and all repository checks again | P0, no other test can change these inputs through its own fixture |
| Native input | Original owned NativeParent and name passed once to native create after revalidation | P0, no shared parent handle |

No extra runtime/repository-generation token is consulted by this legacy Tool
path; the captured policy identities and pre-state are its actual inputs.
Production rah-tools is a dependency of this test binary, so its cfg(test)
create-before-native/force-uncertain hooks are not active. There is no test-global
hook capable of injecting another test's target creation.

## Cleanup, ordering and migration reachability

Normal success awaits runtime.shutdown; directory additionally shuts down both
compositions. TestDirectory Drop ignores remove_dir_all errors but deletes only
its own base. A panic may leave incomplete shutdown or a failed deletion; it
does not make the next fixture reuse that root. Leftover owned directories are
not proven P3 interference. No prior initialization, prior cleanup, execution
order or one-time process state precondition was identified beyond synchronized
sequence allocation/lease-map initialization.

**Migration-reachable shared-state defect: NO.** Task 504 WIP in runtime.rs
only changes verify_effective_workspace_context visibility to pub(crate), not
its behavior. Experimental Codex changes add instance-owned workspace/provider
behavior; experimental tests create their own factory/transport objects.
Neutral runtime files change module documentation only. Desktop production
composition/main/tests are outside this library test process. There are no
Cargo manifest/lock/feature changes. Legacy paths do not construct CodexFactory,
experimental host ports or Desktop state. Common bridge/protocol/connection
helpers are unchanged and instance-scoped. Added tests can change suite load
and scheduling, but no migration change alters shared state reachable by these
legacy tests. No static fixture construction or initialization-order change
was found. This is a source reachability conclusion, not a claim about the
unknown historical errors.

## Bounded order-focused executions

Both CARGO_TARGET_DIR and RAH_TEST_TARGET_DIR were
`F:/temp/rah-task504-target`. No source edits occurred while commands ran.
Full logs are in the evidence directory; every exit code was 0.

1. `cargo test -p rah-runtime-codex --lib dispatches_through_the_generic_bridge -- --test-threads=2`
   — 2 passed, 101 filtered; subset-parallel.log.
2. `cargo test -p rah-runtime-codex --lib -- host_composed_repo_create_directory_uses_generic_bridge_once repository_search_dispatches_through_the_generic_bridge repository_list_dispatches_through_the_generic_bridge --test-threads=3`
   — 3 passed, 100 filtered; three-parallel.log.
3. `cargo test -p rah-runtime-codex --lib -- repository_list_dispatches_through_the_generic_bridge repository_search_dispatches_through_the_generic_bridge host_composed_repo_create_directory_uses_generic_bridge_once --test-threads=1`
   — 3 passed, 100 filtered; three-serial-reversed-filters.log.

Reversed filter arguments are selection arguments, not an execution-order
control. Serial execution was directory, list, search in harness name order.
No reverse execution-order claim is made. These checks establish that the
subset works without other suite tests initializing it; no order dependency
was observed. They do not replace Task 504 full validation or repeat its 20-run matrix.

## Decision, authority and next task

G1 criteria are satisfied: no proven P2/P3 blocker; Task 504B 20-run matrix
passed; restoration/current source integrity verified; no architecture or
authority contradiction established. No correction is justified by this audit.
ADR 0033 unchanged. No dependency edge, Tool, permission, authority or bypass
changed. Static HostInvocationKind in host_invocation.rs:29 has exactly 11:
FsRead, RepoFileInfo, RepoStatus, RepoDiff, RepoDiffStaged, RepoCreateBranch,
RepoPatch, RepoEditFiles, RepoCreateFile, RepoDeleteFile, RepoRenameFile.
**HostExplicit = exactly 11.** No executable inventory was needed.

Recommended next task: **Task 504D — Resume Task 504 deterministic validation
from Phase A on frozen current migration source, without product correction.**
Start a fresh Phase A → Phase B → Phase C → full workspace gates → Desktop
canonical gate → static/frontend/inventory → production validation sequence.
Do not reuse the historical failed Phase A as the current gate, and make no
source edits while a validation command is running. This recommendation grants
no automatic execution in Task 504C.

If these failures recur, stop immediately and use Task 504B's prepared strategy
to capture the typed underlying Tool error, exact directory predicate/stage and
relevant fixture/shared-state snapshot. Do not begin another blind matrix.

## Closure

Only this report and a reference-only append to Task 504B were changed.
Product source hashes are unchanged; historical evidence and Task 504 WIP remain.
git diff --check passed; final status and diff stat were inspected. HEAD remains
the starting checkpoint. Seven tracked product files remain modified; product
runtime_composition.rs, ADR 0033 and Task 504/504A/504B/504C plans remain untracked.
No staging, commit or push occurred; all work remains uncommitted. The recommended
docs commit is optional and was left for later to preserve the existing dirty
checkpoint. No CI or remote state changed. Remaining risk: unexplained historical
failures can recur; the next task must enforce the immediate-stop diagnostic rule.
