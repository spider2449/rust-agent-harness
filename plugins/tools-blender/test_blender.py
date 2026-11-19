"""Run with the Blender installation's bundled python.exe, from any directory."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

import rah_blender

BRIDGE = Path(__file__).with_name("scene_bridge.py").read_text(encoding="utf-8")
BLENDER = Path(sys.executable).parents[3] / "blender.exe"


class BlenderIntegration(unittest.TestCase):
    def test_authoring_without_desktop(self):
        with tempfile.TemporaryDirectory() as directory, patch.dict(os.environ, LOCALAPPDATA=directory), \
                patch.object(os, "startfile", side_effect=AssertionError):
            session = rah_blender.call(BRIDGE, "blender.session", {})["content"][0]["json"]
            self.assertFalse(session["connected"])
            guide = rah_blender.call(BRIDGE, "blender.addon_guide", {})["content"][0]["json"]
            script = Path(directory, guide["filename"])
            script.write_text(guide["python_source"], encoding="utf-8")
            check = "import bpy; assert bpy.ops.rah.hello() == {'FINISHED'}; " + guide["cleanup"]
            subprocess.run([BLENDER, "--background", "--factory-startup", "--python-exit-code", "1",
                            "--python", str(script), "--python-expr", check], timeout=30, check=True)

    def test_rpc_in_a_fresh_scene(self):
        with tempfile.TemporaryDirectory() as directory:
            env = dict(os.environ, LOCALAPPDATA=directory)
            root = Path(directory)
            # In background mode Blender does not run its UI timer loop.
            # Drive the same callback explicitly for this isolated scene.
            pump = (
                "import os, sys, time\nfrom pathlib import Path\n"
                "deadline = time.monotonic() + 45\n"
                "while time.monotonic() < deadline and not "
                "(Path(os.environ['LOCALAPPDATA']) / 'stop').exists():\n"
                "    time.sleep(sys._rah_blender_timer())\n"
            )
            with subprocess.Popen([BLENDER, "--background", "--factory-startup", "--python-exit-code",
                                   "1", "--python-expr", "exec(" + ascii(BRIDGE) + ")",
                                   "--python-expr", pump], env=env, stdout=subprocess.PIPE,
                                  stderr=subprocess.STDOUT) as blender:
                try:
                    for _ in range(400):
                        if list(root.glob("rah/blender/scene-*/ready")) or blender.poll() is not None:
                            break
                        time.sleep(0.05)
                    self.assertIsNone(blender.poll(), blender.stdout.read() if blender.poll() is not None else "")
                    self.assertTrue(list(root.glob("rah/blender/scene-*/ready")))
                    def call(tool_name, **args):
                        return "tools/call", {"name": "blender." + tool_name, "arguments": args}

                    messages = [("initialize", {"config": {"bridge": BRIDGE}}),
                                call("create", shape="cube", name="Cube"),
                                call("list", scope="selection"),
                                call("move", name="Cube.001", x=1, y=-2, z=3.5),
                                call("inspect", name="Cube.001"),
                                call("move", name="Cube.001", x=True, y=0, z=0),
                                call("create", shape="unexpected", name="bad"),
                                call("inspect", name="absent"), call("session"), ("shutdown", {})]
                    payload = "".join(json.dumps({"jsonrpc": "2.0", "id": i, "method": method,
                                                  "params": params}) + "\n"
                                      for i, (method, params) in enumerate(messages))
                    adapter = Path(__file__).with_name("rah_blender.py")
                    proc = subprocess.run([sys.executable, "-I", "-B", str(adapter)], input=payload,
                                          capture_output=True, text=True, timeout=20, env=env, check=True)
                    replies = [json.loads(line) for line in proc.stdout.splitlines()]
                    def result(i):
                        return replies[i]["result"]["content"][0]["json"]

                    self.assertIn("blender.addon_guide", [t["name"] for t in replies[0]["result"]["tools"]])
                    self.assertEqual(result(1)["name"], "Cube.001")
                    self.assertEqual(result(2), ["Cube.001"])
                    self.assertEqual(result(3), result(4))
                    self.assertEqual(result(4)["position"], [1, -2, 3.5])
                    for i in (5, 6):
                        self.assertEqual(replies[i]["error"]["data"]["kind"], "invalid_input")
                    self.assertIn("absent", replies[7]["error"]["message"])
                    self.assertTrue(result(8)["connected"])
                    self.assertEqual(Path(result(8)["executable"]), BLENDER)
                    self.assertEqual(result(8)["scene"], "Scene")
                    self.assertEqual(result(8)["file"], "")
                    self.assertEqual(replies[9]["result"], {})
                finally:
                    (root / "stop").touch()
                    try:
                        blender.wait(timeout=8)
                    except subprocess.TimeoutExpired:
                        blender.kill()
                        blender.wait()
                self.assertEqual(blender.returncode, 0, blender.stdout.read())

    def test_lazy_launch_and_reuse(self):
        with tempfile.TemporaryDirectory() as directory, patch.dict(os.environ, LOCALAPPDATA=directory):
            scene = Path(directory) / "scene"
            with patch.object(rah_blender, "active_scene", side_effect=[None, scene, scene]), \
                    patch.object(os, "startfile") as start:
                self.assertEqual(rah_blender.desktop_scene(BRIDGE), scene)
                self.assertEqual(rah_blender.desktop_scene(BRIDGE), scene)
                start.assert_called_once()
                self.assertEqual(start.call_args.args, (str(BLENDER),))


if __name__ == "__main__":
    unittest.main()
