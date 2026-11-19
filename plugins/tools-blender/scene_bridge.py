"""Run in Blender's Text Editor to connect the current scene to RAH on Windows."""
import atexit
import json
import msvcrt
import os
from pathlib import Path
import sys
import tempfile
import time

import bpy

PRIMITIVES = {
    "cube": "primitive_cube_add", "sphere": "primitive_uv_sphere_add",
    "cylinder": "primitive_cylinder_add", "plane": "primitive_plane_add",
    "cone": "primitive_cone_add",
}


def inspect_object(obj):
    return {"name": obj.name, "type": obj.type, "position": list(obj.matrix_world.translation)}


def execute(name, args):
    scene = bpy.context.scene
    if name == "blender.list":
        objects = scene.objects if args["scope"] == "scene" else bpy.context.selected_objects
        return sorted(obj.name for obj in objects)
    if name == "blender.create":
        if bpy.context.mode != "OBJECT":
            raise ValueError("Switch Blender to Object Mode before creating an object")
        operator = getattr(bpy.ops.mesh, PRIMITIVES[args["shape"]])
        if operator() != {"FINISHED"}:
            raise RuntimeError("Blender cancelled creation")
        obj = bpy.context.object
        obj.name = args["name"]
    else:
        obj = scene.objects.get(args["name"])
        if obj is None:
            raise ValueError("Object not found in the current scene: " + args["name"])
        if name == "blender.move":
            matrix = obj.matrix_world.copy()
            matrix.translation = (args["x"], args["y"], args["z"])
            obj.matrix_world = matrix
        elif name != "blender.inspect":
            raise ValueError("Unknown Blender tool")
    bpy.context.view_layer.update()
    return inspect_object(obj)


def connect():
    existing = getattr(sys, "_rah_blender_timer", None)
    if existing and bpy.app.timers.is_registered(existing):
        return existing
    root = Path(os.environ["LOCALAPPDATA"]) / "rah" / "blender"
    root.mkdir(parents=True, exist_ok=True)
    workspace = tempfile.TemporaryDirectory(prefix="scene-", dir=root, ignore_cleanup_errors=True)
    directory = Path(workspace.name)
    lease = (directory / "active").open("wb")
    msvcrt.locking(lease.fileno(), msvcrt.LK_NBLCK, 1)

    def close():
        lease.close()
        workspace.cleanup()

    atexit.register(close)

    def tick():
        for request in directory.glob("*.request"):
            try:
                request.rename(request.with_suffix(".running"))
            except FileNotFoundError:
                continue
            running = request.with_suffix(".running")
            try:
                data = json.loads(running.read_bytes())
                if time.monotonic() > data["deadline"]:
                    raise ValueError("Blender request expired")
                result = execute(data["name"], data["arguments"])
                reply = json.dumps({"result": result}, allow_nan=False).encode()
                if len(reply) > 128 * 1024:
                    raise ValueError("Result too large; query the selection or one object")
            except Exception as exc:
                reply = json.dumps({"error": str(exc)[:2048]}).encode()
            try:
                pending = request.with_suffix(".out")
                pending.write_bytes(reply)
                pending.replace(request.with_suffix(".reply"))
            finally:
                running.unlink(missing_ok=True)
        return 0.1

    bpy.app.timers.register(tick, persistent=True)
    sys._rah_blender_timer = tick
    (directory / "ready").touch()
    return tick


if __name__ == "__main__":
    connect()
