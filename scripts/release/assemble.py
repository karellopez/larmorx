# SPDX-License-Identifier: Apache-2.0
"""Gather a release's files, check that each one is there, and write SHA256SUMS.

The release workflow's `assemble` job downloads every file its build jobs made into one folder
(any layout) and runs this script, which

- checks the set: per package one wheel for each of the six platforms with the tags the release
  promises (Linux `manylinux_2_28`: glibc 2.28 or newer; macOS 11.0 or newer; Windows x64 and
  arm64; larmorx's `cp312-abi3`, the companions' `py3-none`), the three sdists, and the three
  standalone archives of each platform (`scripts/release/archives.py`); nothing missing, nothing
  else, nothing of another version, no name twice;
- checks each file's size against PyPI's limit (100 MiB a file);
- lays them out for the jobs after it: `<out>/pypi/` (the 21 files PyPI receives; each package's
  publish job takes its own 7), `<out>/archives/` (the 18 archives, for the GitHub Release only)
  and `<out>/SHA256SUMS` (every file of both, in `sha256sum -c`'s form, sorted by name);
- prints the files with their sizes (in GitHub Actions also as one notice annotation, which
  GitHub's API serves without a login).

Usage:
    python -P scripts/release/assemble.py --version 0.1.0rc1 --in <downloaded> --out release
"""

from __future__ import annotations

import argparse
import hashlib
import os
import shutil
import sys
from pathlib import Path

#: The six platforms: the wheels' platform tag and the archives' target triple.
PLATFORMS = {
    "linux-x64": ("manylinux_2_28_x86_64", "x86_64-unknown-linux-gnu"),
    "linux-aarch64": ("manylinux_2_28_aarch64", "aarch64-unknown-linux-gnu"),
    "macos-arm64": ("macosx_11_0_arm64", "aarch64-apple-darwin"),
    "macos-x64": ("macosx_11_0_x86_64", "x86_64-apple-darwin"),
    "windows-x64": ("win_amd64", "x86_64-pc-windows-msvc"),
    "windows-arm64": ("win_arm64", "aarch64-pc-windows-msvc"),
}
#: The packages: their file names' distribution part and their wheels' Python tags.
PACKAGES = {
    "larmorx": ("larmorx", "cp312-abi3"),
    "larmorx-nc": ("larmorx_nc", "py3-none"),
    "larmorx-gpl": ("larmorx_gpl", "py3-none"),
}
#: PyPI's limit on one file.
LIMIT = 100 * 1024 * 1024


def expected(version: str) -> dict[str, tuple[str, str]]:
    """Every file of a release: name -> (folder: pypi or archives, package)."""
    files: dict[str, tuple[str, str]] = {}
    for package, (dist, tags) in PACKAGES.items():
        files[f"{dist}-{version}.tar.gz"] = ("pypi", package)
        for tag, triple in PLATFORMS.values():
            files[f"{dist}-{version}-{tags}-{tag}.whl"] = ("pypi", package)
            ext = ".zip" if "windows" in triple else ".tar.gz"
            files[f"{package}-{version}-{triple}{ext}"] = ("archives", package)
    return files


def collect(folder: Path) -> tuple[dict[str, Path], list[str]]:
    """The files under ``folder`` by name, and the problems (a name found twice)."""
    found: dict[str, Path] = {}
    problems = []
    for path in sorted(p for p in folder.rglob("*") if p.is_file()):
        if path.name in found:
            problems.append(f"{path.name} found twice ({found[path.name]}, {path})")
        found.setdefault(path.name, path)
    return found, problems


def check(version: str, found: dict[str, Path]) -> list[str]:
    want = expected(version)
    problems = [f"missing: {name}" for name in sorted(set(want) - set(found))]
    problems += [f"not a file of {version}'s release: {n}" for n in sorted(set(found) - set(want))]
    for name in sorted(set(want) & set(found)):
        size = found[name].stat().st_size
        if size > LIMIT:
            problems.append(f"{name}: {size:,} bytes, over PyPI's limit of {LIMIT:,}")
        if size == 0:
            problems.append(f"{name}: empty")
    return problems


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def lay_out(version: str, found: dict[str, Path], out: Path) -> list[tuple[str, int, str]]:
    """Copy the files into ``out``'s folders, write SHA256SUMS; (name, size, sha256) each."""
    rows = []
    for name, (folder, _) in sorted(expected(version).items()):
        (out / folder).mkdir(parents=True, exist_ok=True)
        target = out / folder / name
        shutil.copyfile(found[name], target)
        rows.append((name, target.stat().st_size, sha256(target)))
    sums = "".join(f"{digest}  {name}\n" for name, _, digest in rows)
    (out / "SHA256SUMS").write_bytes(sums.encode("ascii"))
    return rows


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--version", required=True, help="the release's version (PEP 440)")
    parser.add_argument("--in", dest="folder", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args(argv)
    found, problems = collect(args.folder)
    problems += check(args.version, found)
    if problems:
        for p in problems:
            print(f"assemble: {p}", file=sys.stderr)
            if os.environ.get("GITHUB_ACTIONS") == "true":
                print(f"::error title=release files::{p}")
        print(f"ASSEMBLE FAILED: {len(problems)} problems")
        return 1
    rows = lay_out(args.version, found, args.out)
    width = max(len(name) for name, _, _ in rows)
    table = [f"{name:<{width}}  {size / 1e6:8.2f} MB  {digest[:16]}" for name, size, digest in rows]
    print("\n".join(table))
    total = sum(size for _, size, _ in rows)
    print(f"ASSEMBLE ok: {len(rows)} files, {total / 1e6:.1f} MB, SHA256SUMS written")
    if os.environ.get("GITHUB_ACTIONS") == "true":
        print("::notice title=release files (MB)::" + "%0A".join(table))
    return 0


if __name__ == "__main__":
    sys.exit(main())
