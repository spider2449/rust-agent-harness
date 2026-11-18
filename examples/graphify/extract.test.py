"""Offline compatibility tests using the pinned Graphify Python environment."""

import tempfile
import json
import os
import subprocess
import unittest
import sys
from pathlib import Path

sys.dont_write_bytecode = True
import extract


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
