"""Offline compatibility tests using the pinned Graphify Python environment."""

import tempfile
import json
import os
import subprocess
import unittest
import sys
import shutil
from pathlib import Path
from textwrap import dedent

sys.dont_write_bytecode = True
import extract


class ConnectionEvidenceTests(unittest.TestCase):
    def test_inherited_cross_file_callback_uses_nearest_unambiguous_definition(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "project"
            root.mkdir()
            (root / "base.py").write_text("class Base:\n    def on_done(self): pass\n")
            (root / "worker.py").write_text(dedent("""\
                from base import Base as Parent

                class Inherited(Parent): pass
                class Override(Inherited):
                    def on_done(self): pass
                class Shadowed(Inherited):
                    if True:
                        on_done = None
                class Decorated(Inherited):
                    @property
                    def on_done(self): pass
                class Rebound(Inherited):
                    def reset(self):
                        self.on_done = None
                class Mixin: pass
                class Multiple(Mixin, Inherited): pass
                class Direct(Parent):
                    def wire(self): self.signal.connect(self.on_done)
                class AssignsCallback(Inherited):
                    def __init__(self):
                        self.on_done = self.fallback
                    def fallback(self): pass
                class ReboundOverride(AssignsCallback):
                    def on_done(self): pass
                class WithHandler(Parent):
                    try:
                        pass
                    except Exception:
                        pass
                """))
            cases = {
                "inherited": ("Inherited", "", ("base.py", "L2")),
                "handler": ("WithHandler", "", ("base.py", "L2")),
                "override": ("Override", "", ("worker.py", "L5")),
                "shadowed": ("Shadowed", "", None),
                "decorated": ("Decorated", "", None),
                "rebound": ("Rebound", "", None),
                "rebound_override": ("ReboundOverride", "", None),
                "multiple": ("Multiple", "", None),
                "reassigned": ("Inherited", "        self.worker = None\n", None),
                "missing": ("Missing", "", None),
            }
            for name, (worker, extra, _) in cases.items():
                (root / f"{name}.py").write_text(
                    f"from worker import {worker} as W\n"
                    "from PySide6.QtWidgets import QPushButton\n"
                    "class App:\n"
                    "    def __init__(self):\n"
                    "        self.worker = W()\n"
                    "        self.button = QPushButton()\n"
                    f"{extra}"
                    "    def wire(self):\n"
                    "        self.button.clicked.connect(self.worker.on_done)\n")
            out = root.parent / "stage"
            subprocess.run([sys.executable, "-I", str(Path(extract.__file__).resolve()),
                            "extract", str(root), "--code-only", "--no-cluster", "--out", str(out)],
                           cwd=root, env={**os.environ, "PYTHONHASHSEED": "0"}, check=True)
            graph = json.loads((out / "graphify-out/graph.json").read_bytes())

        nodes = {n["id"]: n for n in graph["nodes"]}
        sites = {n["source_file"]: n for n in nodes.values() if "rah_connection" in n}
        callbacks = {}
        for edge in graph.get("edges", graph.get("links")):
            if edge["relation"] == "callback":
                site, target = nodes[edge["source"]], nodes[edge["target"]]
                self.assertNotIn(site["source_file"], callbacks)
                self.assertEqual(edge["confidence"], "INFERRED")
                self.assertEqual(edge["source_location"], site["source_location"])
                self.assertEqual(target["label"], ".on_done()")
                callbacks[site["source_file"]] = (target["source_file"], target["source_location"])
        self.assertEqual(set(sites), {f"{name}.py" for name in cases} | {"worker.py"})
        for name, (_, extra, target) in cases.items():
            with self.subTest(name=name):
                file = f"{name}.py"
                site = sites[file]
                self.assertEqual(site["source_location"], "L9:9" if extra else "L8:9")
                self.assertEqual(site["rah_connection"]["kind"], "qt")
                self.assertEqual(site["rah_connection"]["slot"], "self.worker.on_done")
                self.assertEqual(site["rah_connection"]["resolution"],
                                 "inferred" if target else "unresolved")
                self.assertEqual(callbacks.get(file), target)
        self.assertEqual(callbacks["worker.py"], ("base.py", "L2"))
        self.assertFalse(any("_rah_callback_class" in node for node in nodes.values()))

    def test_export_preserves_each_site_and_source(self):
        fixtures = Path(__file__).parent / "fixtures"
        cases = json.loads((fixtures / "navigation-cases.json").read_bytes())
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "project"
            shutil.copytree(fixtures / "navigation", root)
            for index in range(20):
                (root / f"extra_{index}.py").write_text("value = 1\n")
            with (root / "window.py").open("a", encoding="utf-8") as stream:
                stream.write("\nEvent().connect(lambda: None).connect(callback=next_handler)\n")
            stage = root.parent / "stage"
            subprocess.run([sys.executable, "-I", str(Path(extract.__file__).resolve()),
                            "extract", str(root), "--code-only", "--no-cluster", "--max-workers", "4",
                            "--out", str(stage)], cwd=root,
                           env={**os.environ, "PYTHONHASHSEED": "0"}, check=True)
            graph = json.loads((stage / "graphify-out/graph.json").read_bytes())
        sites = {n["id"]: n for n in graph["nodes"] if "rah_connection" in n}
        self.assertEqual(len(sites), 7)
        edges = [e for e in graph.get("edges", graph.get("links")) if e["relation"] == "connection_site"]
        self.assertCountEqual([e["target"] for e in edges], sites.keys())
        owners = {n["id"]: n for n in graph["nodes"]}
        for site in sites.values():
            evidence = site["rah_connection"]
            line = site["source_location"].split(":")[0][1:]
            self.assertEqual(site["source_file"], "window.py")
            if line in cases["window.py"]:
                _, slot, _, kind = cases["window.py"][line]
                self.assertEqual((evidence["slot"], evidence["kind"]), (slot, kind))
                self.assertTrue(evidence["owner"].startswith(".wire"))
            else:
                self.assertIn(evidence["slot"], {"lambda: None", "next_handler"})
                self.assertEqual(evidence["owner"], "window.py")
        for edge in edges:
            self.assertEqual(owners[edge["source"]]["label"], sites[edge["target"]]["rah_connection"]["owner"])
        multiline = next(n for n in sites.values() if n["source_location"].startswith("L37:"))
        self.assertIn("\n", multiline["rah_connection"]["expression"])
        callbacks = {
            (sites[e["source"]]["source_location"].split(":")[0],
             owners[e["target"]]["source_file"], owners[e["target"]]["label"])
            for e in graph.get("edges", graph.get("links")) if e["relation"] == "callback"
        }
        self.assertEqual(callbacks, {
            ("L33", "models.py", ".selSwCtl_trd()"),
            ("L36", "window.py", ".menuHelp_triggered()"),
            ("L37", "window.py", ".menuHelp_triggered()"),
            ("L50", "window.py", ".menuHelp_triggered()"),
            ("L78", "window.py", ".receive()"),
        })
        unresolved = [n for n in sites.values() if n["rah_connection"]["resolution"] == "unresolved"]
        self.assertEqual(len(unresolved), 2)
        access = {(e["relation"], e["source_location"].split(":")[0],
                   owners[e["target"]]["label"])
                  for e in graph.get("edges", graph.get("links"))
                  if e["relation"] in {"reads", "writes"}}
        self.assertTrue({("reads", "L24", "isSetKey"), ("writes", "L27", "isSetKey"),
                         ("reads", "L45", ".switchValue()")} <= access)
        unresolved_calls = {n["source_location"].split(":")[0] for n in graph["nodes"]
                            if n.get("rah_unresolved_call")}
        self.assertEqual(unresolved_calls, {"L21", "L65"})


class InstanceCallbackTests(unittest.TestCase):
    def test_imported_constructor_alias_in_exported_graph(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "project"
            root.mkdir()
            (root / "worker.py").write_text("class Worker:\n def run(self): pass\n")
            app = ("from worker import Worker as W\nclass App:\n"
                   " def __init__(self): self.worker = W()\n"
                   " def start(self): self.worker.run()\n")
            (root / "app.py").write_text(app)
            (root / "shadowed.py").write_text(app.replace("__init__(self)", "__init__(self, W)"))
            (root / "plain.py").write_text(app.replace(" as W", "").replace("W()", "Worker()"))
            out = root.parent / "stage"
            subprocess.run([sys.executable, "-I", str(Path(extract.__file__).resolve()),
                            "extract", str(root), "--code-only", "--no-cluster", "--out", str(out)],
                           cwd=root, env={**os.environ, "PYTHONHASHSEED": "0"}, check=True)
            graph = json.loads((out / "graphify-out/graph.json").read_bytes())
            nodes = {n["id"]: n for n in graph["nodes"]}
            sites = [(nodes[e["source"]]["source_file"], e["source_location"], e["confidence"])
                     for e in graph.get("edges", graph.get("links"))
                     if e["relation"] == "calls" and nodes[e["target"]]["source_file"] == "worker.py"
                     and nodes[e["target"]]["label"] == ".run()"]
            self.assertCountEqual(sites, [("app.py", "L4", "INFERRED"), ("plain.py", "L4", "INFERRED")])

    def test_unique_binding_resolves_to_same_class_method_at_call_site(self):
        source = """class Other:
    def handle(self): pass

class Controller:
    def __init__(self):
        self.callback = self.handle
        self.changing = self.handle
    def handle(self): pass
    def run(self):
        self.callback()
        self.changing()
        self.changing = None
        return self.callback
"""
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "example.py"
            path.write_text(source)
            graph = extract.extractor._get_extractor(path)(path)
            locations = {n["id"]: n["source_location"] for n in graph["nodes"]}
            calls = [(locations[e["source"]], locations[e["target"]], e["source_location"])
                     for e in graph["edges"] if e["relation"] == "calls" and e["confidence"] == "INFERRED"]
            self.assertEqual(calls, [("L9", "L8", "L10")])


class CargoManifestCompatibility(unittest.TestCase):
    def test_plain_bom_and_member_manifests_share_the_compatible_reader(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            plain = root / "Cargo.toml"
            member = root / "member" / "Cargo.toml"
            member.parent.mkdir()
            plain.write_text('[package]\nname = "plain"\n', encoding="utf-8")
            member.write_text('[package]\nname = "bom-member"\n[dependencies]\nserde = "1"\n', encoding="utf-8-sig")
            self.assertEqual(extract.load_manifest(plain)["package"]["name"], "plain")
            self.assertEqual(extract.load_manifest(member)["package"]["name"], "bom-member")
            self.assertEqual(extract.cargo_introspect.package_name(member), "bom_member")
            graph = extract.manifest_ingest.extract_package_manifest(member)
            self.assertEqual(graph["nodes"][0]["id"], "pkg_bom_member")
            self.assertEqual(graph["edges"][0]["target"], "pkg_serde")

    def test_invalid_toml_and_invalid_encoding_remain_errors(self):
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "Cargo.toml"
            manifest.write_text("\ufeff[invalid syntax", encoding="utf-8")
            with self.assertRaises(ValueError):
                extract.load_manifest(manifest)
            manifest.write_bytes(b"\xff\xfe[package]")
            with self.assertRaises(UnicodeError):
                extract.load_manifest(manifest)


if __name__ == "__main__":
    unittest.main()
