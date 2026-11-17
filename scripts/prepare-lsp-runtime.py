"""Prepare a pinned Windows x64 LSP runtime; targets need no npm or downloads.

Only preparation downloads. --offline requires the verified archive cache.
No install scripts run. --check verifies every staged file against runtime.json.
"""

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import shutil
import tarfile
import tempfile
import urllib.request
import zipfile

LOCK = Path(__file__).resolve().parent.parent / "packaging/windows/lsp-runtime.lock.json"
PROGRAMS = {
    "rust": ["rust-analyzer.exe"],
    "typescript": ["node.exe", "node_modules/typescript-language-server/lib/cli.mjs", "--stdio"],
    "pyright": ["node.exe", "node_modules/pyright/langserver.index.js", "--stdio"],
    "clangd": ["clangd/bin/clangd.exe"],
}


def digest(path, algorithm="sha256"):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, algorithm).hexdigest()


def contained(root, relative):
    if "\\" in relative or ":" in relative:
        raise ValueError(f"Unsafe bundle path: {relative}")
    path = PurePosixPath(relative)
    if path.is_absolute() or ".." in path.parts or not path.parts:
        raise ValueError(f"Unsafe bundle path: {relative}")
    result = root.joinpath(*path.parts).resolve()
    if not result.is_relative_to(root.resolve()):
        raise ValueError(f"Bundle path escapes root: {relative}")
    return result


def fetch(asset, cache, offline):
    target = cache / asset["file"]
    if not target.exists():
        if offline:
            raise ValueError(f"Missing cached archive: {target.name}")
        request = urllib.request.Request(asset["url"], headers={"User-Agent": "RAH-LSP-packaging"})
        with urllib.request.urlopen(request, timeout=120) as response, target.open("xb") as output:
            shutil.copyfileobj(response, output)
    if digest(target, asset["algorithm"]) != asset["digest"]:
        raise ValueError(f"Archive checksum mismatch: {target.name}; remove it and retry")
    return target


def extract(archive, destination, prefix):
    destination.mkdir(parents=True, exist_ok=True)
    if archive.suffix == ".zip":
        with zipfile.ZipFile(archive) as source:
            for entry in source.infolist():
                if entry.is_dir() or not entry.filename.startswith(prefix):
                    continue
                if (entry.external_attr >> 16) & 0o170000 == 0o120000:
                    raise ValueError("Archive symlinks are not supported")
                target = contained(destination, entry.filename[len(prefix):])
                target.parent.mkdir(parents=True, exist_ok=True)
                with source.open(entry) as inp, target.open("xb") as out:
                    shutil.copyfileobj(inp, out)
    else:
        with tarfile.open(archive) as source:
            for entry in source:
                if entry.isdir() or not entry.name.startswith(prefix):
                    continue
                if not entry.isfile():
                    raise ValueError("Archive links and special files are not supported")
                target = contained(destination, entry.name[len(prefix):])
                target.parent.mkdir(parents=True, exist_ok=True)
                with source.extractfile(entry) as inp, target.open("xb") as out:
                    shutil.copyfileobj(inp, out)


def check(root):
    root = root.resolve(strict=True)
    manifest = json.loads((root / "runtime.json").read_text(encoding="utf-8"))
    if (manifest.get("format"), manifest.get("platform"), manifest.get("arch")) != (1, "windows", "x86_64"):
        raise ValueError("Unsupported LSP runtime")
    hashes = manifest["sha256"]
    actual = {p.relative_to(root).as_posix() for p in root.rglob("*") if p.is_file() and p != root / "runtime.json"}
    if actual != set(hashes):
        raise ValueError("Runtime file inventory differs from manifest")
    for name, expected in hashes.items():
        if digest(contained(root, name)) != expected:
            raise ValueError(f"Runtime checksum mismatch: {name}")
    for argv in PROGRAMS.values():
        for name in argv:
            if not name.startswith("--") and not contained(root, name).is_file():
                raise ValueError(f"Missing runtime program: {name}")
    if not (root / "node_modules/typescript/lib/tsserver.js").is_file():
        raise ValueError("Missing TypeScript tsserver")
    return manifest


def prepare(args):
    output = args.out.resolve()
    if output.exists():
        raise ValueError("Output already exists; choose a new directory")
    cache = args.cache.resolve()
    cache.mkdir(parents=True, exist_ok=True)
    lock = json.loads(LOCK.read_text(encoding="utf-8"))
    archives = [(asset, fetch(asset, cache, args.offline)) for asset in lock["assets"]]
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="lsp-stage-", dir=output.parent) as temporary:
        stage = Path(temporary) / "runtime"
        stage.mkdir()
        for asset, archive in archives:
            dest = contained(stage, asset["destination"])
            if asset.get("raw"):
                dest.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(archive, dest)
            else:
                extract(archive, dest, asset["prefix"])
        # Node's npm is a build tool, never needed on the target. Keep its license.
        node = stage / "node"
        shutil.copy2(node / "node.exe", stage / "node.exe")
        licenses = stage / "licenses"
        licenses.mkdir(exist_ok=True)
        shutil.copy2(node / "LICENSE", licenses / "node-LICENSE.txt")
        shutil.rmtree(node)
        rust = stage / "rust-temp"
        shutil.copy2(rust / "rust-analyzer.exe", stage / "rust-analyzer.exe")
        shutil.rmtree(rust)
        manifest = {"format": 1, "platform": "windows", "arch": "x86_64", "versions": lock["versions"],
                    "sha256": {p.relative_to(stage).as_posix(): digest(p) for p in sorted(stage.rglob("*")) if p.is_file()}}
        (stage / "runtime.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
        check(stage)
        stage.rename(output)
    print(f"Prepared {output}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--cache", type=Path, default=Path(".rah/lsp-downloads"))
    parser.add_argument("--offline", action="store_true")
    parser.add_argument("--check", type=Path)
    args = parser.parse_args()
    if args.check:
        print(json.dumps(check(args.check)["versions"]))
    elif args.out:
        prepare(args)
    else:
        parser.error("Supply --out or --check")


if __name__ == "__main__":
    main()
