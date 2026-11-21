"""Measure source sites and graph relations independently of the main test suite."""
import argparse
import ast
import asyncio
from collections import Counter, defaultdict
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
from time import perf_counter

HERE = Path(__file__).resolve().parent
FIXTURE = HERE / "fixtures/navigation"
GRAPHIFY = "0.9.107"
CBM = "0.12.1"
NAPARI = "3ce2a1cb12d40fdaffc3dcf9fb6575320ad834ba"
QUERY = (
    "MATCH (a)-[r:CALLS|CALL_REFERENCE|USAGE]->(b) "
    "RETURN a.qualified_name,a.file_path,type(r),b.qualified_name,b.file_path,properties(r)"
)
NAPARI_QUERY = QUERY.replace("RETURN", "WHERE a.file_path STARTS WITH 'src/napari/_qt/' RETURN")


class Source(ast.NodeVisitor):
    def __init__(self, module):
        self.module = module
        self.scope = []
        self.definitions = {}
        self.properties = set()
        self.sites = []
        self.expressions = defaultdict(set)

    def visit_definition(self, node):
        self.scope.append(node)
        self.definitions[node.lineno] = self.symbol
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and any(
            isinstance(decorator, ast.Name) and decorator.id == "property"
            for decorator in node.decorator_list
        ):
            self.properties.add(node.lineno)
        self.generic_visit(node)
        self.scope.pop()

    visit_ClassDef = visit_FunctionDef = visit_AsyncFunctionDef = visit_definition

    @property
    def symbol(self):
        return ".".join([self.module, *(node.name for node in self.scope)])

    def visit_Attribute(self, node):
        kind = "writes" if isinstance(node.ctx, ast.Store) else "reads"
        self.expressions[node.lineno].add((kind, ast.unparse(node), self.symbol))
        self.generic_visit(node)

    def visit_Call(self, node):
        callee = ast.unparse(node.func)
        self.expressions[node.lineno].add(("calls", callee, self.symbol))
        if isinstance(node.func, ast.Attribute) and node.func.attr == "connect":
            arg = node.args[0] if node.args else None
            expected = None
            reason = "callback form outside the same-class rule"
            if isinstance(arg, ast.Attribute) and isinstance(arg.value, ast.Name) and arg.value.id == "self":
                reason = "local method missing, ambiguous, or decorated"
                if len(self.scope) == 2 and isinstance(self.scope[0], ast.ClassDef):
                    cls = self.scope[0]
                    methods = [method for method in cls.body
                               if isinstance(method, (ast.FunctionDef, ast.AsyncFunctionDef))
                               and method.name == arg.attr]
                    if len(methods) == 1 and not methods[0].decorator_list:
                        expected = (f"{self.module}.{cls.name}.{arg.attr}", methods[0].lineno)
                        reason = None
            self.sites.append(dict(line=node.lineno, column=node.col_offset, source=self.symbol,
                                   signal=ast.unparse(node.func.value), expression=ast.unparse(arg) if arg else None,
                                   kind="callback", expected=expected, reason=reason))
        self.generic_visit(node)


def source_cases(root, napari):
    sites, definitions = [], {}
    fixture = {} if napari else json.loads((HERE / "fixtures/navigation-cases.json").read_text())
    if napari:
        tracked, _ = run(["git", "ls-files", "-z", "--", "src/napari/_qt"], cwd=root)
        files = [root / file for file in sorted(tracked.split("\0")) if file.endswith(".py")]
    else:
        files = sorted(root.glob("*.py"))
    for file in files:
        relative = file.relative_to(root)
        module = ".".join(relative.with_suffix("").parts[2 if napari else 0:])
        scan = Source(module)
        scan.visit(ast.parse(file.read_bytes(), filename=str(relative)))
        definitions.update({(relative.as_posix(), line): symbol for line, symbol in scan.definitions.items()})
        if napari:
            sites.extend(dict(site, file=relative.as_posix()) for site in scan.sites)
            continue
        callbacks = {site["line"]: site for site in scan.sites}
        cases = fixture.get(relative.as_posix(), {})
        if callbacks.keys() != {int(line) for line, case in cases.items() if case[0] == "callback"}:
            raise ValueError(f"Fixture callback locations drifted in {relative}")
        for line, (kind, expression, expected, *classification) in cases.items():
            line = int(line)
            if kind == "callback":
                site = callbacks[line]
                if site["expression"] != expression:
                    raise ValueError(f"Fixture callback expression drifted at {relative}:{line}")
                site["classification"] = classification[0]
            else:
                if kind == "property":
                    sources = [scan.definitions[line]]
                    if line not in scan.properties or sources[0].rsplit(".", 1)[-1] != expression:
                        raise ValueError(f"Fixture property drifted at {relative}:{line}")
                else:
                    sources = [source for role, value, source in scan.expressions[line]
                               if role == kind and (value == expression or value.endswith("." + expression))]
                if len(sources) != 1:
                    raise ValueError(f"Fixture expression drifted at {relative}:{line}")
                site = dict(line=line, source=sources[0], expression=expression, kind=kind)
            sites.append(dict(site, file=relative.as_posix(), expected=(expected, None) if expected else None,
                              reason=None if expected else "outside statically asserted targets"))
    if not napari:
        lines_by_symbol = {symbol: line for (_, line), symbol in definitions.items()}
        for site in sites:
            if site["expected"]:
                symbol, _ = site["expected"]
                site["expected"] = (symbol, lines_by_symbol.get(symbol))
    return sites, definitions, len(files)


def run(command, *, env=None, cwd=None):
    start = perf_counter()
    result = subprocess.run(command, env=env, cwd=cwd, check=True, stdout=subprocess.PIPE,
                            encoding="utf-8", timeout=600)
    return result.stdout, round(perf_counter() - start, 3)


def file_name(root, name):
    if not name:
        return None
    path = Path(name)
    if path.is_absolute():
        try:
            path = path.relative_to(root)
        except ValueError:
            return None
    return path.as_posix()


async def graphify_query(graph_path, question):
    from mcp import ClientSession, StdioServerParameters
    from mcp.client.stdio import stdio_client

    server = StdioServerParameters(command=sys.executable, args=[
        "-I", "-m", "graphify.serve", "--graph", str(graph_path),
    ])
    async with asyncio.timeout(30):
        async with stdio_client(server) as (reader, writer):
            async with ClientSession(reader, writer) as session:
                await session.initialize()
                start = perf_counter()
                result = await session.call_tool("query_graph", {
                    "question": question, "depth": 1, "token_budget": 512,
                })
                if result.model_dump(by_alias=True).get("isError"):
                    raise ValueError(f"Graphify query failed: {result.content}")
                return round(perf_counter() - start, 3)


def graphify(root, stage, definitions, question):
    from importlib.metadata import version
    actual = version("graphifyy")
    if actual != GRAPHIFY:
        raise ValueError(f"Expected Graphify {GRAPHIFY}, got {actual}")
    _, elapsed = run([sys.executable, "-I", str(HERE / "extract.py"), "extract", str(root),
                      "--code-only", "--no-cluster", "--max-workers", "4", "--exclude", ".rah/",
                      "--out", str(stage)], cwd=root, env={**os.environ, "PYTHONHASHSEED": "0"})
    start = perf_counter()
    graph_path = stage / "graphify-out/graph.json"
    graph = json.loads(graph_path.read_text(encoding="utf-8"))

    def location(entry):
        match = re.fullmatch(r"L(\d+)(?:-L\d+)?", entry.get("source_location") or "")
        return file_name(root, entry.get("source_file")), int(match[1]) if match else None

    nodes = {node["id"]: node for node in graph["nodes"]}
    edges = []
    for edge in graph["edges"] if "edges" in graph else graph["links"]:
        source, target = nodes[edge["source"]], nodes[edge["target"]]
        source_file, source_line = location(source)
        caller = definitions.get((source_file, source_line))
        if caller is None:
            continue
        target_file, target_line = location(target)
        edge_file, edge_line = location(edge)
        context = edge.get("context")
        relation = "reads" if context == "property_read" else edge["relation"]
        edges.append(dict(source=caller, file=source_file, relation=relation, context=context,
                          target=definitions.get((target_file, target_line), target["label"]),
                          target_file=target_file, target_line=target_line,
                          line=edge_line if edge_file == source_file else None,
                          expression=target["label"].strip(".()"),
                          confidence=edge.get("confidence")))
    for node in nodes.values():
        file, line = location(node)
        symbol = definitions.get((file, line))
        if node.get("_py_property") and symbol:
            edges.append(dict(source=symbol, file=file, line=line, relation="property",
                              target=symbol, target_file=file, target_line=line,
                              expression=symbol.rsplit(".", 1)[-1], callable_label=node["label"]))
    artifact_read_seconds = round(perf_counter() - start, 3)
    query_seconds = asyncio.run(graphify_query(graph_path, question))
    return edges, dict(version=actual, extract_seconds=elapsed, artifact_read_seconds=artifact_read_seconds,
                       sample_query=question, sample_query_seconds=query_seconds)


def cbm(root, stage, binary, napari):
    actual, _ = run([str(binary), "--version"])
    if actual.strip() != f"codebase-memory-mcp {CBM}":
        raise ValueError(f"Unexpected CBM version {actual}")
    stage.mkdir()
    env = dict(os.environ, LOCALAPPDATA=str(stage), XDG_CACHE_HOME=str(stage))
    command = [str(binary), "cli", "--quiet"]
    project = "rah-navigation-baseline"
    result, index_seconds = run(command + ["index_repository", "--repo-path", str(root),
                                           "--name", project, "--mode", "fast"], env=env)
    if json.loads(result)["status"] != "indexed":
        raise ValueError(result)
    result, query_seconds = run(command + ["query_graph", "--project", project, "--query",
                                           NAPARI_QUERY if napari else QUERY, "--format", "json",
                                           "--max-rows", "99998", "--max-output-tokens", "10000000"], env=env)
    response = json.loads(result)
    if response["truncated"] or response["has_more"] or response["total"] != len(response["rows"]):
        raise ValueError("Incomplete CBM query")
    prefix = project + (".src.napari." if napari else ".")
    edges = []
    for source, file, relation, target, target_file, properties in response["rows"]:
        detail = json.loads(properties)
        edges.append(dict(source=source.removeprefix(prefix), file=file_name(root, file),
                          relation=relation, target=target.removeprefix(prefix),
                          target_file=file_name(root, target_file), line=detail.get("line"),
                          expression=detail.get("callee"), strategy=detail.get("strategy"),
                          confidence=detail.get("confidence")))
    with binary.open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
    return edges, dict(version=CBM, sha256=digest,
                       index_seconds=index_seconds, query_seconds=query_seconds, queried_edges=len(edges))


def score(sites, edges):
    by_scope = defaultdict(list)
    for edge in edges:
        by_scope[edge["file"], edge["source"]].append(edge)
    rows = []
    groups = defaultdict(list)
    for site in sites:
        expected = site["expected"][0] if site["expected"] else None
        expression = (site["expression"] or "").rsplit(".", 1)[-1]
        candidates = []
        for edge in by_scope[site["file"], site["source"]]:
            if site["kind"] == "callback":
                if edge["relation"] not in ("CALL_REFERENCE", "USAGE"):
                    continue
            elif site["kind"] == "calls":
                if edge["relation"] not in ("CALLS", "calls"):
                    continue
            elif edge["relation"].lower() != site["kind"]:
                continue
            observed = (edge["expression"] or "").rsplit(".", 1)[-1]
            if edge["line"] not in (None, site["line"]):
                continue
            if observed != expression and (edge["target"] != expected or not expected):
                if site["kind"] != "calls" or edge["line"] != site["line"]:
                    continue
            candidates.append(edge)
            if expected and edge["line"] is None:
                groups[site["file"], site["source"], edge["relation"], edge["target"], expression].append(
                    (site["line"], site.get("column", 0)))
        targets = {edge["target"] for edge in candidates}
        status = target_status(expected, targets) if expected else "unscored"
        rows.append(dict(site, status=status, candidates=candidates,
                         position_hit=any(edge["target"] == expected and edge["line"] == site["line"]
                                          for edge in candidates)))
    pairs = defaultdict(set)
    for row in rows:
        if row["expected"]:
            pairs[row["source"], row["expected"][0], row["kind"]].update(
                edge["target"] for edge in row["candidates"])
    callbacks = [row for row in rows if row["kind"] == "callback"]
    callback_references = {
        (row["source"], row["expected"][0], edge["relation"])
        for row in callbacks if row["expected"] for edge in row["candidates"]
        if edge["target"] == row["expected"][0]
    }
    return dict(sites=dict(Counter(row["status"] for row in rows)),
                callback_sites=dict(Counter(row["status"] for row in callbacks)),
                callback_position_hits=sum(row["position_hit"] for row in callbacks),
                correct_callback_pairs_by_relation=dict(Counter(relation for _, _, relation in callback_references)),
                target_pairs=dict(Counter(target_status(expected, targets)
                                          for (_, expected, _), targets in pairs.items())),
                position_hits=sum(row["position_hit"] for row in rows),
                merged_candidates=[dict(file=file, source=source, relation=relation, target=target,
                                        expression=expression, positions=sorted(set(positions)))
                                   for (file, source, relation, target, expression), positions in sorted(groups.items())
                                   if len(set(positions)) > 1],
                rows=rows)


def target_status(expected, targets):
    if expected in targets:
        return "hit" if len(targets) == 1 else "hit_with_wrong"
    return "wrong" if targets else "miss"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--napari", type=Path, help="Clean pinned napari checkout")
    parser.add_argument("--graphify", action="store_true", help="Use this Python's managed Graphify environment")
    parser.add_argument("--cbm", type=Path, help="Absolute path to an independently verified CBM binary")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="rah-navigation-") as tmp:
        stage = Path(tmp)
        if args.napari:
            root = args.napari.resolve(strict=True)
            head, _ = run(["git", "rev-parse", "HEAD"], cwd=root)
            status, _ = run(["git", "status", "--porcelain", "--untracked-files=all"], cwd=root)
            if head.strip() != NAPARI or status.strip():
                parser.error(f"napari must be a clean checkout of {NAPARI}")
        else:
            root = stage / "fixture"
            shutil.copytree(FIXTURE, root, ignore=shutil.ignore_patterns("__pycache__"))
        start = perf_counter()
        sites, definitions, files = source_cases(root, bool(args.napari))
        report = dict(dataset=NAPARI if args.napari else "fixture", files=files, sites=len(sites),
                      callback_sites=sum(site["kind"] == "callback" for site in sites),
                      unscored=dict(Counter(site["reason"] for site in sites if site["reason"])),
                      source_scan_seconds=round(perf_counter() - start, 3), source=sites)
        if args.graphify:
            question = "QtViewer" if args.napari else "menuHelp_triggered"
            edges, measurements = graphify(root, stage / "graphify", definitions, question)
            report["graphify"] = dict(measurements, **score(sites, edges))
        if args.cbm:
            edges, measurements = cbm(root, stage / "cbm", args.cbm.resolve(strict=True), bool(args.napari))
            report["cbm"] = dict(measurements, **score(sites, edges))
        print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
