# SPDX-License-Identifier: Apache-2.0
"""Sub-task 4's first pass: where FreeSurfer's scripts and programs name the data files they read,
and the installed package's data files with their sizes and SHA-256.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS: this tool reads FreeSurfer's source and prints lines
of it.

    python -P validation/investigations/freesurfer-inventory/data_refs.py refs \\
        <workspace>/tags/freesurfer-8.2.0 docs/freesurfer/programs.toml > <scratch>/data-refs.txt
    python -P validation/investigations/freesurfer-inventory/data_refs.py package \\
        <package root> --tsv <scratch>/package-files.tsv [--workers 4]

``refs`` prints, for every script and Python program on a default path (programs.toml's
``source``), the recon-all configuration files and every C/C++ file of the call graph's reach
(``--reached <licences.json>``), each line that names a data location (``FREESURFER_HOME``,
``FREESURFER``, ``average/``, ``models/``, ``fsaverage``, an atlas or table extension). It is a
reading aid: the variables in a path (``$GCA``, ``$GCADIR``) are resolved by reading the script.
``package`` lists every file under the package's data folders (``average``, ``models``,
``subjects``, ``luts``, ``mni``, the top-level tables) with its size in bytes and SHA-256.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
import tomllib
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

DATA = re.compile(
    r"FREESURFER_HOME|getenv\(\s*\"FREESURFER|\baverage/|\bmodels/|fsaverage|"
    r"\.(gca|gcs|tif|h5|npy|pt|ctab|lut|4dfp\.img|xfm|lta|dat|annot|label)\b|"
    r"LUT\.txt|colortable|mni305|MNI152|talairach",
    re.I,
)
CONFIG = [
    "distribution/etc/recon-config.yaml",
    "distribution/etc/global-expert-options.v8.txt",
]
DATA_DIRS = ["average", "models", "subjects", "luts", "mni", "mni-1.4"]


def refs(root: Path, programs: Path, reached: Path | None) -> None:
    table = tomllib.loads(programs.read_text(encoding="utf-8"))["program"]
    files = list(CONFIG)
    for p in table:
        if p["streams"] and p["source"] and p["kind"] not in ("c++", "c", "c+fortran"):
            files.append(p["source"])
    if reached is not None:
        report = json.loads(reached.read_text(encoding="utf-8"))
        files += [k for k in report if not k.startswith("packages/")]
    seen = set()
    for rel in files:
        if rel in seen:
            continue
        seen.add(rel)
        path = root / rel
        if not path.is_file():
            continue
        hits = []
        for n, line in enumerate(
            path.read_text(encoding="utf-8", errors="replace").splitlines(), 1
        ):
            code = line.strip()
            if DATA.search(code) and not code.startswith(("//", "*", "/*")):
                hits.append(f"  {n}: {code[:150]}")
        if hits:
            print(f"## {rel} ({len(hits)})")
            print("\n".join(hits))


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def package(root: Path, tsv: Path, workers: int) -> None:
    paths = [p for p in sorted(root.glob("*.txt")) if p.is_file()]
    for d in DATA_DIRS:
        if (root / d).is_dir():
            paths += [
                p for p in sorted((root / d).rglob("*")) if p.is_file() and not p.is_symlink()
            ]
    with ThreadPoolExecutor(max_workers=workers) as pool:
        digests = list(pool.map(sha256, paths))
    lines = ["path\tbytes\tsha256"]
    total = 0
    for p, digest in zip(paths, digests, strict=True):
        size = p.stat().st_size
        total += size
        lines.append(f"{p.relative_to(root)}\t{size}\t{digest}")
    tsv.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"{len(paths)} files, {total:,} bytes")


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="mode", required=True)
    r = sub.add_parser("refs")
    r.add_argument("root", type=Path)
    r.add_argument("programs", type=Path)
    r.add_argument("--reached", type=Path, default=None, help="licences.py's --json output")
    p = sub.add_parser("package")
    p.add_argument("root", type=Path)
    p.add_argument("--tsv", type=Path, required=True)
    p.add_argument("--workers", type=int, default=4)
    args = parser.parse_args(argv)
    if args.mode == "refs":
        refs(args.root.resolve(), args.programs, args.reached)
    else:
        package(args.root.resolve(), args.tsv, args.workers)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
