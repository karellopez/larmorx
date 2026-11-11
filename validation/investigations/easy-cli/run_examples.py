# SPDX-License-Identifier: Apache-2.0
"""Runs a document's example as a user types it: the copyable block after a marker, line by line,
in an interactive bash or zsh, and checks that every line ended with exit status 0.

    python -P run_examples.py --doc README.md --marker "**An example, start to end**" \\
        --lang bash --shell zsh --work <new folder> [--python <python3.12>] \\
        [--replace "pip install larmorx=pip install <wheel>"]

The block's lines are fed to `zsh -f -i` (or `bash --norc --noprofile -i`) through a terminal
(`script`), each followed by a line that prints its exit status, in a clean environment: a new
home folder in <work>, `PATH` the system's folders with only a `python3` (``--python``, Python
3.12 or newer, as the docs ask) in front, no FSL or larmorx variables. zsh is interactive, so its
rule that a `#` typed at the prompt starts no comment applies (INTERACTIVE_COMMENTS off; the
session prints a probe that shows it). ``--replace old=new`` changes one typed line (a wheel of the
branch for `pip install larmorx`, which would install the release on PyPI). The session's
transcript is <work>/session.log; the last line: EXAMPLE <shell> <lines> lines, <failed> failed.
"""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
from pathlib import Path

_FENCE = re.compile(r"^\s*(`{3,}|~{3,})\s*([\w+-]*)")


def block_after(text: str, marker: str, lang: str) -> list[str]:
    """The lines of the first fenced block of ``lang`` after the line holding ``marker``."""
    lines = text.split("\n")
    start = next(i for i, line in enumerate(lines) if marker in line)
    inside, out = False, []
    for line in lines[start:]:
        m = _FENCE.match(line)
        if m and not inside and m.group(2) == lang:
            inside = True
            continue
        if m and inside:
            return out
        if inside:
            out.append(line)
    raise SystemExit(f"no {lang} block after {marker!r}")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--doc", required=True, type=Path)
    ap.add_argument("--marker", required=True)
    ap.add_argument("--lang", default="bash")
    ap.add_argument("--shell", required=True, help="the shell program (bash, or a zsh's path)")
    ap.add_argument("--work", required=True, type=Path)
    ap.add_argument("--python", default=sys.executable)
    ap.add_argument("--replace", action="append", default=[])
    a = ap.parse_args()
    typed = block_after(a.doc.read_text(encoding="utf-8"), a.marker, a.lang)
    for r in a.replace:
        old, new = r.split("=", 1)
        assert typed.count(old) == 1, f"{old!r} is not one typed line"
        typed[typed.index(old)] = new
    a.work.mkdir(parents=True, exist_ok=False)
    home = a.work / "home"
    shim = a.work / "shim"
    home.mkdir()
    shim.mkdir()
    (shim / "python3").symlink_to(Path(a.python).resolve())
    zsh = "zsh" in Path(a.shell).name
    session = [f"cd {a.work}", "echo zsh-comment-probe # visible-in-zsh"]
    for n, line in enumerate(typed, 1):
        session += [line, f'echo "@@STATUS $? :: {n}"']
    session.append("exit")
    feed = a.work / "typed.txt"
    feed.write_text("\n".join(session) + "\n", encoding="utf-8")
    shell = f"{a.shell} -f -i" if zsh else f"{a.shell} --norc --noprofile -i"
    env = {
        "HOME": str(home),
        "PATH": f"{shim}:/usr/local/bin:/usr/bin:/bin",
        "TERM": "dumb",
        "LANG": "C.UTF-8",
        "PIP_CACHE_DIR": str(a.work.parent / "pip-cache"),
        "PIP_DISABLE_PIP_VERSION_CHECK": "1",
        "TMPDIR": os.environ.get("TMPDIR", "/tmp"),
    }
    log = a.work / "session.log"
    with feed.open("rb") as stdin, log.open("wb") as out:
        subprocess.run(
            ["script", "-q", "-e", "-c", shell, "/dev/null"],
            stdin=stdin,
            stdout=out,
            stderr=subprocess.STDOUT,
            env=env,
            timeout=3600,
            check=False,
        )
    text = log.read_text(encoding="utf-8", errors="replace").replace("\r", "")
    status = {
        int(m.group(2)): int(m.group(1))
        for m in re.finditer(r"(?m)^@@STATUS (\d+) :: (\d+)$", text)
    }
    failed = [n for n in range(1, len(typed) + 1) if status.get(n) != 0]
    for n in failed:
        print(f"line {n} exit {status.get(n, 'missing')}: {typed[n - 1]}")
    if zsh:
        probe = "zsh-comment-probe # visible-in-zsh" in text
        print(f"zsh's comment rule active (the probe's '#' printed): {probe}")
        failed += [] if probe else [0]
    print(f"EXAMPLE {Path(a.shell).name} {len(typed)} lines, {len(failed)} failed")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
