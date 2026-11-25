"""RAH process plugin for the Windows Maya 2023+ command port."""

import base64
import ctypes
import json
import math
import mmap
import os
from pathlib import Path
import re
import socket
import struct
import sys
import time
import zlib

LIMIT = 128 * 1024
PRIMITIVES = {"cube": "polyCube", "sphere": "polySphere", "cylinder": "polyCylinder",
              "plane": "polyPlane", "cone": "polyCone"}
IDENT = r"[A-Za-z_][A-Za-z_0-9]*"
NODE = rf"\|?(?::?{IDENT}(?::{IDENT})*)(?:\|:?{IDENT}(?::{IDENT})*)*"
MIN_MAYA_VERSION = 2023
DEFAULT_PORT_NAME = "commandportDefault"
STRUCTURED_TOOLS = {"maya.inspect", "maya.select", "maya.plugin_status"}
TRANSFORM_FLAGS = {
    "maya.move": "-worldSpace -translation",
    "maya.rotate": "-worldSpace -rotation",
    "maya.scale": "-objectSpace -scale",
}
TRANSFORM_FIELDS = {
    "node": {"type": "string"},
    **{axis: {"type": "number"} for axis in ("x", "y", "z")},
}


def tool(tool_name, description, permission, *, required=None, **fields):
    return {"name": "maya." + tool_name, "description": description, "permission": permission,
            "input_schema": {"type": "object", "properties": fields,
                             "required": list(fields) if required is None else required,
                             "additionalProperties": False}}


TOOLS = [
    tool("locate", "Find Maya 2023+ installations and their mayapy executables without launching Maya.", "read"),
    tool("plugin_guide", "Start here to write or test a Maya plug-in. Omit topic for a Python API 2.0 "
         "command example and GUI test. Use topic=standalone for an isolated mayapy test script.", "read",
         required=[], topic={"type": "string", "enum": ["command", "standalone"]}),
    tool("session", "Get the connected Maya executable, Maya/API/Python/Qt versions and scene in one call. "
         "Use when the task needs the running environment.", "read"),
    tool("list", "List full transform paths. Use selection for selected transforms or scene to find a node.", "read",
         scope={"type": "string", "enum": ["scene", "selection"]}),
    tool("inspect", "Verify a transform by name or full path from maya.list. Returns world position, "
         "world rotation, local scale, and shapes.", "read", node={"type": "string"}),
    tool("select", "Replace the selection with one transform. Pass the full path when names repeat.", "write",
         node={"type": "string"}),
    tool("plugin_status", "List loaded Maya plug-ins. Pass a Maya plug-in name or absolute file path "
         "to see its loaded status, commands, and node types.", "read", required=[], plugin={"type": "string"}),
    tool("create", "Create one polygon primitive. Use the first returned name for later calls.", "write",
         shape={"type": "string", "enum": list(PRIMITIVES)}, name={"type": "string", "pattern": "^" + IDENT + "$"}),
    tool("move", "Set one transform's absolute world position in scene units.", "write", **TRANSFORM_FIELDS),
    tool("rotate", "Set one transform's absolute world rotation in the scene's angle units.",
         "write", **TRANSFORM_FIELDS),
    tool("scale", "Set one transform's absolute local scale factors. Use 1 to keep an axis at default size.",
         "write", **TRANSFORM_FIELDS),
    tool("help", "Look up Maya's MEL flags before writing a command, e.g. polyCube.", "read",
         command={"type": "string", "pattern": "^" + IDENT + "$"}),
    tool("python_paths", "Return Maya's sys.path and __file__ for an already imported module. "
         "Omit module to list only sys.path.", "read", required=[], module={"type": "string"}),
    tool("eval_python", "Use for Python imports and scripts in Maya. Prefer maya.list/create/move for "
         "basic scene tasks and maya.inspect to verify changes. Pass plain Python with normal newlines and indentation. "
         "Example: import maya.cmds as cmds\nprint(cmds.ls(selection=True))\n"
         "Read stdout for print output, result for a single expression. Skip Markdown fences.", "execute",
         python={"type": "string", "minLength": 1, "maxLength": 8192}),
    tool("eval_mel", "Use for MEL commands. Prefer named Maya tools for basic scene tasks. "
         "Check flags with maya.help. Pass plain MEL in the mel field. Example: ls -selection; "
         "Read result for the command value, stdout for print output. Send Python imports to "
         "maya.eval_python. Inspect the scene after execution errors before trying again.", "execute",
         mel={"type": "string", "minLength": 1, "maxLength": 8192}),
]


class InvalidInput(ValueError):
    pass


class NotStarted(Exception):
    pass


PLUGIN_SOURCE = '''import maya.api.OpenMaya as om


def maya_useNewAPI():
    pass


class RahHello(om.MPxCommand):
    def doIt(self, args):
        self.setResult("Hello from Maya")


def initializePlugin(plugin):
    om.MFnPlugin(plugin, "Example", "1.0", "Any").registerCommand("rahHello", RahHello)


def uninitializePlugin(plugin):
    om.MFnPlugin(plugin).deregisterCommand("rahHello")
'''

PLUGIN_TEST = '''def test_plugin(path):
    from pathlib import Path
    import maya.cmds as cmds

    before = cmds.pluginInfo(query=True, listPlugins=True) or []
    source = Path(path).resolve()
    loaded_paths = cmds.pluginInfo(query=True, listPluginsPath=True) or []
    if any(Path(loaded_path).resolve() == source for loaded_path in loaded_paths):
        raise RuntimeError(f"{path} was already loaded. Use a fresh Maya session for this test.")
    loaded = cmds.loadPlugin(path) or []
    if not loaded:
        raise RuntimeError(f"Maya did not load {path}. Check for an already loaded plug-in with the same name.")
    plugin = loaded[0]
    if plugin in before:
        raise RuntimeError(f"{plugin} was already loaded. Use a fresh Maya session for this test.")
    try:
        loaded_path = cmds.pluginInfo(plugin, query=True, path=True)
        if Path(loaded_path).resolve() != source:
            raise AssertionError(f"Expected to load {path}, got {loaded_path}")
        commands = cmds.pluginInfo(plugin, query=True, command=True) or []
        if "rahHello" not in commands:
            raise AssertionError(f"Expected rahHello in registered commands, got {commands!r}")
        actual = cmds.rahHello()
        if actual != ["Hello from Maya"]:
            raise AssertionError(f"Unexpected rahHello result: {actual!r}")
    finally:
        cmds.unloadPlugin(plugin)
    print(f"PASS Maya {cmds.about(version=True)}: {plugin}")
'''

STANDALONE_MAIN = '''

if __name__ == "__main__":
    import argparse
    from pathlib import Path
    import maya.standalone

    parser = argparse.ArgumentParser(description="Smoke test a Maya command plug-in")
    parser.add_argument("plugin", type=Path)
    args = parser.parse_args()
    if not args.plugin.is_absolute() or not args.plugin.is_file():
        parser.error("plugin must be an existing absolute file path")
    maya.standalone.initialize()
    try:
        test_plugin(str(args.plugin))
    finally:
        maya.standalone.uninitialize()
'''


def plugin_guide(topic):
    if topic == "standalone":
        return {
            "filename": "test_maya_plugin.py",
            "python_source": PLUGIN_TEST + STANDALONE_MAIN,
            "steps": [
                "Save python_source as test_maya_plugin.py. Adapt test_plugin to check your actual "
                "commands or node behavior and clean up test nodes before unloading.",
                "Use maya.locate to get the mayapy path for each target version. Run the script "
                "with that mayapy and pass the plug-in's absolute path.",
                "Check the exit code and PASS line for each version. Failed checks raise a traceback. "
                "Use a disposable Maya GUI scene for UI and viewport tests.",
            ],
            "powershell_example": (
                "& 'C:/aw/Maya2023/bin/mayapy.exe' -B 'C:/work/test_maya_plugin.py' 'C:/work/rah_hello.py'"),
        }
    return {
        "filename": "rah_hello.py",
        "python_source": PLUGIN_SOURCE,
        "instructions": "Save the source as rah_hello.py. Open a disposable Maya scene and run test_python "
                        "in Script Editor's Python tab after replacing the absolute path. "
                        "Unload before editing and loading again. Use maya.plugin_status to check "
                        "plug-ins and maya.python_paths to diagnose imported modules. "
                        "For a headless test, request maya.plugin_guide with topic=standalone.",
        "test_python": PLUGIN_TEST + "\ntest_plugin(r'C:/work/rah_hello.py')\n",
        "expected": ["Hello from Maya"],
    }


def compile_mel(name, args):
    spec = next((t for t in TOOLS if t["name"] == name), None)
    if not spec:
        raise InvalidInput("Unknown Maya tool")
    fields = spec["input_schema"]["properties"]
    if (not isinstance(args, dict) or not args.keys() <= fields.keys()
            or not set(spec["input_schema"]["required"]) <= args.keys()):
        raise InvalidInput("Expected exactly these fields: " + ", ".join(fields))
    for key, value in args.items():
        schema = fields[key]
        if schema["type"] == "number":
            if type(value) not in (float, int) or abs(value) > sys.float_info.max or not math.isfinite(value):
                raise InvalidInput(key + " must be a finite number")
        elif not isinstance(value, str) or not value.strip() or len(value) > schema.get("maxLength", 1024):
            raise InvalidInput(key + " must be a nonempty string within the length limit")
        if "pattern" in schema and not re.fullmatch(IDENT, value):
            raise InvalidInput(key + " must be an identifier, e.g. box1")
        if "enum" in schema and value not in schema["enum"]:
            raise InvalidInput(key + " must be one of: " + ", ".join(schema["enum"]))
    if name in ("maya.eval_mel", "maya.eval_python"):
        language = name.removeprefix("maya.eval_")
        if args[language].lstrip().startswith("```"):
            raise InvalidInput(f"Remove the Markdown fences and send the code in the {language} field.")
    if name == "maya.list":
        return "ls -long -transforms" + (" -selection" if args["scope"] == "selection" else "") + ";"
    if name == "maya.create":
        return f'{PRIMITIVES[args["shape"]]} -name "{args["name"]}";'
    if name in TRANSFORM_FLAGS or name in {"maya.inspect", "maya.select"}:
        if not re.fullmatch(NODE, args["node"]):
            raise InvalidInput("node must be a transform name or DAG path, e.g. |group1|box")
    if name in TRANSFORM_FLAGS:
        return 'xform -absolute {flags} {x} {y} {z} "{node}";'.format(flags=TRANSFORM_FLAGS[name], **args)
    if name == "maya.help":
        return f'help "{args["command"]}";'
    if name == "maya.session":
        expr = ("__import__('json').dumps(dict(executable=__import__('sys').executable, "
                "maya=cmds.about(version=True), api=cmds.about(apiVersion=True), "
                "python=__import__('sys').version.split()[0], "
                "qt=cmds.about(qtVersion=True), "
                "scene=cmds.file(query=True, sceneName=True)))")
        expr = "(lambda cmds: " + expr + ")(__import__('maya.cmds', fromlist=['about']))"
        return "python(" + json.dumps(expr) + ");"
    if name in STRUCTURED_TOOLS or name in ("maya.locate", "maya.plugin_guide", "maya.eval_python",
                                            "maya.python_paths"):
        return None
    if re.match(r"\s*(?:import\s|from\s+\w+[.\w]*\s+import\s|cmds\.\w+\s*\()", args["mel"]):
        raise InvalidInput("maya.eval_mel accepts MEL. Use maya.eval_python for Python code "
                           "or maya.session for environment details.")
    if re.match(r"\s*puts\s", args["mel"]):
        raise InvalidInput('Use MEL print("hello"); to write to stdout.')
    if re.fullmatch(r"\s*about\s+-(?:qtversion|productVersion)\s*;?\s*", args["mel"]):
        raise InvalidInput("Use maya.session {} for the installed Maya and Qt versions.")
    return args["mel"]


def packet(value):
    return (json.dumps(value, allow_nan=False) + "\n").encode("ascii")


def command_port_name():
    name = os.environ.get("RAH_MAYA_PORT_NAME", DEFAULT_PORT_NAME)
    if not re.fullmatch(r"[A-Za-z_][A-Za-z_0-9-]{0,106}", name):
        raise InvalidInput("RAH_MAYA_PORT_NAME must be a Maya command port name, e.g. rahMaya2023")
    return name


def registered_port():
    # Maya 2023 and 2025 ServerRegistryMMap store the bitmap and 34 network-order
    # (108-byte name, u16 port) records in this Windows session mapping.
    target = command_port_name().encode("ascii")
    record = struct.Struct("!108sH")
    with mmap.mmap(-1, 4 + 34 * record.size, tagname="Maya_Unix_Socket_Share",
                   access=mmap.ACCESS_READ) as mapping:
        bitmap, = struct.unpack_from("=I", mapping)
        for index in range(32):
            if bitmap & (1 << index):
                name, port = record.unpack_from(mapping, 4 + index * record.size)
                if name.rstrip(b"\0") == target and port:
                    return port
    return None


def maya_running():
    from ctypes import wintypes

    class ProcessInfo(ctypes.Structure):
        _fields_ = [("session", wintypes.DWORD), ("pid", wintypes.DWORD),
                    ("name", wintypes.LPWSTR), ("sid", ctypes.c_void_p)]

    session = wintypes.DWORD()
    if not ctypes.windll.kernel32.ProcessIdToSessionId(os.getpid(), ctypes.byref(session)):
        raise NotStarted("Cannot identify the Windows session")
    processes = ctypes.POINTER(ProcessInfo)()
    count = wintypes.DWORD()
    wts = ctypes.windll.wtsapi32
    if not wts.WTSEnumerateProcessesW(None, 0, 1, ctypes.byref(processes), ctypes.byref(count)):
        raise NotStarted("Cannot check whether Maya is running")
    try:
        return any(p.session == session.value and (p.name or "").lower() == "maya.exe"
                   for p in processes[:count.value])
    finally:
        wts.WTSFreeMemory(processes)


def executable_candidates():
    override = os.environ.get("RAH_MAYA_EXE")
    if override:
        yield Path(override)
    for directory in os.get_exec_path():
        if Path(directory).is_absolute():
            yield Path(directory) / "maya.exe"
    location = os.environ.get("MAYA_LOCATION")
    if location:
        yield Path(location) / "bin" / "maya.exe"
    import winreg
    try:
        with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, r"SOFTWARE\Autodesk\Maya",
                            access=winreg.KEY_READ | winreg.KEY_WOW64_64KEY) as key:
            versions = [winreg.EnumKey(key, i) for i in range(winreg.QueryInfoKey(key)[0])]
            for version in sorted((v for v in versions if re.fullmatch(r"[0-9]{4}", v)
                                   and int(v) >= MIN_MAYA_VERSION),
                                  key=int, reverse=True):
                try:
                    with winreg.OpenKey(key, version + r"\Setup\InstallPath") as install:
                        location, _ = winreg.QueryValueEx(install, "MAYA_INSTALL_LOCATION")
                        yield Path(location) / "bin" / "maya.exe"
                except OSError:
                    continue
    except OSError:
        pass


def locate():
    installations = []
    seen = set()
    for candidate in executable_candidates():
        if not candidate.is_absolute() or not candidate.is_file():
            continue
        key = os.path.normcase(str(candidate.resolve()))
        if key in seen:
            continue
        seen.add(key)
        mayapy = candidate.with_name("mayapy.exe")
        installations.append({"executable": str(candidate), "mayapy": str(mayapy) if mayapy.is_file() else None})
    override = os.environ.get("RAH_MAYA_EXE")
    executable = installations[0]["executable"] if installations else None
    if override and (not executable or Path(override) != Path(executable)):
        executable = None
    result = {"executable": executable, "installations": installations, "port_name": command_port_name()}
    if executable is None:
        result["instructions"] = ("Open Maya manually or set RAH_MAYA_EXE to its absolute executable "
                                  "path before starting RAH.")
    return result


def desktop_port():
    port = registered_port()
    if port is not None:
        return port
    name = command_port_name()
    if name != DEFAULT_PORT_NAME:
        raise NotStarted(f"Open the named MEL command port {name} in the intended Maya session first.")
    if not maya_running():
        # Open a user-owned GUI document application. Leaving RAH must not
        # terminate Maya or discard its unsaved scene.
        try:
            installation = locate()
            if not installation["executable"]:
                raise NotStarted(installation["instructions"])
            os.startfile(installation["executable"])
        except OSError as exc:
            raise NotStarted(f"Could not open Maya: {exc}") from exc
    deadline = time.monotonic() + 90
    while time.monotonic() < deadline:
        time.sleep(0.5)
        port = registered_port()
        if port is not None:
            return port
    raise NotStarted(f"Maya has not registered {name}. Check Maya's Applications preferences "
                     "for its default command port.")


# This function runs inside Maya. Its source travels with the request because
# the desktop interpreter need not share the plugin's filesystem or sys.path.
EVALUATE = '''def evaluate(language, source):
    import __main__, contextlib, io, json, traceback
    out, err = io.StringIO(), io.StringIO()
    try:
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            if language == "python":
                try:
                    code = compile(source, "<maya.eval_python>", "eval")
                except SyntaxError:
                    code = compile(source, "<maya.eval_python>", "exec")
                value = eval(code, vars(__main__))
            else:
                import maya.mel as mel
                import maya.api.OpenMaya as om
                messages = om.MCommandMessage
                def capture(text, kind, *_):
                    if kind in (messages.kError, messages.kWarning):
                        err.write(text)
                    elif kind in (messages.kDisplay, messages.kInfo):
                        out.write(text)
                callback = messages.addCommandOutputCallback(capture)
                try:
                    value = mel.eval(source)
                finally:
                    om.MMessage.removeCallback(callback)
        return json.dumps(dict(value=value, stdout=out.getvalue(), stderr=err.getvalue()),
                          default=repr, allow_nan=False)
    except BaseException:
        return json.dumps(dict(error=traceback.format_exc(), stdout=out.getvalue(), stderr=err.getvalue()))
'''

STRUCTURED = '''def structured(action, args):
    from maya import cmds

    if action == "maya.plugin_status":
        from os.path import normpath
        from pathlib import Path

        plugin = args.get("plugin")
        loaded = sorted(cmds.pluginInfo(query=True, listPlugins=True) or [])
        if plugin is None:
            return loaded
        path = Path(normpath(plugin))
        if path.is_absolute():
            plugin = next((name for name in loaded
                           if Path(normpath(cmds.pluginInfo(name, query=True, path=True))) == path), plugin)
        if plugin not in loaded:
            return dict(plugin=plugin, loaded=False)
        return dict(
            plugin=plugin,
            loaded=True,
            path=cmds.pluginInfo(plugin, query=True, path=True),
            commands=cmds.pluginInfo(plugin, query=True, command=True) or [],
            node_types=cmds.pluginInfo(plugin, query=True, dependNode=True) or [],
        )

    node = args["node"]
    matches = cmds.ls(node, long=True, type="transform") or []
    if len(matches) != 1:
        detail = "No transform found" if not matches else "Ambiguous transform; use a full DAG path"
        raise ValueError(f"{detail}: {node}")
    path = matches[0]
    if action == "maya.select":
        cmds.select(path, replace=True)
        return cmds.ls(selection=True, long=True) or []
    return dict(
        path=path,
        type=cmds.nodeType(path),
        world_position=cmds.xform(path, query=True, worldSpace=True, translation=True),
        world_rotation=cmds.xform(path, query=True, worldSpace=True, rotation=True),
        local_scale=cmds.getAttr(path + ".scale")[0],
        shapes=cmds.listRelatives(path, shapes=True, fullPath=True) or [],
    )


def reply(action, args):
    import json
    try:
        return json.dumps(dict(value=structured(action, args)), allow_nan=False)
    except Exception as exc:
        return json.dumps(dict(error=str(exc), stdout="", stderr=""))
'''


def call_maya(name, args, port):
    script = compile_mel(name, args)
    if name == "maya.locate":
        return {"content": [{"type": "json", "json": locate()}]}
    if name == "maya.plugin_guide":
        return {"content": [{"type": "json", "json": plugin_guide(args.get("topic", "command"))}]}
    # Use the built-in MEL endpoint. JSON preserves arrays, scalars and null,
    # and an interpreter error stays distinguishable from a successful value.
    if name in ("maya.eval_python", "maya.eval_mel"):
        language = "python" if name == "maya.eval_python" else "mel"
        expr = ("(lambda g: (exec(" + ascii(EVALUATE) + ",g),g['evaluate']("
                + repr(language) + "," + ascii(args[language]) + "))[1])({})")
    elif name == "maya.python_paths":
        expr = ("(lambda s,m: __import__('json').dumps({'value':dict(sys_path=s.path, "
                "module=m, loaded=m in s.modules, file=vars(s.modules[m]).get('__file__') "
                "if s.modules.get(m) is not None else None)}))(__import__('sys'),"
                + ascii(args.get("module")) + ")")
    elif name in STRUCTURED_TOOLS:
        expr = ("(lambda g: (exec(" + ascii(STRUCTURED) + ",g),g['reply']("
                + repr(name) + "," + ascii(args) + "))[1])({})")
    else:
        expr = ("__import__('json').dumps({'value': "
                "__import__('maya.mel', fromlist=['eval']).eval(" + ascii(script) + ")}, allow_nan=False)")
    request = "python(" + json.dumps(expr) + ");\n"
    # Preserve room for source that fit the port before output capture added
    # a wrapper. Decide the encoding before sending the command exactly once.
    if len(request) > 4095 and (name in STRUCTURED_TOOLS or name in ("maya.eval_python", "maya.eval_mel")):
        encoded = base64.b64encode(zlib.compress(expr.encode("ascii"))).decode("ascii")
        expr = "eval(__import__('zlib').decompress(__import__('base64').b64decode('" + encoded + "')))"
        request = "python(" + json.dumps(expr) + ");\n"
    if port is None:
        port = desktop_port()
    try:
        conn = socket.create_connection(("127.0.0.1", port), timeout=2)
    except OSError as exc:
        raise NotStarted(f"Maya is unavailable on 127.0.0.1:{port}: {exc}") from exc
    with conn:
        conn.sendall(request.encode("ascii"))
        # Maya accumulates full receive buffers until a short read. EOF also
        # completes a request that occupies an exact multiple of its buffer.
        conn.shutdown(socket.SHUT_WR)
        deadline = time.monotonic() + 45
        data = bytearray()
        while not data.endswith(b"\n\0"):
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError("Maya response timed out; inspect the scene before retrying")
            conn.settimeout(remaining)
            chunk = conn.recv(8192)
            if not chunk:
                raise RuntimeError("Maya disconnected; inspect the scene before retrying")
            data.extend(chunk)
            if len(data) > LIMIT:
                raise RuntimeError("Maya result too large; narrow the query")
    body = data[:-2].decode("utf-8")
    try:
        reply = json.loads(body)
        if "error" in reply:
            details = [reply["error"]]
            if name == "maya.eval_python" and re.match(r"\s*\w+\s+-[a-zA-Z]", args["python"]):
                details.append("If this is MEL command syntax, use maya.eval_mel. "
                               "For Python, use maya.cmds functions with keyword arguments.")
            if (name == "maya.eval_python" and "\\n" in args["python"]
                    and "SyntaxError: unexpected character after line continuation character" in reply["error"]):
                details.append("Separate Python statements with actual newlines in the decoded python field. "
                               "Encode each newline once in JSON.")
            for stream in ("stdout", "stderr"):
                if reply[stream]:
                    details.append(stream + ":\n" + reply[stream])
            raise RuntimeError("\n".join(details))
        value = reply["value"]
    except (ValueError, TypeError, KeyError) as exc:
        raise RuntimeError("Maya command port: " + body[:4096]) from exc
    if name == "maya.session":
        value = {**json.loads(value), "host": "127.0.0.1", "port": port}
    payload = {"result": value}
    if name == "maya.eval_python":
        payload["python"] = args["python"]
    elif script is not None:
        payload["mel"] = script
    if name in ("maya.eval_python", "maya.eval_mel"):
        payload.update(stdout=reply["stdout"], stderr=reply["stderr"])
    return {"content": [{"type": "json", "json": payload}]}


def run():
    port = None
    for line in sys.stdin.buffer:
        msg = json.loads(line)
        if "id" not in msg:
            continue
        method = msg.get("method")
        response = {"jsonrpc": "2.0", "id": msg["id"]}
        try:
            if method == "initialize":
                config = msg.get("params", {}).get("config") or {}
                port = config.get("port")
                if port is not None and (type(port) is not int or not 1 <= port <= 65535):
                    raise InvalidInput("port must be an integer from 1 to 65535")
                response["result"] = {"protocol": 2, "version": "0.1.0", "provides": ["tools"],
                                      "requires": [], "tools": TOOLS}
            elif method == "shutdown":
                response["result"] = {}
            elif method == "tools/call":
                params = msg["params"]
                response["result"] = call_maya(params["name"], params.get("arguments", {}), port)
            else:
                response["error"] = {"code": -32601, "message": "unknown method"}
        except (InvalidInput, NotStarted) as exc:
            kind = "invalid_input" if isinstance(exc, InvalidInput) else "not_started"
            response["error"] = {"code": -32602, "message": str(exc), "data": {"kind": kind}}
        except Exception as exc:
            response["error"] = {"code": -32000, "message": str(exc)}
        sys.stdout.buffer.write(packet(response))
        sys.stdout.buffer.flush()
        if method == "shutdown":
            break


if __name__ == "__main__":
    run()
