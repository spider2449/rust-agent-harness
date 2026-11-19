"""Run with Maya 2025 mayapy. Uses a fresh scene and a loopback port."""
import json
import os
import socket
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch
import socketserver
import rah_maya
import maya.utils

import maya.standalone
from maya import cmds
from PySide6.QtCore import QCoreApplication

PLUGIN = Path(__file__).with_name("rah_maya.py").resolve()


class DesktopRouting(unittest.TestCase):
    def test_reuses_registered_port(self):
        with patch.object(rah_maya, "registered_port", return_value=50010), patch.object(os, "startfile") as launch:
            self.assertEqual(rah_maya.desktop_port(), 50010)
            launch.assert_not_called()

    def test_waits_for_startup_and_only_launches_if_missing(self):
        for running in (False, True):
            with self.subTest(running=running), patch.object(rah_maya, "registered_port", side_effect=[None, 50010]), \
                    patch.object(rah_maya, "maya_running", return_value=running), \
                    patch.object(os, "startfile") as launch, patch.object(time, "sleep"):
                self.assertEqual(rah_maya.desktop_port(), 50010)
                self.assertEqual(launch.call_count, int(not running))


class MayaIntegration(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        with socket.socket() as probe:
            probe.bind(("127.0.0.1", 0))
            cls.port = probe.getsockname()[1]
        cls.address = f"127.0.0.1:{cls.port}"
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

    def test_plugin_guide(self):
        with patch.object(rah_maya, "desktop_port", side_effect=AssertionError):
            guide = rah_maya.call_maya("maya.plugin_guide", {}, None)["content"][0]["json"]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory, guide["filename"])
            path.write_text(guide["python_source"], encoding="utf-8")
            plugin = cmds.loadPlugin(str(path))[0]
            try:
                self.assertEqual(cmds.rahHello(), guide["expected"])
            finally:
                cmds.unloadPlugin(plugin)

    def test_invalid_probes_rejected_before_connection(self):
        for script in ("puts hello", "import sys; print(sys.version);", "from maya import cmds",
                       "about -qtversion;", "about -productVersion;"):
            with self.subTest(script=script), self.assertRaises(rah_maya.InvalidInput):
                rah_maya.call_maya("maya.eval_mel", {"mel": script}, None)


if __name__ == "__main__":
    unittest.main()
