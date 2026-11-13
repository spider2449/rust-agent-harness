"""Run the pinned Graphify extractor with BOM-compatible Cargo manifest reads."""

from pathlib import Path

from graphify import cargo_introspect, manifest_ingest


def load_manifest(path: Path) -> dict:
    """Accept the optional UTF-8 BOM without hiding malformed manifest errors."""
    try:
        import tomllib
    except ModuleNotFoundError:
        import tomli as tomllib
    return tomllib.loads(path.read_text(encoding="utf-8-sig"))


# This affects only this supervised child and its Python worker imports.
cargo_introspect._load_toml = load_manifest

_cargo_parser = manifest_ingest._PARSERS["cargo"]


def parse_cargo_manifest(text: str) -> dict | None:
    """Keep package/dependency AST nodes as well as Cargo introspection nodes."""
    return _cargo_parser(text.removeprefix("\ufeff"))


manifest_ingest._PARSERS["cargo"] = parse_cargo_manifest

if __name__ == "__main__":
    from graphify.__main__ import main

    main()
