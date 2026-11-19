"""RAH process plugin for the built-in Maya 2025 command port."""

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

LIMIT = 128 * 1024
PRIMITIVES = {"cube": "polyCube", "sphere": "polySphere", "cylinder": "polyCylinder",
              "plane": "polyPlane", "cone": "polyCone"}
IDENT = r"[A-Za-z_][A-Za-z_0-9]*"
NODE = rf"\|?(?::?{IDENT}(?::{IDENT})*)(?:\|:?{IDENT}(?::{IDENT})*)*"


def tool(tool_name, description, permission, **fields):
    return {"name": "maya." + tool_name, "description": description, "permission": permission,
            "input_schema": {"type": "object", "properties": fields, "required": list(fields),
                             "additionalProperties": False}}


TOOLS = [
    tool("locate", "Find the installed Maya executable on this Windows machine before launch.", "read"),
    tool("plugin_guide", "Start here to write a Maya Python API 2.0 plug-in. Returns a working command "
         "plug-in and Python testing instructions locally, without connecting to Maya.", "read"),
    tool("session", "Get the connected Maya executable, Maya/API/Python/Qt versions and scene in one call. "
         "Use when the task needs the running environment.", "read"),
    tool("list", "List transform paths. Choose scene or selection.", "read",
         scope={"type": "string", "enum": ["scene", "selection"]}),
    tool("create", "Create one polygon primitive. Use the first returned name for later calls.", "write",
         shape={"type": "string", "enum": list(PRIMITIVES)}, name={"type": "string", "pattern": "^" + IDENT + "$"}),
    tool("move", "Set one transform's absolute world position in scene units.", "write",
         node={"type": "string"}, x={"type": "number"}, y={"type": "number"}, z={"type": "number"}),
    tool("help", "Look up Maya's MEL flags before writing a command, e.g. polyCube.", "read",
         command={"type": "string", "pattern": "^" + IDENT + "$"}),
    tool("eval_mel", "Execute MEL only. For Python plug-in development use maya.plugin_guide. "
         "Prefer create/move/list. Use MEL command syntax, "
         'e.g. xform -q -ws -t "box";. Inspect the scene after errors before trying again.', "execute",
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


def compile_mel(name, args):
    spec = next((t for t in TOOLS if t["name"] == name), None)
    if not spec:
        raise InvalidInput("Unknown Maya tool")
    fields = spec["input_schema"]["properties"]
    if not isinstance(args, dict) or args.keys() != fields.keys():
        raise InvalidInput("Expected exactly these fields: " + ", ".join(fields))
    for key, schema in fields.items():
        value = args[key]
        if schema["type"] == "number":
            if type(value) not in (float, int) or abs(value) > sys.float_info.max or not math.isfinite(value):
                raise InvalidInput(key + " must be a finite number")
        elif not isinstance(value, str) or not value.strip() or len(value) > schema.get("maxLength", 1024):
            raise InvalidInput(key + " must be a nonempty string within the length limit")
        if "pattern" in schema and not re.fullmatch(IDENT, value):
            raise InvalidInput(key + " must be an identifier, e.g. box1")
        if "enum" in schema and value not in schema["enum"]:
            raise InvalidInput(key + " must be one of: " + ", ".join(schema["enum"]))
    if name == "maya.list":
        return "ls -long -transforms" + (" -selection" if args["scope"] == "selection" else "") + ";"
    if name == "maya.create":
        return f'{PRIMITIVES[args["shape"]]} -name "{args["name"]}";'
    if name == "maya.move":
        if not re.fullmatch(NODE, args["node"]):
            raise InvalidInput("node must be a transform name or DAG path, e.g. |group1|box")
        return 'xform -absolute -worldSpace -translation {x} {y} {z} "{node}";'.format(**args)
    if name == "maya.help":
        return f'help "{args["command"]}";'
    if name == "maya.session":
        expr = ("__import__('json').dumps(dict(executable=__import__('sys').executable, "
                "maya=cmds.about(version=True), api=cmds.about(apiVersion=True), "
                "python=__import__('sys').version.split()[0], "
                "qt=__import__('PySide6.QtCore', fromlist=['qVersion']).qVersion(), "
                "scene=cmds.file(query=True, sceneName=True)))")
        expr = "(lambda cmds: " + expr + ")(__import__('maya.cmds', fromlist=['about']))"
        return "python(" + json.dumps(expr) + ");"
    if name in ("maya.locate", "maya.plugin_guide"):
        return None
    if re.match(r"\s*(?:import\s|from\s+\w+[.\w]*\s+import\s|puts\s)", args["mel"]):
        raise InvalidInput("maya.eval_mel accepts MEL. Use maya.plugin_guide for Python plug-ins "
                           "or maya.session for environment details.")
    if re.fullmatch(r"\s*about\s+-(?:qtversion|productVersion)\s*;?\s*", args["mel"]):
        raise InvalidInput("Use maya.session {} for the installed Maya and Qt versions.")
    return args["mel"]


def packet(value):
    return (json.dumps(value, allow_nan=False) + "\n").encode("ascii")


def registered_port():
    # Maya 2025 ServerRegistryMMap stores the bitmap and 34 network-order
    # (108-byte name, u16 port) records in this Windows session mapping.
    record = struct.Struct("!108sH")
    with mmap.mmap(-1, 4 + 34 * record.size, tagname="Maya_Unix_Socket_Share",
                   access=mmap.ACCESS_READ) as mapping:
        bitmap, = struct.unpack_from("=I", mapping)
        for index in range(32):
            if bitmap & (1 << index):
                name, port = record.unpack_from(mapping, 4 + index * record.size)
                if name.rstrip(b"\0") == b"commandportDefault" and port:
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
        return
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
            for version in sorted((v for v in versions if v.isdecimal() and int(v) >= 2025),
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
    for candidate in executable_candidates():
        if candidate.is_absolute() and candidate.is_file():
            return {"executable": str(candidate)}
    return {"executable": None, "instructions": "Open Maya manually or set RAH_MAYA_EXE "
            "to its absolute executable path before starting RAH."}


def desktop_port():
    port = registered_port()
    if port is not None:
        return port
    if not maya_running():
        # Open a user-owned GUI document application. Leaving RAH must not
        # terminate Maya or discard its unsaved scene.
        try:
            installation = locate()
            if not installation["executable"]:
                raise NotStarted(installation["instructions"])
            os.startfile(installation["executable"])
        except OSError as exc:
            raise NotStarted(f"Could not open Maya 2025: {exc}") from exc
    deadline = time.monotonic() + 90
    while time.monotonic() < deadline:
        time.sleep(0.5)
        port = registered_port()
        if port is not None:
            return port
    raise NotStarted("Maya has not registered commandportDefault. Check Maya's Applications "
                     "preferences for its default command port, then restart Maya.")


def call_maya(name, args, port):
    script = compile_mel(name, args)
    if name == "maya.locate":
        return {"content": [{"type": "json", "json": locate()}]}
    if name == "maya.plugin_guide":
        return {"content": [{"type": "json", "json": {
            "filename": "rah_hello.py", "python_source": PLUGIN_SOURCE,
            "instructions": "Save the source as rah_hello.py. In Maya Script Editor's Python tab, "
                            "replace path with the saved absolute path and run the example. "
                            "Unload before editing and loading the plug-in again.",
            "test_python": "import maya.cmds as cmds\npath = r'C:/work/rah_hello.py'\n"
                           "plugin = cmds.loadPlugin(path)[0]\nprint(cmds.rahHello())\ncmds.unloadPlugin(plugin)",
            "expected": ["Hello from Maya"],
        }}]}
    # Use the built-in MEL endpoint. JSON preserves arrays, scalars and null,
    # and an interpreter error stays distinguishable from a successful value.
    expr = ("__import__('json').dumps({'value': "
            "__import__('maya.mel', fromlist=['eval']).eval(" + ascii(script) + ")}, allow_nan=False)")
    request = "python(" + json.dumps(expr) + ");\n"
    if len(request) > 4095:
        raise InvalidInput("MEL exceeds Maya's command port buffer; split it into smaller commands")
    if port is None:
        port = desktop_port()
    try:
        conn = socket.create_connection(("127.0.0.1", port), timeout=2)
    except OSError as exc:
        raise NotStarted(f"Maya is unavailable on 127.0.0.1:{port}: {exc}") from exc
    with conn:
        conn.sendall(request.encode("ascii"))
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
        value = json.loads(body)["value"]
    except (ValueError, TypeError, KeyError) as exc:
        raise RuntimeError("Maya command port: " + body[:4096]) from exc
    if name == "maya.session":
        value = {**json.loads(value), "host": "127.0.0.1", "port": port}
    return {"content": [{"type": "json", "json": {"mel": script, "result": value}}]}


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
