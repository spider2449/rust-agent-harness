"""Prepare/check a relocatable Graphify runtime without using uv.

Supply a portable Python distribution, a Node executable and its license.
Dependencies come from pip (optionally an offline wheelhouse) or a prepared
site-packages directory built for the same Python ABI/platform. A venv is not
a portable interpreter and is refused. Targets never need pip or uv.
"""

import argparse
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys

VERSION = "0.9.107"


def run(argv, timeout=180):
    result = subprocess.run([str(value) for value in argv], capture_output=True,
                            timeout=timeout, env={**os.environ, "PYTHONDONTWRITEBYTECODE": "1"})
    if result.returncode:
        raise ValueError(f"Runtime command failed: {result.stderr.decode('utf-8', 'replace')[-4000:]}")
    return result.stdout.decode("utf-8").strip()


def host():
    system = {"win32": "windows", "darwin": "macos"}.get(sys.platform, sys.platform)
    machine = platform.machine().lower()
    arch = {"amd64": "x86_64", "arm64": "aarch64"}.get(machine, machine)
    return system, arch


def contained(root, relative):
    path = Path(relative)
    if path.is_absolute() or ".." in path.parts:
        raise ValueError("Runtime program escapes its bundle")
    resolved = (root / path).resolve(strict=True)
    if not resolved.is_relative_to(root.resolve()) or not resolved.is_file():
        raise ValueError("Runtime program escapes its bundle or is missing")
    return resolved


def check(root):
    root = Path(root).resolve(strict=True)
    manifest = json.loads((root / "runtime.json").read_text(encoding="utf-8"))
    system, arch = host()
    if (manifest.get("format"), manifest.get("graphify_version"), manifest.get("platform"), manifest.get("arch")) != (1, VERSION, system, arch):
        raise ValueError("Runtime format, Graphify version or platform does not match")
    node = contained(root, manifest["node"])
    python = contained(root, manifest["python"])
    node_version = run([node, "--version"], 10)
    if int(node_version.lstrip("v").split(".")[0]) < 20:
        raise ValueError("Node.js 20+ is required")
    code = (
        "import sys,json,importlib.metadata as m,pathlib; "
        "assert sys.version_info >= (3,10); "
        f"assert m.version('graphifyy') == '{VERSION}'; "
        "import graphify,graphify.serve,mcp,tree_sitter; "
        "assert pathlib.Path(sys.base_prefix).resolve().is_relative_to(pathlib.Path(sys.argv[1]).resolve()); "
        "assert all(pathlib.Path(p).resolve().is_relative_to(pathlib.Path(sys.argv[1]).resolve()) for p in sys.path if p); "
        "assert pathlib.Path(graphify.__file__).resolve().is_relative_to(pathlib.Path(sys.argv[1]).resolve()); "
        "print(json.dumps({'python_version':sys.version.split()[0],"
        "'packages':sorted([{'name':d.metadata['Name'],'version':d.version} for d in m.distributions()],key=lambda p:p['name'].lower())}))"
    )
    details = json.loads(run([python, "-I", "-c", code, root]))
    return {**manifest, **details, "node_version": node_version}


def prepare(args):
    source = Path(args.python_root).resolve(strict=True)
    node = Path(args.node).resolve(strict=True)
    license_file = Path(args.node_license).resolve(strict=True)
    output = Path(args.out).resolve()
    if output.exists():
        raise ValueError("Output already exists; use a new directory to preserve prior bundles")
    if args.offline and not args.site_packages and not args.wheelhouse:
        raise ValueError("Offline preparation needs --wheelhouse or --site-packages")
    if (source / "pyvenv.cfg").exists():
        raise ValueError("A venv cannot be bundled as a portable interpreter; supply a complete portable Python root")
    if not any((source / name).is_file() for name in ["LICENSE.txt", "LICENSE", "LICENSE.rst"]):
        raise ValueError("The portable Python root must include its license file")
    sources = [source]
    if args.site_packages:
        sources.append(Path(args.site_packages).resolve(strict=True))
    if any(output.is_relative_to(item) for item in sources):
        raise ValueError("Output must not be inside a source directory")
    for folder, directories, files in os.walk(source):
        for name in directories + files:
            entry = Path(folder) / name
            if entry.is_symlink() and not entry.resolve().is_relative_to(source):
                raise ValueError(f"Python root contains an external symlink: {entry}")
    output.mkdir(parents=True)
    shutil.copytree(source, output / "python", ignore=shutil.ignore_patterns("__pycache__", "*.pyc"))
    windows = sys.platform == "win32"
    python_relative = "python/python.exe" if windows else "python/bin/python3"
    python = contained(output, python_relative)
    node_relative = "node.exe" if windows else "node"
    shutil.copy2(node, output / node_relative)
    (output / "licenses").mkdir()
    shutil.copy2(license_file, output / "licenses" / "node-LICENSE.txt")
    target_version = json.loads(run([python, "-I", "-c", "import json,sys; print(json.dumps(list(sys.version_info[:2])))"]))
    if target_version < [3, 10]:
        raise ValueError("Python 3.10+ is required")
    site = output / "python" / ("Lib/site-packages" if windows else f"lib/python{target_version[0]}.{target_version[1]}/site-packages")
    site.mkdir(parents=True, exist_ok=True)
    if args.site_packages:
        shutil.copytree(sources[1], site, dirs_exist_ok=True, ignore=shutil.ignore_patterns("__pycache__", "*.pyc"))
    else:
        command = [python, "-I", "-m", "pip", "install", "--disable-pip-version-check", "--no-compile", "--upgrade", "--only-binary=:all:",
                   "--target", site, f"graphifyy[mcp]=={VERSION}"]
        if args.wheelhouse:
            command.extend(["--no-index", "--find-links", Path(args.wheelhouse).resolve(strict=True)])
        run(command)
    # Environment-local bootstrap hooks must not modify the relocated runtime.
    for filename in ["_virtualenv.pth", "_virtualenv.py"]:
        (site / filename).unlink(missing_ok=True)
    # Windows embeddable Python uses an explicit relative import-path file.
    for path_file in (output / "python").glob("python*._pth"):
        zips = [p.name for p in (output / "python").glob("python*.zip")]
        path_file.write_text("\n".join(zips + [".", "Lib", "Lib/site-packages", "import site", ""]), encoding="utf-8")
    system, arch = host()
    manifest = {"format": 1, "graphify_version": VERSION, "platform": system, "arch": arch,
                "python": python_relative, "node": node_relative}
    (output / "runtime.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    manifest = check(output)
    (output / "runtime.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"runtime": str(output), "python": manifest["python_version"], "node": manifest["node_version"], "graphify": VERSION}))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", metavar="DIR", help="Validate an existing bundle without uv or downloads")
    parser.add_argument("--python-root")
    parser.add_argument("--node")
    parser.add_argument("--node-license")
    parser.add_argument("--out")
    parser.add_argument("--site-packages")
    parser.add_argument("--wheelhouse")
    parser.add_argument("--offline", action="store_true")
    args = parser.parse_args()
    try:
        if args.check:
            manifest = check(args.check)
            print(json.dumps({"graphify": manifest["graphify_version"], "python": manifest["python_version"], "node": manifest["node_version"]}))
        else:
            if not all([args.python_root, args.node, args.node_license, args.out]):
                parser.error("Preparation requires --python-root, --node, --node-license and --out")
            prepare(args)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        parser.exit(1, f"Graphify runtime: {error}\n")


if __name__ == "__main__":
    main()
