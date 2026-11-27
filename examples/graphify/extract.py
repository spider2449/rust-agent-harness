"""Run the pinned Graphify extractor with RAH workspace evidence."""

import ast
from collections import Counter, defaultdict
from hashlib import sha256
from pathlib import Path
from tokenize import open as open_python

from graphify import cargo_introspect, extract as extractor, manifest_ingest
from graphify.security import redact_secrets


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
_extract_workspace = extractor.extract


def self_attribute(node):
    if isinstance(node, ast.Attribute) and isinstance(node.value, ast.Name) and node.value.id == "self":
        return node.attr
    return None


def binding_counts(nodes):
    bindings = Counter()
    imported_names = Counter()
    for node in nodes:
        if isinstance(node, ast.alias):
            if node.name == "*":
                return Counter(), Counter()
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
    bindings.pop(None, None)
    return bindings, imported_names


def class_body_nodes(cls):
    """Walk class bindings without entering nested namespaces."""
    pending = list(cls.body)
    while pending:
        node = pending.pop()
        yield node
        if not isinstance(node, (ast.ClassDef, ast.FunctionDef, ast.AsyncFunctionDef, ast.Lambda,
                                 ast.ListComp, ast.SetComp, ast.DictComp, ast.GeneratorExp)):
            pending.extend(ast.iter_child_nodes(node))


def normalize_constructor_aliases(module, calls):
    """Let Graphify check class ownership and import edges using the declared name."""
    imports = {a.asname: a.name for stmt in module.body if isinstance(stmt, ast.ImportFrom)
               for a in stmt.names if a.asname}
    bindings, imported_names = binding_counts(ast.walk(module))
    for call in calls:
        alias = call["_python_self_attr_type"]
        name = imports.get(alias)
        if name and bindings[alias] == 1 and not bindings[name] and imported_names[name] == 1:
            call["_python_self_attr_type"] = name


def add_python_sites(module, graph, source_text):
    """Record per-location connection and member access evidence."""
    file_node = graph["nodes"][0]
    source = file_node["source_file"]
    source_key = sha256(source.replace("\\", "/").encode()).hexdigest()
    lines = source_text.encode().split(b"\n")
    bindings, _ = binding_counts(ast.walk(module))
    qt_imports = {
        alias.asname or alias.name
        for stmt in module.body
        if isinstance(stmt, ast.ImportFrom) and not stmt.level and stmt.module
        and stmt.module.split(".")[0] in {"PySide2", "PySide6", "PyQt5", "PyQt6"}
        for alias in stmt.names if bindings[alias.asname or alias.name] == 1
    }
    classes = {node.name: node for node in module.body if isinstance(node, ast.ClassDef)
               and bindings[node.name] == 1}
    members = {}
    for cls in classes.values():
        writes = Counter(self_attribute(n) for n in ast.walk(cls)
                         if isinstance(n, ast.Attribute) and isinstance(n.ctx, (ast.Store, ast.Del)))
        members[cls] = writes, {
            stmt.targets[0].attr
            for method in cls.body if isinstance(method, ast.FunctionDef)
            and method.name == "__init__" and not method.decorator_list
            for stmt in method.body if isinstance(stmt, ast.Assign) and len(stmt.targets) == 1
            and self_attribute(stmt.targets[0]) and writes[stmt.targets[0].attr] == 1
            and isinstance(stmt.value, ast.Call) and isinstance(stmt.value.func, ast.Name)
            and stmt.value.func.id in qt_imports
        }

    def qt_member(cls, name):
        while cls in members:
            writes, typed = members[cls]
            if writes[name]:
                return name in typed
            if len(cls.bases) != 1 or not isinstance(cls.bases[0], ast.Name):
                break
            base = classes.get(cls.bases[0].id)
            if base is None or base.lineno >= cls.lineno:
                break
            cls = base
        return False

    definitions = {(n["label"], n.get("source_location")): n for n in graph["nodes"]
                   if n.get("_callable")}
    imported_classes = {
        alias.asname or alias.name: ((stmt.module.replace(".", "/"), alias.name), stmt.lineno)
        for stmt in module.body if isinstance(stmt, ast.ImportFrom) and not stmt.level and stmt.module
        for alias in stmt.names if bindings[alias.asname or alias.name] == 1
    }
    scopes = {}
    for cls in classes.values():
        class_bindings, _ = binding_counts(class_body_nodes(cls))
        class_node = definitions.get((cls.name, f"L{cls.lineno}"))
        if class_node:
            base = None
            if len(cls.bases) == 1 and not cls.keywords and isinstance(cls.bases[0], ast.Name):
                base_name = cls.bases[0].id
                local = classes.get(base_name)
                imported = imported_classes.get(base_name)
                if local and local.lineno < cls.lineno:
                    base = (None, base_name)
                elif imported and imported[1] < cls.lineno:
                    base = imported[0]
            instance_writes = sorted(
                name for name in members[cls][0] if name is not None
            )
            class_node["_rah_callback_class"] = (base, sorted(class_bindings), instance_writes)
        names = defaultdict(list)
        for item in cls.body:
            if isinstance(item, (ast.FunctionDef, ast.AsyncFunctionDef)):
                names[item.name].append(item)
        definitions_by_name = {}
        for name, items in names.items():
            if len(items) != 1:
                continue
            method = items[0]
            parameters = method.args.posonlyargs + method.args.args
            if not parameters or parameters[0].arg != "self":
                continue
            target = definitions.get((f".{name}()", f"L{method.lineno}"))
            if target and (not method.decorator_list or target.get("_py_property")):
                definitions_by_name[name] = target
                if (not method.decorator_list and class_bindings[name] == 1
                        and not members[cls][0][name]):
                    target["_rah_callback_method"] = True
        constructor_fields = {}
        initializers = names.get("__init__", [])
        if "__init__" in definitions_by_name and initializers:
            for stmt in initializers[0].body:
                if isinstance(stmt, ast.Assign) and len(stmt.targets) == 1:
                    name = self_attribute(stmt.targets[0])
                    if name:
                        constructor_fields.setdefault(name, []).append(stmt)
        for name, assignments in constructor_fields.items():
            if len(assignments) != 1 or name in names or cls.bases:
                continue
            location = f"L{assignments[0].targets[0].lineno}"
            field_id = f"rah:python-field:{source_key}:{cls.lineno}:{name}"
            graph["nodes"].append(dict(id=field_id, label=name, file_type="code",
                                       source_file=source, source_location=location,
                                       rah_member=f"{cls.name}.{name}"))
            definitions_by_name[name] = graph["nodes"][-1]
            if class_node:
                graph["edges"].append(dict(source=class_node["id"], target=field_id, relation="contains",
                                           confidence="EXTRACTED", source_file=source,
                                           source_location=location, weight=1.0))
        scopes[cls] = class_bindings, definitions_by_name, constructor_fields

    def callback_target(cls, slot):
        name = self_attribute(slot)
        if name:
            return [None, cls.name, name]
        if isinstance(slot, ast.Attribute) and cls in scopes:
            receiver = self_attribute(slot.value)
            class_bindings, targets, fields = scopes[cls]
            if (receiver and not class_bindings[receiver] and members[cls][0][receiver] == 1
                    and targets.get("__init__", {}).get("_rah_callback_method")
                    and len(fields.get(receiver, [])) == 1):
                value = fields[receiver][0].value
                if isinstance(value, ast.Call) and isinstance(value.func, ast.Name):
                    imported = imported_classes.get(value.func.id)
                    if imported:
                        return [*imported[0], slot.attr]
        return None

    def expression(node):
        first, last = node.lineno - 1, node.end_lineno - 1
        if first == last:
            segment = lines[first][node.col_offset:node.end_col_offset]
        else:
            segment = b"\n".join([lines[first][node.col_offset:], *lines[first + 1:last],
                                  lines[last][:node.end_col_offset]])
        return redact_secrets(segment.decode())

    def visit(node, owner, cls=None, parent=None, method=False):
        if isinstance(node, ast.ClassDef):
            cls = node
            method = False
            owner = definitions.get((node.name, f"L{node.lineno}"), owner)
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            label = f"{node.name}()"
            candidates = (label, f".{label}") if cls is None else (f".{label}", label)
            owner = next((definitions[(name, f"L{node.lineno}")] for name in candidates
                          if (name, f"L{node.lineno}") in definitions), owner)
            method = cls in scopes and isinstance(parent, ast.ClassDef) and (
                scopes[cls][1].get(node.name) is owner)
        if isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute) and node.func.attr == "connect":
            signal = node.func.value
            event = signal.value if isinstance(signal, ast.Subscript) else signal
            receiver = event.value if isinstance(event, ast.Attribute) else event
            member = self_attribute(receiver)
            direct_qt = (isinstance(receiver, ast.Call) and isinstance(receiver.func, ast.Name)
                         and receiver.func.id in qt_imports)
            kind = "qt" if direct_qt or (member and qt_member(cls, member)) else "event"
            location = f"L{node.lineno}:{node.col_offset + 1}"
            site = f"rah:python-connect:{source_key}:{location}:{node.end_lineno}:{node.end_col_offset}"
            slot = node.args[0] if node.args else next(
                (keyword.value for keyword in node.keywords if keyword.arg in {"slot", "callback"}), None)
            graph["nodes"].append(dict(
                id=site, label=f".connect() {location}", file_type="code", source_file=source,
                source_location=location, rah_connection=dict(
                    version=1, kind=kind, owner=owner["label"],
                    signal=expression(signal), slot=expression(slot) if slot is not None else "",
                    expression=expression(node))))
            if method:
                graph["nodes"][-1]["_rah_callback"] = callback_target(cls, slot)
            graph["edges"].append(dict(source=owner["id"], target=site, relation="connection_site",
                                       confidence="EXTRACTED", source_file=source,
                                       source_location=location, weight=1.0))
        elif method and isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute):
            name = self_attribute(node.func)
            receiver = node.func.value
            indexed = False
            while isinstance(receiver, (ast.Attribute, ast.Subscript)):
                indexed |= isinstance(receiver, ast.Subscript)
                receiver = receiver.value
            if (name and members[cls][0][name] > 1
                    or indexed and isinstance(receiver, ast.Name) and receiver.id == "self"):
                location = f"L{node.lineno}:{node.col_offset + 1}"
                site = f"rah:python-call:{source_key}:{location}"
                graph["nodes"].append(dict(id=site, label=expression(node), file_type="code",
                                           source_file=source, source_location=location,
                                           rah_unresolved_call=True))
                graph["edges"].append(dict(source=owner["id"], target=site, relation="unresolved_call_site",
                                           confidence="EXTRACTED", source_file=source,
                                           source_location=location, weight=1.0))
        name = self_attribute(node)
        if method and name and not (isinstance(parent, ast.Call) and parent.func is node):
            target = scopes[cls][1].get(name)
            if target and (target.get("rah_member") or target.get("_py_property")):
                if isinstance(node.ctx, (ast.Load, ast.Store)) and not isinstance(parent, ast.AugAssign):
                    relation = "reads" if isinstance(node.ctx, ast.Load) else "writes"
                    location = f"L{node.lineno}:{node.col_offset + 1}"
                    site = f"rah:python-access:{source_key}:{location}"
                    graph["nodes"].append(dict(id=site, label=expression(node), file_type="code",
                                               source_file=source, source_location=location,
                                               rah_member_access=relation))
                    graph["edges"].append(dict(
                        source=owner["id"], target=site, relation="member_access_site",
                        confidence="EXTRACTED", source_file=source, source_location=location,
                        weight=1.0))
                    graph["edges"].append(dict(
                        source=site, target=target["id"], relation=relation,
                        confidence="EXTRACTED", source_file=source, source_location=location,
                        weight=1.0))
        for child in ast.iter_child_nodes(node):
            visit(child, owner, cls, node, method and not isinstance(node, ast.Lambda))

    visit(module, file_node)


def extract_python(path, *, root=None):
    """Resolve callback bindings and imported constructor aliases."""
    graph = _extract_python(path, root=root)
    calls = [c for c in graph.get("raw_calls", []) if c.get("receiver") == "self"
             and c.get("is_member_call") and not c.get("_python_attr_read")]
    typed_calls = [c for c in graph.get("raw_calls", []) if c.get("_python_self_attr_type")]
    if "error" in graph or not graph.get("nodes"):
        return graph
    try:
        with open_python(path) as stream:
            source_text = stream.read()
        module = ast.parse(source_text)
    except (SyntaxError, ValueError, RecursionError, OSError):
        return graph
    add_python_sites(module, graph, source_text)
    if typed_calls:
        normalize_constructor_aliases(module, typed_calls)
    if not calls:
        return graph
    by_definition = {(n["label"], n.get("source_location")): n for n in graph["nodes"]}
    aliases_by_caller = {}
    for cls in (n for n in module.body if isinstance(n, ast.ClassDef)):
        methods = {}
        declarations = Counter(fn.name for fn in cls.body
                               if isinstance(fn, (ast.FunctionDef, ast.AsyncFunctionDef)))
        for fn in cls.body:
            if not isinstance(fn, (ast.FunctionDef, ast.AsyncFunctionDef)) or fn.decorator_list:
                continue
            if declarations[fn.name] != 1:
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
            if alias and writes[alias] == 1 and target and not writes[target.name]:
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


def extract_workspace(*args, **kwargs):
    graph = _extract_workspace(*args, **kwargs)
    by_id = {node["id"]: node for node in graph["nodes"]}
    paths = {node["source_file"].replace("\\", "/") for node in graph["nodes"]
             if node.get("source_file")}
    classes = defaultdict(list)
    class_details = {}
    for node in graph["nodes"]:
        details = node.pop("_rah_callback_class", None)
        if details is not None:
            path = node["source_file"].replace("\\", "/")
            classes[path, node["label"]].append(node["id"])
            base, bindings, instance_writes = details
            class_details[node["id"]] = (path, base, set(bindings), set(instance_writes))
    methods = defaultdict(list)
    for edge in graph["edges"]:
        if edge["relation"] == "method":
            method = by_id.get(edge["target"])
            if edge["source"] in class_details and method and method.get("_rah_callback_method"):
                methods[edge["source"], method["label"]].append(method["id"])

    def resolve(path, module, cls, method):
        visited = set()
        target = None
        while True:
            if module is not None:
                matches = paths.intersection((module + ".py", module + "/__init__.py"))
                if len(matches) != 1:
                    break
                path = matches.pop()
            candidates = classes.get((path, cls), [])
            if len(candidates) != 1:
                break
            owner = candidates[0]
            path, base, bindings, instance_writes = class_details[owner]
            if owner in visited or method in instance_writes:
                return None
            visited.add(owner)
            if target is None:
                targets = methods.get((owner, f".{method}()"), [])
                if len(targets) == 1:
                    target = targets[0]
                elif targets or method in bindings:
                    return None
            # An inherited initializer can assign the instance attribute even
            # when a nearer class defines a method with the same name.
            if base is None:
                break
            module, cls = base
        return target

    for node in graph["nodes"]:
        candidate = node.pop("_rah_callback", None)
        if not node.get("rah_connection"):
            continue
        target = resolve(node["source_file"].replace("\\", "/"), *candidate) if candidate else None
        node["rah_connection"]["resolution"] = "inferred" if target else "unresolved"
        if target:
            graph["edges"].append(dict(source=node["id"], target=target,
                                       relation="callback", confidence="INFERRED",
                                       source_file=node["source_file"],
                                       source_location=node["source_location"], weight=1.0))
    for node in graph["nodes"]:
        node.pop("_rah_callback_method", None)
    return graph


extractor.extract = extract_workspace

if __name__ == "__main__":
    from graphify.__main__ import main

    main()
