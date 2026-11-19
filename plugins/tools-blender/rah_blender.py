"""RAH protocol 2 adapter for the Blender desktop scene bridge (Windows)."""
import errno
import json
import msvcrt
import os
from pathlib import Path
import subprocess
import sys
import time
import uuid


def tool(tool_name, description, permission, **fields):
    return {"name": "blender." + tool_name, "description": description, "permission": permission,
            "input_schema": {"type": "object", "properties": fields, "required": list(fields),
                             "additionalProperties": False}}


NAME = {"type": "string", "minLength": 1, "maxLength": 128}
NUMBER = {"type": "number"}
TOOLS = [
    tool("locate", "Find the installed Blender executable on this Windows machine before launch.", "read"),
    tool("addon_guide", "Start here to write a Blender Python add-on. Returns a working operator and "
         "Text Editor testing instructions without opening Blender or running shell commands.", "read"),
    tool("session", "Check whether a Blender scene is connected to RAH. Returns its executable, "
         "versions and scene when connected, or connection instructions. Never launches Blender.", "read"),
    tool("list", "List object names in the current scene or selection.", "read",
         scope={"type": "string", "enum": ["scene", "selection"]}),
    tool("inspect", "Read an object's type and world position.", "read", name=NAME),
    tool("create", "Create a mesh in Object Mode. Use the returned object name.", "write",
         shape={"type": "string", "enum": ["cube", "sphere", "cylinder", "plane", "cone"]}, name=NAME),
    tool("move", "Set an object's absolute world position in scene units.", "write",
         name=NAME, x=NUMBER, y=NUMBER, z=NUMBER),
]


ADDON_SOURCE = '''bl_info = {
    "name": "RAH Hello",
    "blender": (5, 3, 0),
    "category": "Object",
}

import bpy


class RAH_OT_hello(bpy.types.Operator):
    bl_idname = "rah.hello"
    bl_label = "RAH Hello"

    def execute(self, context):
        self.report({"INFO"}, "Hello from Blender")
        return {"FINISHED"}


def register():
    bpy.utils.register_class(RAH_OT_hello)


def unregister():
    bpy.utils.unregister_class(RAH_OT_hello)


if __name__ == "__main__":
    register()
'''


class InvalidInput(ValueError):
    pass


class NotStarted(Exception):
    pass


def validate(name, arguments):
    spec = next((tool for tool in TOOLS if tool["name"] == name), None)
    if spec is None:
        raise InvalidInput("Unknown Blender tool")
    fields = spec["input_schema"]["properties"]
    if not isinstance(arguments, dict) or arguments.keys() != fields.keys():
        raise InvalidInput("Expected fields: " + ", ".join(fields))
    for key, schema in fields.items():
        value = arguments[key]
        if schema["type"] == "number":
            if type(value) not in (int, float) or not abs(value) <= 3.4028234e38:
                raise InvalidInput(key + " must be a finite Blender coordinate")
        elif not isinstance(value, str) or not value.strip() or len(value) > 128 or "\0" in value:
            raise InvalidInput(key + " must be a nonempty string of at most 128 characters")
        elif "enum" in schema and value not in schema["enum"]:
            raise InvalidInput(key + " must be one of: " + ", ".join(schema["enum"]))


def try_lock(file):
    try:
        msvcrt.locking(file.fileno(), msvcrt.LK_NBLCK, 1)
    except OSError as exc:
        if exc.errno != errno.EACCES:
            raise
        return False
    return True


def active_scene(root):
    scenes = []
    for marker in root.glob("scene-*/ready"):
        try:
            with marker.with_name("active").open("r+b") as lease:
                if not try_lock(lease):
                    scenes.append(marker.parent)
        except FileNotFoundError:
            continue
    if len(scenes) > 1:
        raise NotStarted("Multiple RAH-connected Blender scenes. Close the extra connected windows.")
    return next(iter(scenes), None)


def executable_candidates():
    override = os.environ.get("RAH_BLENDER_EXE")
    if override:
        yield Path(override)
        return
    for directory in os.get_exec_path():
        if Path(directory).is_absolute():
            yield Path(directory) / "blender.exe"
    import winreg
    for hive in (winreg.HKEY_CURRENT_USER, winreg.HKEY_LOCAL_MACHINE):
        try:
            with winreg.OpenKey(hive, r"Software\Microsoft\Windows\CurrentVersion\App Paths\blender.exe") as key:
                executable, _ = winreg.QueryValueEx(key, None)
                yield Path(os.path.expandvars(executable.strip('"')))
        except OSError:
            pass
    programs = os.environ.get("ProgramFiles")
    if programs:
        yield from sorted(Path(programs).glob("Blender Foundation/Blender */blender.exe"), reverse=True)
    # Portable installs may only appear in the current user's launch history.
    store = r"Software\Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Compatibility Assistant\Store"
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, store) as key:
            for index in range(winreg.QueryInfoKey(key)[1]):
                path = Path(winreg.EnumValue(key, index)[0])
                if path.name.lower() == "blender.exe":
                    yield path
    except OSError:
        pass


def locate():
    for candidate in executable_candidates():
        if candidate.is_absolute() and candidate.is_file():
            return {"executable": str(candidate)}
    return {"executable": None, "instructions": "Open Blender manually or set RAH_BLENDER_EXE "
            "to its absolute executable path before starting RAH."}


def desktop_scene(bridge):
    root = Path(os.environ["LOCALAPPDATA"]) / "rah" / "blender"
    root.mkdir(parents=True, exist_ok=True)
    deadline = time.monotonic() + 90
    with (root / "startup.lock").open("a+b") as startup:
        while not try_lock(startup):
            if time.monotonic() >= deadline:
                raise NotStarted("Timed out waiting for Blender startup")
            time.sleep(0.1)
        scene = active_scene(root)
        if scene is None:
            installation = locate()
            if not installation["executable"]:
                raise NotStarted(installation["instructions"])
            arguments = subprocess.list2cmdline(["--python-expr", "exec(" + ascii(bridge) + ")"])
            os.startfile(installation["executable"], arguments=arguments)
        while scene is None:
            if time.monotonic() >= deadline:
                raise NotStarted("Blender did not connect. Check its window and startup dialogs.")
            time.sleep(0.2)
            scene = active_scene(root)
        return scene


def call(bridge, name, arguments):
    validate(name, arguments)
    if name == "blender.locate":
        return {"content": [{"type": "json", "json": locate()}]}
    if name == "blender.addon_guide":
        return {"content": [{"type": "json", "json": {
            "filename": "rah_hello.py", "python_source": ADDON_SOURCE,
            "instructions": "In Blender's Text Editor open the saved file and click Run Script once. "
                            "In the Python Console run bpy.ops.rah.hello(). Before running the script "
                            "again, unregister the operator with the cleanup command.",
            "cleanup": "bpy.utils.unregister_class(bpy.types.RAH_OT_hello)",
        }}]}
    try:
        if name == "blender.session":
            directory = active_scene(Path(os.environ["LOCALAPPDATA"]) / "rah" / "blender")
            if directory is None:
                return {"content": [{"type": "json", "json": {
                    "connected": False, "instructions": "In the existing Blender window open "
                    "plugins/tools-blender/scene_bridge.py in the Text Editor and click Run Script.",
                }}]}
            if not (directory / "session-info").exists():
                raise NotStarted("The connected window uses an older bridge. Restart Blender and "
                                 "run the updated scene_bridge.py in the desired window.")
        else:
            directory = desktop_scene(bridge)
    except OSError as exc:
        raise NotStarted(str(exc)) from exc
    base = directory / uuid.uuid4().hex
    pending, request, reply = (base.with_suffix(ext) for ext in (".in", ".request", ".reply"))
    deadline = time.monotonic() + 45
    try:
        pending.write_text(json.dumps({"name": name, "arguments": arguments, "deadline": deadline}),
                           encoding="ascii")
        pending.replace(request)
        while not reply.is_file():
            if time.monotonic() >= deadline:
                raise RuntimeError("Blender timed out; inspect the scene before retrying")
            time.sleep(0.05)
        response = json.loads(reply.read_bytes())
        if "error" in response:
            raise RuntimeError(response["error"])
        return {"content": [{"type": "json", "json": response["result"]}]}
    finally:
        for path in (pending, request, reply):
            path.unlink(missing_ok=True)


def run():
    bridge = None
    for line in sys.stdin.buffer:
        msg = json.loads(line)
        if "id" not in msg:
            continue
        method = msg.get("method")
        params = msg.get("params", {})
        response = {"id": msg["id"], "jsonrpc": "2.0"}
        try:
            if method == "initialize":
                bridge = (params.get("config") or {}).get("bridge")
                if not isinstance(bridge, str) or not bridge:
                    raise InvalidInput("Expected the bundled Blender bridge")
                response["result"] = {"protocol": 2, "version": "0.1.0", "provides": ["tools"],
                                      "requires": [], "tools": TOOLS}
            elif method == "tools/call":
                response["result"] = call(bridge, params["name"], params.get("arguments", {}))
            elif method == "shutdown":
                response["result"] = {}
            else:
                response["error"] = {"code": -32601, "message": "Unknown method"}
        except (InvalidInput, NotStarted) as exc:
            kind = "not_started" if isinstance(exc, NotStarted) else "invalid_input"
            response["error"] = {"code": -32602, "message": str(exc), "data": {"kind": kind}}
        except Exception as exc:
            response["error"] = {"code": -32000, "message": str(exc)}
        print(json.dumps(response, allow_nan=False), flush=True)
        if method == "shutdown":
            break


if __name__ == "__main__":
    run()
