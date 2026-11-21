# Relationship wording (graphifyy 0.9.107)

These mappings describe extractor endpoint conventions, not proven program
behavior. EXTRACTED and INFERRED remain independent provenance classifications.
Unknown relations keep their spelling and stored order, with meaning unverified.

The available RAH snapshot contains `calls`, `case_of`, `cites`, `contains`,
`crate_depends_on`, `defines`, `depends_on`, `implements`, `imports`,
`imports_from`, `indirect_call`, `inherits`, `method`, `rationale_for` and
`references`. Only the table below has verified endpoint wording in this change;
the rest use the unverified fallback. Other extractor paths may emit additional
names; the mapping is deliberately not exhaustive.

| Raw relation | Source role | Target role | Sentence | Evidence in pinned package |
|---|---|---|---|---|
| calls | caller | callee | A calls B | extractors/engine.py; Python/Java fixtures |
| imports | importing file | imported module | A imports B | extract.py Python import handling; Python fixture |
| contains | containing file | definition | A contains B | extractors/engine.py; Python/Java fixtures |
| method | owning type | method | A has method B | extractors/engine.py; Python/Java fixtures |
| inherits | derived type | base type | A inherits from B | extractors/engine.py; Python/Java fixtures |
| implements | implementing type | interface | A implements B | extractors/engine.py; Java fixture |
| depends_on | package | dependency | A depends on B | manifest_ingest.py extract_package_manifest; Cargo fixture |
| crate_depends_on | dependent crate | dependency crate | A depends on B | cargo_introspect.py; Cargo workspace fixture |

`semantically_similar_to` describes symmetric conceptual similarity (llm.py's
relation vocabulary and analyze.py's semantic-similarity scoring).
It has no arrowhead; stored endpoints are retained for identity only. Other names,
including `uses`, `references` and `imports_from`, remain unverified here.

Run the offline source-backed checks with the pinned Python environment:
`python -B examples/graphify/relations.test.py`. Normal Node tests need neither
Python nor Graphify. The Python checks require the already installed pinned
package and never download it. They check actual emission and endpoint labels.

The pinned export.py restores `_src`/`_tgt` to `source`/`target` before writing
graph.json; undirected NetworkX storage does not authorize reversing displayed
edges. Upstream may collapse reverse relationships before export; this view
cannot reconstruct missing edges.
# Python connection evidence

Managed extraction emits a `connection_site` edge from the enclosing symbol
or file to each Python `.connect()` call. Each site has its own node and
workspace-relative source location. The column is a one-based UTF-8 byte
column. The version 1 `rah_connection` node field contains the enclosing
symbol label, the signal and callback expressions when present, and the full
call expression with Graphify's credential redaction. Qt imports and recognizable receiver constructor bindings
identify Qt candidates. Other calls remain general event candidates. An edge
records a call site. An inferred `callback` edge links the site to a uniquely
identified method. The extractor handles direct `self` references, simple
same-file inheritance, and a single constructor assignment through an
unambiguous absolute import rooted in the indexed workspace. Other callbacks
retain the source expression and an unresolved status.

Search the relationship view by callback expression to find the site nodes.
Select a site to copy its source location and view the call text within the
view's 512-character detail limit. Older artifacts remain readable and gain
site evidence on their next successful managed rebuild.

For supported Python member accesses, `member_access_site` links the
enclosing method to an individual read or write site. A `reads` or `writes`
edge then links that site to a property definition or to a field first
declared through a unique direct assignment in a class initializer. This
records the member name even when its value can change. Indexed receivers
and calls through multiply assigned instance attributes retain unresolved
call-site evidence. These rules cover a subset of Python's runtime binding.
