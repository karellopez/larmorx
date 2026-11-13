# SPDX-License-Identifier: Apache-2.0
"""The merge's check of the published site (the documentation revision, step 1): no file of the
site, its source and its build, holds a home path, the workspace's folder or its parent's, an
agent's worktree or scratch folder, the account's name or the machine's name.

    python -P site_paths_check.py <site checkout, built>

Fonts and images are skipped; the evidence images (``static/evidence/blobs/``) are scanned on
their own (``grep -a``). Exit status 1 when anything is found.
"""

from __future__ import annotations

import getpass
import socket
import sys
from pathlib import Path


def words() -> list[str]:
    """What must not be published, read from this machine (nothing is written here by name)."""
    found = [str(Path.home()), ".claude/worktrees", "/tmp/claude-"]
    for parent in Path(__file__).resolve().parents:
        if (parent / "oracles").is_dir():
            found += [parent.name, parent.parent.name]
            break
    found += [getpass.getuser(), socket.gethostname()]
    return [w for w in dict.fromkeys(found) if len(w) > 2]


def main() -> int:
    site = Path(sys.argv[1])
    skip = {".png", ".webp", ".woff2", ".woff", ".ttf", ".bin", ".gz", ".ico", ".jpg"}
    bad = 0
    for f in sorted(site.rglob("*")):
        if not f.is_file() or ".git" in f.parts or "blobs" in f.parts or f.suffix in skip:
            continue
        try:
            text = f.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
        for w in words():
            if n := text.count(w):
                bad += 1
                print(f"{f.relative_to(site)}: {w!r} x{n}")
    print(f"{bad} problems")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
