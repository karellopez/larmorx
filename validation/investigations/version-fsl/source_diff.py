# SPDX-License-Identifier: Apache-2.0
"""What changed in FSL's source between two installations, package by package.

For every FSL package the replicas translate from (crates-nc/fsl-source/MANIFEST.tsv, plus the
packages whose headers the translated code includes: fsl-armawrap), lists every file that was
added, removed or changed between ``~/fsl/src`` (6.0.7.17) and ``~/fsl-6.0.7.23/src``, with the
number of changed lines once whitespace is ignored and whether only comments changed. Build
logs and conda metadata are left out. The package versions come from each package's
``metadata_conda_debug.yaml`` (or, where 6.0.7.23 no longer installs it, from
``conda-meta/<package>-<version>-*.json``).

Usage: python -P source_diff.py [--old ~/fsl] [--new ~/fsl-6.0.7.23] [--package NAME ...]
"""

from __future__ import annotations

import argparse
import difflib
import re
import sys
from pathlib import Path

PACKAGES = [
    "fsl-mcflirt",
    "fsl-bet2",
    "fsl-newimage",
    "fsl-miscmaths",
    "fsl-newnifti",
    "fsl-znzlib",
    "fsl-meshclass",
    "fsl-utils",
    "fsl-avwutils",
    "fsl-armawrap",
]
SKIP = re.compile(r"conda_build|metadata_conda|build_env|depend\.mk|\.log$|/test/|/tests/")
COMMENT = re.compile(r"^\s*(//|/\*|\*|\*/|#(?!include|define|if|else|endif))")


def version(fsldir: Path, package: str) -> str:
    meta = fsldir / "src" / package / "metadata_conda_debug.yaml"
    if meta.is_file():
        m = re.search(r"^\s*version:\s*'?([^'\s]+)'?", meta.read_text(encoding="utf-8"), re.M)
        if m:
            return m.group(1)
    for j in sorted((fsldir / "pkgs").glob(f"{package}-[0-9]*")) + sorted(
        (fsldir / "conda-meta").glob(f"{package}-[0-9]*.json")
    ):
        m = re.match(re.escape(package) + r"-([0-9][^-]*)-", j.name)
        if m:
            return m.group(1)
    return "?"


def files(root: Path) -> set[str]:
    if not root.is_dir():
        return set()
    return {
        str(p.relative_to(root))
        for p in root.rglob("*")
        if p.is_file() and not SKIP.search(str(p.relative_to(root)))
    }


def text(p: Path) -> list[str]:
    try:
        return p.read_text(encoding="utf-8", errors="replace").splitlines()
    except OSError:
        return []


def compare(a: Path, b: Path) -> tuple[int, bool]:
    """(changed lines ignoring whitespace and blank lines, whether only comments changed)."""
    la = [" ".join(x.split()) for x in text(a) if x.strip()]
    lb = [" ".join(x.split()) for x in text(b) if x.strip()]
    changed = [
        d[2:]
        for d in difflib.unified_diff(la, lb, lineterm="", n=0)
        if d[:1] in "+-" and not d.startswith(("+++", "---"))
    ]
    return len(changed), all(COMMENT.match(x) for x in changed)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--old", type=Path, default=Path.home() / "fsl")
    ap.add_argument("--new", type=Path, default=Path.home() / "fsl-6.0.7.23")
    ap.add_argument("--package", nargs="*", default=PACKAGES)
    a = ap.parse_args()
    for pkg in a.package:
        old, new = a.old / "src" / pkg, a.new / "src" / pkg
        fo, fn = files(old), files(new)
        print(f"## {pkg} {version(a.old, pkg)} -> {version(a.new, pkg)}")
        for f in sorted(fo - fn):
            print(f"  removed  {f}")
        for f in sorted(fn - fo):
            print(f"  added    {f}")
        for f in sorted(fo & fn):
            if (old / f).read_bytes() == (new / f).read_bytes():
                continue
            n, comments = compare(old / f, new / f)
            kind = "whitespace only" if n == 0 else ("comments only" if comments else "code")
            print(f"  changed  {f}: {n} lines ({kind})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
