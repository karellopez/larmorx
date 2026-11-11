# SPDX-License-Identifier: Apache-2.0
"""The roadmap's change at the publication of 0.1.0rc1 (relay 5b's proposal, 2026-11-11; the
roadmap is the orchestrator's page), applied to a site checkout: the section "The first release"
says the release candidate is out, the table of completed steps gets the release's row at its
top, "Last updated" the day, and the front matter's description names the release after
0.1.0rc1. Each replacement must apply exactly once, or nothing is written. The same changes, dated
2026-11-12, are in the release part's HANDOFF-site.toml; this script takes the day the release is
actually published.

Usage: python -P handoff_roadmap.py <site checkout> <release date YYYY-MM-DD>
"""

import re
import sys
from pathlib import Path

OLD_SECTION = """## The first release {#release}

- A check on clean machines, without compilers or runtimes installed (Windows, a minimal Linux).
- The Windows runtime linked into the programs; smaller binaries.
- A release workflow: the wheels for the six platforms, built, tested and uploaded on a version tag; the package names reserved on PyPI.
- A page explaining why LARMORX is built in Rust with Python on top: why the original tools have no Windows version, what Rust gives and what the project's own rules add (its own maths functions, one fixed order for matrix arithmetic, the same digits on every platform), and the honest limits.
"""
NEW_SECTION = """## The first release {#release}

The release candidate 0.1.0rc1 is out ([installation](/installation/)). Release 0.1.0 follows once it installs cleanly on all six platforms.
"""
ROW = (
    "| {day} | **The first release: 0.1.0rc1, a release candidate.** The three packages on PyPI"
    " at one version: `larmorx` (Apache-2.0), `larmorx-nc` (the FSL replicas; the FSL Licence,"
    " non-commercial use only) and `larmorx-gpl` (tedana's `t2smap`; the GNU GPL version 3),"
    ' `pip install "larmorx[all]"` for all three; wheels for Linux, macOS and Windows on x86-64'
    " and ARM, built and tested on every platform by a release workflow on the version's tag,"
    " with Windows' C runtime built into the programs and smaller binaries; the standalone"
    " programs for each platform and two container images for Apptainer, Docker and Podman;"
    " every file with its SHA-256 and GitHub's record of its build; the files rehearsed on clean"
    " Linux systems and on Windows before the upload. With it, the pages"
    " [Installation](/installation/) and [Why Rust and Python](/why-rust-and-python/) |\n"
)
TABLE_TOP = "|---|---|\n"
FIRST_ROW = "| 2026-11-10 | **Field maps, part B"
UPDATED = "**Last updated: 2026-11-10.**"
DESCRIPTION = (
    "the final validation phase, the first release, and every step completed so far with its date.",
    "the final validation phase, the release after 0.1.0rc1, and every step completed so far with"
    " its date.",
)


def main(argv: list[str]) -> int:
    if len(argv) != 2 or not re.fullmatch(r"\d{4}-\d{2}-\d{2}", argv[1]):
        raise SystemExit(__doc__)
    site, day = Path(argv[0]), argv[1]
    path = site / "content/roadmap.md"
    text = path.read_text(encoding="utf-8")
    for needle in (OLD_SECTION, TABLE_TOP + FIRST_ROW, UPDATED, DESCRIPTION[0]):
        if text.count(needle) != 1:
            raise SystemExit(f"content/roadmap.md: not found exactly once: {needle[:80]!r}")
    text = text.replace(OLD_SECTION, NEW_SECTION)
    text = text.replace(TABLE_TOP + FIRST_ROW, TABLE_TOP + ROW.format(day=day) + FIRST_ROW)
    text = text.replace(UPDATED, f"**Last updated: {day}.**")
    text = text.replace(*DESCRIPTION)
    path.write_text(text, encoding="utf-8")
    print("roadmap: the release section, the completed row, last updated", day)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
