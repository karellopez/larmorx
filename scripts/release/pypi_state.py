# SPDX-License-Identifier: Apache-2.0
"""What PyPI holds of a release: nothing before the upload, every file after it.

`absent`: none of the projects has the version yet (PyPI never accepts a version twice, so a
release whose version is already there stops in its first job, before anything is built). A
project that does not exist yet counts as absent.

`wait`: waits until PyPI lists every file given (by name) with its SHA-256 under its project's
version (`https://pypi.org/pypi/<project>/<version>/json`), asking every 20 seconds; a file PyPI
lists with another SHA-256 is an error at once. larmorx's publish job waits so for its two
companions before it uploads larmorx itself: `larmorx[all]==<version>` then never names a version
PyPI does not have, and a failed companion upload stops larmorx's.

`--report`: print the answer (a notice in GitHub Actions) and exit 0, as dry runs do.

Usage:
    python -P scripts/release/pypi_state.py absent --version 0.1.0rc1 larmorx larmorx-nc larmorx-gpl
    python -P scripts/release/pypi_state.py wait --version 0.1.0rc1 --timeout 900 FILE...
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

PYPI = "https://pypi.org/pypi"


def project(filename: str) -> str:
    """The project a wheel or sdist belongs to: ``larmorx_nc-0.1.0rc1-....whl`` -> larmorx-nc."""
    return re.sub(r"[-_.]+", "-", filename.split("-", 1)[0]).lower()


def listed(project_name: str, version: str) -> dict[str, str] | None:
    """The files PyPI lists for the version (name -> SHA-256); None when it has no such version."""
    url = f"{PYPI}/{project_name}/{version}/json"
    request = urllib.request.Request(url, headers={"User-Agent": "larmorx-release"})
    try:
        with urllib.request.urlopen(request, timeout=30) as r:
            data = json.load(r)
    except urllib.error.HTTPError as e:
        if e.code == 404:
            return None
        raise
    return {f["filename"]: f["digests"]["sha256"] for f in data.get("urls", [])}


def compare(want: dict[str, str], have: dict[str, str] | None) -> tuple[list[str], list[str]]:
    """(files not listed yet, files listed with another SHA-256)."""
    have = have or {}
    missing = sorted(n for n in want if n not in have)
    other = sorted(n for n in want if n in have and have[n] != want[n])
    return missing, other


def _say(message: str, kind: str = "notice") -> None:
    print(message)
    if os.environ.get("GITHUB_ACTIONS") == "true":
        print(f"::{kind} title=PyPI::{message}")


def absent(version: str, projects: list[str]) -> list[str]:
    return [f"{p} {version} is already on PyPI" for p in projects if listed(p, version) is not None]


def wait(version: str, files: list[Path], timeout: float, every: float = 20) -> list[str]:
    want: dict[str, dict[str, str]] = {}
    for f in files:
        want.setdefault(project(f.name), {})[f.name] = hashlib.sha256(f.read_bytes()).hexdigest()
    deadline = time.monotonic() + timeout
    while True:
        waiting, problems = [], []
        for name, files_of in sorted(want.items()):
            try:
                missing, other = compare(files_of, listed(name, version))
            except (urllib.error.URLError, OSError, ValueError) as e:
                missing, other = sorted(files_of), []
                print(f"pypi_state: {name}: {e} (asking again)")
            problems += [f"PyPI lists {n} with another SHA-256 than this release's" for n in other]
            waiting += missing
        if problems:
            return problems
        if not waiting:
            return []
        if time.monotonic() > deadline:
            return [f"not on PyPI after {timeout:.0f} s: {', '.join(waiting)}"]
        print(f"pypi_state: waiting for {len(waiting)} files")
        time.sleep(every)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="command", required=True)
    a = sub.add_parser("absent", help="none of the projects has the version")
    a.add_argument("projects", nargs="+")
    w = sub.add_parser("wait", help="wait until PyPI lists every file")
    w.add_argument("files", nargs="+", type=Path)
    w.add_argument("--timeout", type=float, default=900)
    for p in (a, w):
        p.add_argument("--version", required=True, help="the release's version (PEP 440)")
        p.add_argument("--report", action="store_true", help="print the answer; exit 0")
    args = parser.parse_args(argv)
    if args.command == "absent":
        problems = absent(args.version, args.projects)
        good = f"{args.version} is not on PyPI yet ({', '.join(args.projects)})"
    else:
        problems = wait(args.version, args.files, args.timeout)
        good = f"PyPI lists all {len(args.files)} files of {args.version} with their SHA-256"
    for p in problems:
        _say(p, "notice" if args.report else "error")
    if not problems:
        _say(good)
    return 0 if args.report or not problems else 1


if __name__ == "__main__":
    sys.exit(main())
