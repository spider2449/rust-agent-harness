"""Offline source-backed endpoint checks for the pinned extractor."""
import importlib.metadata
import tempfile
import unittest
from pathlib import Path
from graphify.extract import extract_python, extract_java
from graphify.manifest_ingest import extract_package_manifest
from graphify.cargo_introspect import introspect_cargo


class Relations(unittest.TestCase):
    def test_endpoint_conventions(self):
        self.assertEqual(importlib.metadata.version('graphifyy'), '0.9.107')
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            py = root / 'sample.py'
            py.write_text('import os\nclass Base: pass\nclass Child(Base):\n def run(self): return helper()\ndef helper(): pass\n')
            java = root / 'Sample.java'
            java.write_text('interface Face { void run(); } class Base {} class Child extends Base implements Face { public void run() { helper(); } void helper() {} }')
            def triples(graph):
                names = {n['id']: n['label'] for n in graph['nodes']}
                return {(names.get(e['source'], e['source']), e['relation'], names.get(e['target'], e['target'])) for e in graph['edges']}
            actual = triples(extract_python(py))
            for expected in [('sample.py', 'imports', 'os'), ('sample.py', 'contains', 'Child'),
                             ('Child', 'inherits', 'Base'), ('Child', 'method', '.run()'),
                             ('.run()', 'calls', 'helper()')]:
                self.assertIn(expected, actual)
            self.assertIn(('Child', 'implements', 'Face'), triples(extract_java(java)))
            manifest = root / 'Cargo.toml'
            manifest.write_text('[workspace]\nmembers=["a","b"]\nresolver="2"\n')
            for name in ['a', 'b']:
                member = root / name
                member.mkdir()
                (member / 'src').mkdir()
                (member / 'src/lib.rs').write_text('')
                (member / 'Cargo.toml').write_text(f'[package]\nname="{name}"\nversion="0.1.0"\n' + ('[dependencies]\nb={path="../b"}\n' if name == 'a' else ''))
            package = extract_package_manifest(root / 'a/Cargo.toml')
            self.assertTrue(any(e['source'] == 'pkg_a' and e['target'] == 'pkg_b' and e['relation'] == 'depends_on' for e in package['edges']))
            workspace = introspect_cargo(root)
            self.assertTrue(any(e['source'] == 'crate:a' and e['target'] == 'crate:b' and e['relation'] == 'crate_depends_on' for e in workspace['edges']))


if __name__ == '__main__':
    unittest.main()
