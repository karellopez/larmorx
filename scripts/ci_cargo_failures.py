# SPDX-License-Identifier: Apache-2.0
"""Prints the failures in a ``cargo test`` log as GitHub workflow annotations.

GitHub serves job logs only to logged-in users, but annotations to anyone (CLAUDE.md, "Reading
CI results without a login"). For each failing test, its captured output (the panic message);
for a build error, the message and its source excerpt; otherwise the log's last lines.

Usage: ``ci_cargo_failures.py <cargo test log>``
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

#: GitHub shows at most 10 error annotations per step.
MAX_ANNOTATIONS = 10


def _escape(text: str) -> str:
    return text.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")


def _escape_property(text: str) -> str:
    """A property value (``title=``) also escapes ``:`` and ``,``: test names hold ``::``."""
    return _escape(text).replace(":", "%3A").replace(",", "%2C")


def failures(log: str) -> list[tuple[str, str]]:
    """(title, body) for each failing test and build error in ``log``."""
    lines = log.splitlines()
    found: list[tuple[str, str]] = []
    i = 0
    while i < len(lines):
        test = re.match(r"^---- (.+) stdout ----$", lines[i])
        if test:
            j = i + 1
            while j < len(lines) and not lines[j].startswith("---- ") and lines[j] != "failures:":
                j += 1
            found.append((f"test {test.group(1)}", "\n".join(lines[i + 1 : j]).strip()))
            i = j
            continue
        if re.match(r"^error(\[E\d+\])?: ", lines[i]):
            j = i + 1
            while j < len(lines) and lines[j].strip():
                j += 1
            found.append(("cargo test", "\n".join(lines[i:j])))
            i = j
            continue
        i += 1
    if not found:
        found.append(("cargo test", "\n".join(lines[-40:])))
    return found


def main(argv: list[str]) -> int:
    log = Path(argv[1]).read_text(encoding="utf-8", errors="replace")
    for title, body in failures(log)[:MAX_ANNOTATIONS]:
        print(f"::error title={_escape_property(title[:200])}::{_escape(body[:3000])}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
