"""Run with Maya 2023 or 2025 mayapy. Uses a fresh scene and a loopback port."""
from contextlib import nullcontext
import json
import os
import random
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch
from uuid import uuid4
import socketserver
import rah_maya
import maya.utils

import maya.standalone
from maya import cmds
try:
    from PySide2.QtCore import QCoreApplication
except ImportError:
    from PySide6.QtCore import QCoreApplication

PLUGIN = Path(__file__).with_name("rah_maya.py").resolve()


class DesktopRouting(unittest.TestCase):
    def test_locate_prefers_running_maya_without_launching(self):
        with tempfile.TemporaryDirectory() as directory, patch.dict(os.environ, {}, clear=True), \
                patch.object(rah_maya, "maya_process_ids", return_value=[42]), \
                patch.object(os, "startfile", side_effect=AssertionError):
            root = Path(directory)
            running_bin = root / "running" / "bin"
            running_bin.mkdir(parents=True)
            running = running_bin / "maya.exe"
            running.touch()
            mayapy = running.with_name("mayapy.exe")
            mayapy.touch()
            installed = root / "maya.exe"
            installed.touch()
            with patch.object(rah_maya, "running_executables", return_value=[running]) as query, \
                    patch.object(rah_maya, "executable_candidates", return_value=[installed, running]):
                result = rah_maya.call_maya("maya.locate", {}, None)
            query.assert_called_once_with([42])
            info = result["content"][0]["json"]
            self.assertEqual(info["executable"], str(running))
            self.assertEqual(info["installations"], [
                {"executable": str(running), "mayapy": str(mayapy)},
                {"executable": str(installed), "mayapy": None},
            ])

    def test_locate_falls_back_to_path_without_a_readable_running_path(self):
        with tempfile.TemporaryDirectory() as directory, patch.dict(os.environ, clear=True, PATH=directory), \
                patch.object(rah_maya, "running_executables", return_value=[]):
            executable = Path(directory, "maya.exe")
            executable.touch()
            os.environ["RAH_MAYA_EXE"] = str(Path(directory, "missing", "maya.exe"))
            for process_ids in ([], [42]):
                with self.subTest(process_ids=process_ids):
                    self.assertEqual(rah_maya.locate(process_ids)["executable"], str(executable))

    def test_locate_accepts_the_installation_directory(self):
        with tempfile.TemporaryDirectory() as directory, patch.dict(os.environ, {}, clear=True):
            root = Path(directory)
            bin_dir = root / "bin"
            bin_dir.mkdir()
            executable = bin_dir / "maya.exe"
            executable.touch()
            os.environ["MAYA_LOCATION"] = str(root)
            self.assertEqual(rah_maya.locate(())["executable"], str(executable))

    def test_missing_maya_requests_the_installation_directory(self):
        with patch.dict(os.environ, {}, clear=True), patch.object(rah_maya, "registered_port", return_value=None), \
                patch.object(rah_maya, "maya_process_ids", return_value=[]), \
                patch.object(rah_maya, "executable_candidates", return_value=[]), \
                patch.object(os, "startfile") as launch:
            info = rah_maya.locate()
            self.assertIsNone(info["executable"])
            self.assertIn("provide the Maya installation directory", info["instructions"])
            with self.assertRaisesRegex(rah_maya.NotStarted, "MAYA_LOCATION"):
                rah_maya.desktop_port()
            launch.assert_not_called()

    def test_registry_lists_2023_and_newer_in_descending_order(self):
        import winreg

        versions = ["2022", "2023", "2025", "2030", "Capabilities"]
        with patch.dict(os.environ, {}, clear=True), patch.object(os, "get_exec_path", return_value=[]), \
                patch.object(winreg, "OpenKey") as open_key, \
                patch.object(winreg, "QueryInfoKey", return_value=(len(versions), 0, 0)), \
                patch.object(winreg, "EnumKey", side_effect=lambda _, index: versions[index]), \
                patch.object(winreg, "QueryValueEx", side_effect=lambda key, _: (key, 0)):
            open_key.side_effect = lambda _, path, **kwargs: nullcontext(path)
            candidates = list(rah_maya.executable_candidates())
        self.assertEqual([p.parts[0] for p in candidates], ["2030", "2025", "2023"])

    def test_reuses_registered_port(self):
        with patch.object(rah_maya, "registered_port", return_value=50010), patch.object(os, "startfile") as launch:
            self.assertEqual(rah_maya.desktop_port(), 50010)
            launch.assert_not_called()

    def test_waits_for_startup_and_only_launches_if_missing(self):
        for running in (False, True):
            with self.subTest(running=running), patch.dict(os.environ, RAH_MAYA_PORT_NAME=rah_maya.DEFAULT_PORT_NAME), \
                    patch.object(rah_maya, "registered_port", side_effect=[None, 50010]), \
                    patch.object(rah_maya, "maya_process_ids", return_value=[42] if running else []), \
                    patch.object(rah_maya, "locate", return_value={"executable": "maya.exe"}), \
                    patch.object(os, "startfile") as launch, patch.object(time, "sleep"):
                self.assertEqual(rah_maya.desktop_port(), 50010)
                self.assertEqual(launch.call_count, int(not running))

    def test_missing_custom_port_does_not_launch_default_maya(self):
        with patch.dict(os.environ, RAH_MAYA_PORT_NAME="rahMaya2023"), \
                patch.object(rah_maya, "registered_port", return_value=None), patch.object(os, "startfile") as launch:
            with self.assertRaisesRegex(rah_maya.NotStarted, "Open the named MEL command port rahMaya2023"):
                rah_maya.desktop_port()
            launch.assert_not_called()


class MayaIntegration(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.address = "rahTest" + uuid4().hex
        maya.standalone.initialize()
        cls.app = QCoreApplication.instance() or QCoreApplication([])
        from maya.app.general import CommandPort
        # Maya standalone stops its handler thread but leaves its listening
        # socket to garbage collection. Close the fixture socket explicitly.
        class Server(socketserver.TCPServer):
            def server_activate(self):
                super().server_activate()
                cls.addClassCleanup(self.server_close)

        with patch.object(socketserver, "TCPServer", Server):
            error = CommandPort.openCommandPort(cls.address, "mel", "", True, False, False,
                                                4096, False, "rah_result")
        if error:
            raise RuntimeError(error)
        with patch.dict(os.environ, RAH_MAYA_PORT_NAME=cls.address):
            cls.port = rah_maya.registered_port()
        if cls.port is None:
            raise RuntimeError(f"Maya did not register {cls.address}")

    @classmethod
    def tearDownClass(cls):
        from maya.app.general import CommandPort
        CommandPort.closeCommandPort(cls.address)
        maya.standalone.uninitialize()

    def rpc(self, messages):
        output = tempfile.TemporaryFile()
        proc = subprocess.Popen([sys.executable, "-B", str(PLUGIN)], stdin=subprocess.PIPE,
                                stdout=output, stderr=subprocess.PIPE)
        try:
            for i, (method, params) in enumerate(messages + [("shutdown", {})]):
                proc.stdin.write((json.dumps({"jsonrpc": "2.0", "id": i, "method": method,
                                              "params": params}) + "\n").encode())
            proc.stdin.close()
            deadline = time.monotonic() + 35
            while proc.poll() is None and time.monotonic() < deadline:
                self.app.processEvents()
                maya.utils.processIdleEvents()
                time.sleep(0.005)
            self.assertIsNotNone(proc.poll(), "Maya client timed out")
            self.assertEqual(proc.returncode, 0, proc.stderr.read())
            output.seek(0)
            return [json.loads(line) for line in output]
        finally:
            if proc.poll() is None:
                proc.kill()
            proc.wait()
            output.close()
            proc.stderr.close()

    def test_scene_workflow(self):
        def call(tool_name, **args):
            return ("tools/call", {"name": "maya." + tool_name, "arguments": args})

        replies = self.rpc([
            ("initialize", {"config": {"port": self.port}}),
            call("create", shape="cube", name="rahBox"),
            call("list", scope="selection"),
            call("move", node="|rahBox", x=1, y=-2, z=3.5),
            call("eval_mel", mel='xform -q -ws -t "rahBox";'),
            call("help", command="polyCube"),
            call("move", node='box"; delete "rahBox', x=0, y=0, z=0),
            call("eval_mel", mel='noSuchRahCommand;'),
        ])
        def result(i):
            return replies[i]["result"]["content"][0]["json"]["result"]

        self.assertEqual({t["name"] for t in replies[0]["result"]["tools"]},
                         {t["name"] for t in rah_maya.TOOLS})
        self.assertEqual(result(1)[0], "rahBox")
        self.assertIn("|rahBox", result(2))
        self.assertEqual(result(4), [1, -2, 3.5])
        self.assertIn("-width", str(result(5)))
        self.assertEqual(replies[6]["error"]["data"]["kind"], "invalid_input")
        self.assertNotIn("data", replies[7]["error"])
        self.assertTrue(cmds.objExists("rahBox"))

    def test_session(self):
        reply = self.rpc([("initialize", {"config": {"port": self.port}}),
                          ("tools/call", {"name": "maya.session", "arguments": {}})])[1]
        info = reply["result"]["content"][0]["json"]["result"]
        self.assertEqual(info["port"], self.port)
        self.assertEqual(info["maya"], cmds.about(version=True))
        self.assertEqual(info["executable"], sys.executable)
        self.assertTrue(info["qt"])

    def test_inspect_and_select_in_named_session(self):
        left = cmds.createNode("transform", name="rahLeft")
        right = cmds.createNode("transform", name="rahRight")
        cmds.createNode("transform", parent=left, name="rahChild")
        cmds.createNode("transform", parent=right, name="rahChild")
        self.addCleanup(cmds.delete, [left, right])

        def call(name, node):
            return ("tools/call", {"name": "maya." + name, "arguments": {"node": node}})

        with patch.dict(os.environ, RAH_MAYA_PORT_NAME=self.address):
            replies = self.rpc([("initialize", {}), call("inspect", "rahChild"),
                                call("inspect", "|rahLeft|rahChild"), call("select", "|rahRight|rahChild"),
                                call("select", "*"), call("inspect", "rahMissingNode")])
        self.assertIn("Ambiguous transform", replies[1]["error"]["message"])
        inspected = replies[2]["result"]["content"][0]["json"]["result"]
        self.assertEqual(inspected["path"], "|rahLeft|rahChild")
        self.assertEqual(inspected["world_position"], [0, 0, 0])
        self.assertEqual(inspected["local_scale"], [1, 1, 1])
        self.assertEqual(replies[3]["result"]["content"][0]["json"]["result"], ["|rahRight|rahChild"])
        self.assertEqual(replies[4]["error"]["data"]["kind"], "invalid_input")
        self.assertIn("No transform found", replies[5]["error"]["message"])

    def test_rotate_scale_and_verify(self):
        node = cmds.createNode("transform", name="rahTransform")
        self.addCleanup(cmds.delete, node)
        replies = self.rpc([
            ("initialize", {"config": {"port": self.port}}),
            ("tools/call", {"name": "maya.rotate", "arguments": {"node": node, "x": 0, "y": 30, "z": 0}}),
            ("tools/call", {"name": "maya.scale", "arguments": {"node": node, "x": 2, "y": 3, "z": 1}}),
            ("tools/call", {"name": "maya.inspect", "arguments": {"node": node}}),
        ])
        for response in replies[1:4]:
            self.assertNotIn("error", response)
        info = replies[3]["result"]["content"][0]["json"]["result"]
        self.assertAlmostEqual(info["world_rotation"][1], 30)
        self.assertEqual(info["local_scale"], [2, 3, 1])

    def test_evaluators_and_module_paths(self):
        def call(name, **arguments):
            return ("tools/call", {"name": "maya." + name, "arguments": arguments})

        replies = self.rpc([
            ("initialize", {"config": {"port": self.port}}),
            call("eval_python", python="import sys, json\nprint('中文')\nprint('warning', file=sys.stderr)"),
            call("eval_python", python="json.loads('[1,2]')"),
            call("python_paths", module="json"),
            call("python_paths", module="rah_missing_module"),
            call("python_paths"),
            call("eval_mel", mel='string $rahVersion = `about -version`; print($rahVersion);'),
            call("eval_mel", mel='about -version;'),
            call("eval_python", python="print('before failure')\nraise ValueError('test error')"),
            call("eval_mel", mel='string $rahText = "' + 'a' * 3000 + '"; size($rahText);'),
        ])
        payloads = [reply["result"]["content"][0]["json"] for reply in replies[1:8]]
        streams, expression, paths, missing, all_paths, printed, version = payloads
        self.assertEqual(streams["stdout"], "中文\n")
        self.assertEqual(streams["stderr"], "warning\n")
        self.assertEqual(expression["result"], [1, 2])
        self.assertEqual(paths["result"]["file"], json.__file__)
        self.assertFalse(missing["result"]["loaded"])
        self.assertEqual(all_paths["result"]["sys_path"], sys.path)
        self.assertIn(cmds.about(version=True), printed["stdout"])
        self.assertEqual(version["result"], cmds.about(version=True))
        self.assertIn("before failure", replies[8]["error"]["message"])
        self.assertIn("ValueError: test error", replies[8]["error"]["message"])
        self.assertEqual(replies[9]["result"]["content"][0]["json"]["result"], 3000)

    def test_multiline_python(self):
        # Include poorly compressible data so this valid program crosses the
        # port's 4096-byte receive buffer even after compression.
        rng = random.Random(42)
        token = "".join(chr(rng.randrange(0x4e00, 0x9fff)) for _ in range(3500))
        source = "\r\n".join([
            "def rah_multiline():",
            "    path = r'C:\\assets\\rig'",
            "    return path, " + repr(token),
            "",
            "for item in rah_multiline():",
            "    print(item)",
        ])
        for program, expected in [('print("hello test")', "hello test\n"),
                                  (source.replace(repr(token), repr("中文")), "C:\\assets\\rig\n中文\n"),
                                  (source, "C:\\assets\\rig\n" + token + "\n")]:
            with self.subTest(length=len(program)):
                reply = self.rpc([
                    ("initialize", {"config": {"port": self.port}}),
                    ("tools/call", {"name": "maya.eval_python", "arguments": {"python": program}}),
                ])[1]
                self.assertNotIn("error", reply)
                self.assertEqual(reply["result"]["content"][0]["json"]["stdout"], expected)

    def test_language_hints(self):
        for name, field, source, hint in [
            ("eval_python", "python", "```python\nprint(1)\n```", "Remove the Markdown fences"),
            ("eval_mel", "mel", "```mel\nls;\n```", "Remove the Markdown fences"),
            ("eval_mel", "mel", "cmds.ls(selection=True)", "maya.eval_python"),
            ("eval_mel", "mel", "puts hello", "print"),
        ]:
            with self.subTest(source=source), self.assertRaisesRegex(rah_maya.InvalidInput, hint):
                rah_maya.call_maya("maya." + name, {field: source}, None)

        for source, hint in [("ls -selection;", "maya.eval_mel"),
                             (r"import sys\nprint(sys.version)", "actual newlines")]:
            reply = self.rpc([
                ("initialize", {"config": {"port": self.port}}),
                ("tools/call", {"name": "maya.eval_python", "arguments": {"python": source}}),
            ])[1]
            self.assertIn(hint, reply["error"]["message"])
            self.assertIn("Traceback", reply["error"]["message"])

    def test_plugin_guide(self):
        with patch.object(rah_maya, "desktop_port", side_effect=AssertionError):
            guide = rah_maya.call_maya("maya.plugin_guide", {}, None)["content"][0]["json"]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory, guide["filename"])
            path.write_text(guide["python_source"], encoding="utf-8")
            test_scope = {}
            exec(guide["test_python"].replace("r'C:/work/rah_hello.py'", repr(str(path))), test_scope)
            plugin = cmds.loadPlugin(str(path))[0]
            try:
                self.assertEqual(cmds.rahHello(), guide["expected"])
                with self.assertRaisesRegex(RuntimeError, "already loaded"):
                    test_scope["test_plugin"](str(path))
                replies = self.rpc([
                    ("initialize", {"config": {"port": self.port}}),
                    ("tools/call", {"name": "maya.plugin_status"}),
                    ("tools/call", {"name": "maya.plugin_status", "arguments": {"plugin": str(path)}}),
                    ("tools/call", {"name": "maya.plugin_status", "arguments": {"plugin": "rahMissingPlugin"}}),
                ])
                loaded, info, missing = [reply["result"]["content"][0]["json"]["result"] for reply in replies[1:4]]
                self.assertIn(plugin, loaded)
                self.assertTrue(info["loaded"])
                self.assertEqual(Path(info["path"]), path)
                self.assertEqual(info["commands"], ["rahHello"])
                self.assertFalse(missing["loaded"])
            finally:
                cmds.unloadPlugin(plugin)

    def test_standalone_guide(self):
        guide = rah_maya.plugin_guide("standalone")
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory, "rah_hello.py")
            runner = Path(directory, guide["filename"])
            path.write_text(rah_maya.PLUGIN_SOURCE, encoding="utf-8")
            runner.write_text(guide["python_source"], encoding="utf-8")
            argv = [sys.executable, "-B", str(runner), str(path)]
            passed = subprocess.run(argv, capture_output=True, text=True, timeout=60)
            self.assertEqual(passed.returncode, 0, passed.stdout + passed.stderr)
            self.assertIn("PASS Maya " + cmds.about(version=True), passed.stdout)
            path.write_text(rah_maya.PLUGIN_SOURCE.replace("Hello from Maya", "wrong result"), encoding="utf-8")
            failed = subprocess.run(argv, capture_output=True, text=True, timeout=60)
            self.assertNotEqual(failed.returncode, 0)
            self.assertIn("Unexpected rahHello result", failed.stderr)

    def test_invalid_probes_rejected_before_connection(self):
        for script in ("puts hello", "import sys; print(sys.version);", "from maya import cmds",
                       "about -qtversion;", "about -productVersion;"):
            with self.subTest(script=script), self.assertRaises(rah_maya.InvalidInput):
                rah_maya.call_maya("maya.eval_mel", {"mel": script}, None)


if __name__ == "__main__":
    unittest.main()
