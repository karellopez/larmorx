# SPDX-License-Identifier: Apache-2.0
"""Make the source distribution of larmorx-nc or larmorx-gpl.

`maturin sdist` cannot make it in `crates-nc/` or `crates-gpl/`: their crates use the root
workspace's crates by path (`../crates/larmorx-core`, ...), so maturin roots the sdist at the
repository and then puts both workspaces' `Cargo.toml` at the same place ("File ... was already
added"). Here the repository's tracked files that a build can need (the root workspace's manifests,
`crates/`, the companion's directory, the licences) are copied into a staging directory, with the
companion's `pyproject.toml` moved to its root and every path in it given the companion's
directory as prefix; `maturin sdist` there makes an sdist that builds the same program, with the
wheel data and every licence file, FSL's original source for larmorx-nc included.

The files are taken from the working tree (``git ls-files``: tracked files only, as they are now),
so that a description rewritten for PyPI before the build (`scripts/release/pypi_readme.py`) is the
one packaged. The sdist is checked afterwards: the companion's licences, notices and the sources of
its program's crates (for larmorx-nc FSL's original source and the wheel data too) are in it, and
nothing of what is never distributed (`crates-gpl/tools/`, the verification tools).

Usage:
    python -P scripts/release/companion_sdist.py nc --out dist [--maturin PATH]
    python -P scripts/release/companion_sdist.py gpl --out dist
"""

from __future__ import annotations

import argparse
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
#: What the staging copy holds besides the companion's directory.
SHARED = ("Cargo.toml", "Cargo.lock", "LICENSE", "NOTICE", "LICENSES/", "crates/")
#: Files of the companion's directory each sdist must hold (globs relative to it), and those it
#: must not (never distributed).
REQUIRED = {
    "nc": (
        "LICENSE",
        "NOTICE",
        "PYPI.md",
        "LICENSES/*",
        "Cargo.toml",
        "Cargo.lock",
        "fsl-source/**/*",
        "wheel-data/**/*",
        "larmorx-nc-*/Cargo.toml",
        "larmorx-nc-*/src/**/*",
        # the program's version in PEP 440's form (it includes crates/larmorx-core/src/pep440.rs)
        "larmorx-nc-cli/build.rs",
    ),
    "gpl": (
        "LICENSE",
        "NOTICE",
        "PYPI.md",
        # The program's licences (LICENSES/LGPL-2.1.txt is the verification tools' glibc text).
        "LICENSES/LGPL-2.1-tedana.txt",
        "LICENSES/Apache-2.0.txt",
        "LICENSES/NOTICE-larmorx.txt",
        "LICENSES/third-party-rust.txt",
        "Cargo.toml",
        "Cargo.lock",
        "larmorx-gpl-*/Cargo.toml",
        "larmorx-gpl-*/src/**/*",
        # the program's version in PEP 440's form (it includes crates/larmorx-core/src/pep440.rs)
        "larmorx-gpl-cli/build.rs",
    ),
}
FORBIDDEN = {"nc": (), "gpl": ("tools/",)}


def git(*args: str) -> str:
    return subprocess.run(
        ["git", "-C", str(ROOT), *args], check=True, capture_output=True, text=True,
        encoding="utf-8",
    ).stdout  # fmt: skip


def tracked() -> list[str]:
    return [p for p in git("ls-files", "-z").split("\0") if p]


def staged_pyproject(text: str, sub: str) -> str:
    """The companion's pyproject.toml with its paths relative to the repository's root."""

    def prefix(m: re.Match[str]) -> str:
        return f'{m.group(1)}"{sub}/{m.group(2)}"'

    text = re.sub(r'(?m)^(readme = )"([^"]+)"', prefix, text)
    text = re.sub(r'(?m)^(manifest-path = )"([^"]+)"', prefix, text)
    text = re.sub(r'(?m)^(data = )"([^"]+)"', prefix, text)
    text = re.sub(r'(\{ ?path = )"([^"]+)"', prefix, text)

    def files(m: re.Match[str]) -> str:
        return m.group(1) + re.sub(r'"([^"]+)"', rf'"{sub}/\1"', m.group(2)) + m.group(3)

    return re.sub(r"(?s)(license-files = \[)(.*?)(\])", files, text)


def stage(pkg: str, where: Path) -> Path:
    """Copy what the build needs into ``where``; returns the staging root."""
    sub = f"crates-{pkg}"
    wanted = (*SHARED, f"{sub}/")
    for rel in tracked():
        if not any(rel == w or (w.endswith("/") and rel.startswith(w)) for w in wanted):
            continue
        src = ROOT / rel
        if not src.is_file():
            continue  # deleted in the working tree
        dst = where / rel
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(src, dst)
    pyproject = where / sub / "pyproject.toml"
    (where / "pyproject.toml").write_bytes(
        staged_pyproject(pyproject.read_text(encoding="utf-8"), sub).encode("utf-8")
    )
    pyproject.unlink()
    return where


def glob_regex(pattern: str) -> re.Pattern[str]:
    """A glob of ``/``-separated paths as a regular expression: ``**/`` any directories, ``*``
    any name part (Python 3.12's ``PurePath.match`` has no recursive ``**``)."""
    out = ""
    for part in re.split(r"(\*\*/|\*)", pattern):
        out += {"**/": "(?:[^/]+/)*", "*": "[^/]*"}.get(part, re.escape(part))
    return re.compile(out + r"\Z")


def check_sdist(pkg: str, sdist: Path) -> list[str]:
    """What the sdist lacks or must not hold; empty if complete."""
    sub = f"crates-{pkg}"
    with tarfile.open(sdist) as tar:
        names = [n.split("/", 1)[1] for n in tar.getnames() if "/" in n]
    held = {n for n in names if n.startswith(f"{sub}/")}
    problems = []
    for pattern in REQUIRED[pkg]:
        rx = glob_regex(f"{sub}/{pattern}")
        want = {rel for rel in tracked() if rx.match(rel)}
        if not want:
            problems.append(f"no tracked file matches {sub}/{pattern}")
        problems += [f"missing {rel}" for rel in sorted(want - held)]
    for prefix in FORBIDDEN[pkg]:
        problems += [
            f"never distributed, but in it: {n}"
            for n in sorted(held)
            if n.startswith(f"{sub}/{prefix}")
        ]
    return problems


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "package", choices=sorted(REQUIRED), help="nc (larmorx-nc) or gpl (larmorx-gpl)"
    )
    parser.add_argument("--out", type=Path, required=True, help="where the sdist is written")
    parser.add_argument(
        "--maturin", default="maturin", help="the maturin program (default: maturin)"
    )
    args = parser.parse_args(argv)
    sub = f"crates-{args.package}"
    name = tomllib.loads((ROOT / sub / "pyproject.toml").read_text(encoding="utf-8"))["project"][
        "name"
    ]
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    before = set(out.glob("*.tar.gz"))
    with tempfile.TemporaryDirectory(prefix=f"larmorx-{args.package}-sdist-") as tmp:
        root = stage(args.package, Path(tmp))
        result = subprocess.run([args.maturin, "sdist", "--out", str(out)], cwd=root, check=False)
        if result.returncode != 0:
            print(f"companion_sdist: maturin sdist failed for {name}", file=sys.stderr)
            return result.returncode
    made = (
        sorted(set(out.glob("*.tar.gz")) - before)
        or sorted(out.glob(f"{name.replace('-', '_')}-*.tar.gz"), key=lambda p: p.stat().st_mtime)[
            -1:
        ]
    )
    if not made:
        print(f"companion_sdist: no sdist of {name} in {out}", file=sys.stderr)
        return 1
    problems = check_sdist(args.package, made[-1])
    for p in problems:
        print(f"companion_sdist: {made[-1].name}: {p}", file=sys.stderr)
    if not problems:
        print(f"companion_sdist: {made[-1]} (complete)")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
