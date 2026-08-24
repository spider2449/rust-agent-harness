# RAH v0.7 Multi-Replacement `repo.patch` Research

Status: Task 071 research/design only

Baseline: Task 070 `f29d891039e81b6e70fd8a824197f027e8c65795`; RAH v0.6.0;
`codex-cli 0.149.0`.

## Decisions

**Amend ADR 0012 narrowly and extend `repo.patch` in place.** Multiple exact
replacements are expanded operation expressiveness, not a new authority class,
only because the operation remains one host-selected repository, one existing
clean HEAD-tracked regular UTF-8 file, one repository lease, one complete
preimage, and one final native replacement. It grants no namespace, mode,
index, history, process, generic filesystem, or multi-file authority.

Use two mutually exclusive closed request forms during v0.7: retain legacy
`expected_old_text`/`replacement_text` for backward compatibility and add a
new `replacements` form. Normalize the legacy form privately to one item.
Reject mixed forms, partial legacy fields, neither form, and unknown fields.
This avoids needless `repo.patch-multiple` profile/bridge churn, while a
version selector would add model freedom without clarifying behavior. The
existing Generic Tool Bridge passes the Tool JSON schema generically; Task 072
adds its changed-schema/dispatch tests but needs no production bridge change.

## Exact JSON schema

The advertised root is a `oneOf`; each branch is closed with
`additionalProperties: false`:

```json
{
  "oneOf": [
    {
      "type": "object",
      "properties": {
        "path": {"type": "string", "minLength": 1, "maxLength": 1024},
        "expected_file_sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
        "expected_file_byte_length": {"type": "integer", "minimum": 0, "maximum": 1048576},
        "expected_old_text": {"type": "string", "minLength": 1, "maxLength": 65536},
        "replacement_text": {"type": "string", "maxLength": 65536}
      },
      "required": ["path", "expected_file_sha256", "expected_file_byte_length", "expected_old_text", "replacement_text"],
      "additionalProperties": false
    },
    {
      "type": "object",
      "properties": {
        "path": {"type": "string", "minLength": 1, "maxLength": 1024},
        "expected_file_sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
        "expected_file_byte_length": {"type": "integer", "minimum": 0, "maximum": 1048576},
        "replacements": {
          "type": "array", "minItems": 1, "maxItems": 16,
          "items": {
            "type": "object",
            "properties": {
              "expected_old_text": {"type": "string", "minLength": 1, "maxLength": 65536},
              "replacement_text": {"type": "string", "maxLength": 65536}
            },
            "required": ["expected_old_text", "replacement_text"],
            "additionalProperties": false
          }
        }
      },
      "required": ["path", "expected_file_sha256", "expected_file_byte_length", "replacements"],
      "additionalProperties": false
    }
  ]
}
```

No item ID, occurrence index, per-item digest, or flexible occurrence count is
permitted. Every item has the implicit invariant `expected_occurrences = 1`.
The full-file SHA-256 and byte length already bind every item to the same
snapshot; item preconditions add no protection.

## Matching and construction

All matching is against the **original snapshot**, never sequentially mutated
content. After the existing strict UTF-8/NUL/preimage validation, exclude one
leading UTF-8 BOM from matchable text and preserve its bytes. For each item,
find its exact old text once in the original decoded body, then convert that
match to an original UTF-8 byte range.

- Empty old text, zero/multiple occurrences, old equal to new (verified no-op),
  duplicate old texts, duplicate ranges, and overlaps are refusals before any
  target write. Adjacent ranges are allowed.
- Sort ranges by increasing original start byte and construct the complete
  final body in one pass: untouched original bytes, replacement bytes, then
  the next untouched bytes. Reattach the preserved BOM.
- New text never becomes later matching input. Thus `A -> X` plus `X -> Y`
  fails for original `A B`; new text containing another old text has no extra
  effect. Nested old texts ordinarily overlap and therefore fail.

This avoids order-dependent discovery, arbitrary offsets, fuzzy/regex/line
editing, and partial semantic success.

## Fixed bounds and byte rules

| Limit | Value |
| --- | ---: |
| Serialized request | 64 KiB UTF-8 bytes |
| Path | 1,024 UTF-8 bytes |
| Replacement count | 16 |
| Each old/new text | 64 KiB UTF-8 bytes |
| Aggregate old/new text | 64 KiB UTF-8 bytes |
| Input file | 1 MiB raw bytes |
| Final output | 1 MiB raw bytes |

These retain current limits and add an aggregate list budget compatible with
the existing 64 KiB serialized-input ceiling. JSON `maxLength` is only an
admission hint; Rust byte checks are authoritative. Request text rejects NUL
and U+FEFF. Matching is exact UTF-8 byte/string equality: no Unicode
normalization, case folding, or encoding/newline conversion. LF, CRLF, and
mixed endings remain exact bytes; unchanged bytes are preserved. A leading
BOM is retained and request strings cannot add or remove it.

## Repository, temp, commit, and race contract

All current eligibility remains: non-bare canonical root, safe relative path,
existing regular non-link/non-reparse target, HEAD tracking, exactly one clean
stage-0 entry equal to HEAD, and rejection of unsupported index flags,
submodules, nested repositories, hard links where detectable, and unsupported
Windows attributes. Retain one exclusive RAH repository lease for the whole
operation; it does not exclude external actors.

The Task 072 algorithm is:

```text
lease -> capture/revalidate repository and target -> read/validate preimage
-> locate original ranges -> construct bounded final bytes in memory
-> exclusive host-named same-directory temp -> write/flush/close/revalidate temp
-> revalidate repository/target/Git/preimage immediately before commit
-> one native replace -> post-observe identity, repository, hash, and length
```

The native replacement call is the sole externally visible content commit
point. Immediately before it, reread and require exact captured-preimage bytes
as well as unchanged target identity and Git state. This detects an external
change before commit. An external change after commit that makes post-checks
incomplete or contradictory is `uncertain`, never rollback.

Temps remain unpredictable host-named UUIDs in the validated target directory,
exclusive, regular, non-link/non-reparse, bounded, flushed, closed,
identity/content revalidated, and never model-visible. Clean them before
commit or after failed replacement only when ownership and contents are proven.
Cleanup ambiguity is uncertain. A crash before commit can leave a temp; v0.7
does not add startup cleanup because guessing ownership would itself be
destructive authority.

Success requires the target to have the expected temp identity, the computed
postimage SHA-256 and length, and unchanged required repository observations.
No extra Git status command is needed. Do not promise timestamps, ownership,
ACLs, or power-loss durability. Task 072 must preserve the Git regular-file
mode (`100644`/`100755`): the current Unix rename installs temp-file mode, so
this is a targeted existing implementation gap. Windows normal attributes are
supported only where current validation proves them; read-only, compressed,
and encrypted inputs remain refused. No chmod, ownership, ACL, or attribute
mutation is authorized.

## Outcomes, cancellation, timeout, crash

| Result | Required meaning |
| --- | --- |
| Rejected / validation failure | No native replacement attempt; unchanged only when the preimage is proven intact. Retry needs fresh observation. |
| Failed before commit | No target attempt; safely clean temp where possible; unchanged only with preimage proof. |
| Success | One replacement attempt and exact final target post-verification. |
| Uncertain | Commit may have happened or observation/cleanup proof failed. Never retry, replay, restore, or infer state; re-observe. |

Existing public vocabulary is sufficient: keep `precondition_failed`,
`replacement_failed_known`, `ok`, and `uncertain` with redacted reasons.
Parsing errors remain `ToolError::InvalidInput`; no public API expansion is
needed. Cancellation/timeout before commit prevents an attempt only when the
preimage is proven intact. During/after commit, cancellation, timeout,
disconnect, or crash is uncertain unless post-observation proves the existing
public outcome. There is no journal, transaction, auto-recovery, or replay.

## ADR and integration decision

Choose an **ADR 0012 amendment**, not ADR 0013. Repository practice has
already hardened ADR 0012 in place, and this is a narrow extension of the same
single-file worktree authority. Before Task 072, amend its excluded,
initial-scope, preconditions, encoding, mutation, failure, deferred, and
alternative text to replace forbidden `multi-edit` with this exact bounded
one-file contract. Preserve the multi-file prohibition and state explicitly
that no new authority class is granted.

`PermissionLevel::Execute` is unchanged: it is the outer runtime gate while
the private policy/ADR is actual authorization. The trusted-profile schema and
version are unchanged: the existing closed `repo.patch` capability keeps its
name, permission, and symbolic bindings. Generic Tool Bridge production code
is unchanged; later deterministic tests cover exact schema, Execute gating,
dispatch/dedupe, output redaction, and no replay after uncertainty.

## Task 072 scope and test plan

Task 072 implements only this contract and adds a private/test-only seam at:
before temp write, after temp write, before native replace, immediately after
native replace, and before post-verification. It does not change profiles,
bridge production code, or live examples.

Success tests: two independent edits; 16 edits; adjacent ranges; length
changes; CRLF; Unicode; BOM; new text containing another old; legacy and new
forms; Windows normal tracked file; Unix executable mode. Refusals: stale
hash/length; missing/multiple old; overlap; duplicates; empty old; no-op;
count/item/aggregate/output bounds; unknown/mixed forms; untracked/directory/
non-UTF8 target; symlink/reparse; repository identity, Git/index/HEAD, and
external pre-commit races.

Windows additionally covers target and temp/final sharing violations,
reparse/junction aliases, ADS-like colon rejection, case variant identity,
reserved forms, supported long paths, and native replacement uncertainty. Unix
additionally covers executable mode, observed permissions without ownership
claims, symlinks, case-distinct paths, invalid UTF-8 filename admission, and
external rename races.

A later bridge task proves advertised schema, Execute gating, one multi-item
dispatch, dedupe/no replay, and redaction. A later live Codex gate uses the
approved exact baseline and one fixture with three independent edits and an
unrelated sentinel. It requires observe -> exactly one `repo.patch` ->
`repo.diff` -> exact marker, and proves all edits, unchanged sentinel/index,
observer result, terminal completion, and cleanup without extra authority.

## Deferred

Still deferred: create/delete/rename; mode/ACL/ownership mutation; binary or
non-UTF8 edits; multiple files; unified diff; fuzzy/regex/glob/line/range/
occurrence-index edits; generic `fs.write`; Git add/commit/history; rollback,
automatic recovery/replay; profile schema work; bridge features; and live
implementation.
