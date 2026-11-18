"""Run the pinned Graphify extractor with BOM-compatible Cargo manifest reads."""

import ast
from collections import Counter
from pathlib import Path

from graphify import cargo_introspect, extract as extractor, manifest_ingest


def load_manifest(path: Path) -> dict:
    """Accept the optional UTF-8 BOM without hiding malformed manifest errors."""
    try:
        import tomllib
    except ModuleNotFoundError:
        import tomli as tomllib
    return tomllib.loads(path.read_text(encoding="utf-8-sig"))


# This affects only this supervised child and its Python worker imports.
cargo_introspect._load_toml = load_manifest

_cargo_parser = manifest_ingest._PARSERS["cargo"]


def parse_cargo_manifest(text: str) -> dict | None:
    """Keep package/dependency AST nodes as well as Cargo introspection nodes."""
    return _cargo_parser(text.removeprefix("\ufeff"))


manifest_ingest._PARSERS["cargo"] = parse_cargo_manifest

_extract_python = extractor.extract_python


def self_attribute(node):
    if isinstance(node, ast.Attribute) and isinstance(node.value, ast.Name) and node.value.id == "self":
        return node.attr
    return None


def normalize_constructor_aliases(module, calls):
    """Let Graphify check class ownership and import edges using the declared name."""
    imports = {a.asname: a.name for stmt in module.body if isinstance(stmt, ast.ImportFrom)
               for a in stmt.names if a.asname}
    if not imports:
        return
    bindings = Counter()
    imported_names = Counter()
    for node in ast.walk(module):
        if isinstance(node, ast.alias):
            if node.name == "*":
                return
            bindings[node.asname or node.name.split(".")[0]] += 1
            imported_names[node.name] += 1
        elif isinstance(node, ast.Name) and isinstance(node.ctx, (ast.Store, ast.Del)):
            bindings[node.id] += 1
        elif isinstance(node, ast.arg):
            bindings[node.arg] += 1
        elif isinstance(node, (ast.ClassDef, ast.FunctionDef, ast.AsyncFunctionDef,
                               ast.ExceptHandler, ast.MatchAs, ast.MatchStar)):
            bindings[node.name] += 1
        elif isinstance(node, ast.MatchMapping):
            bindings[node.rest] += 1
    for call in calls:
        alias = call["_python_self_attr_type"]
        name = imports.get(alias)
        if name and bindings[alias] == 1 and not bindings[name] and imported_names[name] == 1:
            call["_python_self_attr_type"] = name


def extract_python(path, *, root=None):
    """Resolve callback bindings and imported constructor aliases."""
    graph = _extract_python(path, root=root)
    calls = [c for c in graph.get("raw_calls", []) if c.get("receiver") == "self"
             and c.get("is_member_call") and not c.get("_python_attr_read")]
    typed_calls = [c for c in graph.get("raw_calls", []) if c.get("_python_self_attr_type")]
    if not calls and not typed_calls:
        return graph
    try:
        module = ast.parse(path.read_bytes())
    except (SyntaxError, ValueError, RecursionError, OSError):
        return graph
    if typed_calls:
        normalize_constructor_aliases(module, typed_calls)
    if not calls:
        return graph
    by_definition = {(n["label"], n.get("source_location")): n for n in graph["nodes"]}
    aliases_by_caller = {}
    for cls in (n for n in module.body if isinstance(n, ast.ClassDef)):
        methods = {}
        for fn in cls.body:
            if not isinstance(fn, (ast.FunctionDef, ast.AsyncFunctionDef)) or fn.decorator_list:
                continue
            args = fn.args.posonlyargs + fn.args.args
            if args and args[0].arg == "self":
                methods[fn.name] = fn
        init = methods.get("__init__")
        if init is None:
            continue
        writes = Counter(self_attribute(n) for n in ast.walk(cls)
                         if isinstance(n, ast.Attribute) and isinstance(n.ctx, (ast.Store, ast.Del)))
        aliases = {}
        for stmt in init.body:
            if not isinstance(stmt, ast.Assign) or len(stmt.targets) != 1:
                continue
            alias = self_attribute(stmt.targets[0])
            target = methods.get(self_attribute(stmt.value))
            if alias and writes[alias] == 1 and target:
                node = by_definition.get((f".{target.name}()", f"L{target.lineno}"))
                if node:
                    aliases[alias] = node["id"]
        if aliases:
            for fn in methods.values():
                node = by_definition.get((f".{fn.name}()", f"L{fn.lineno}"))
                if node:
                    aliases_by_caller[node["id"]] = aliases
    for call in calls:
        caller = call["caller_nid"]
        target = aliases_by_caller.get(caller, {}).get(call["callee"])
        if target:
            graph["edges"].append(dict(source=caller, target=target, relation="calls",
                                       confidence="INFERRED", source_file=call["source_file"],
                                       source_location=call["source_location"], weight=1.0))
    return graph


extractor.extract_python = extractor._DISPATCH[".py"] = extract_python

if __name__ == "__main__":
    from graphify.__main__ import main

    main()
