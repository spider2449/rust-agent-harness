from pathlib import Path

path = Path("crates/rah-desktop/src/git_discovery.rs")
text = path.read_text(encoding="utf-8")
old = """    let units: Vec<u16> = value\n        .bytes\n        .chunks_exact(2)\n        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))\n        .collect();\n"""
new = """    let units: Vec<u16> = value\n        .bytes\n        .as_chunks::<2>()\n        .0\n        .iter()\n        .map(|pair| u16::from_le_bytes(*pair))\n        .collect();\n"""
if text.count(old) != 1:
    raise SystemExit("expected exactly one git_discovery chunks_exact block")
path.write_text(text.replace(old, new, 1), encoding="utf-8")
