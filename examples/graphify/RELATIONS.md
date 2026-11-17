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
