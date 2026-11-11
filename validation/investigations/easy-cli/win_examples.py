# SPDX-License-Identifier: Apache-2.0
"""Writes the PowerShell script that runs documents' PowerShell examples as a user types them, on
the Windows helper (`win_examples.sh` copies it there and runs it in Windows PowerShell 5.1).

    python -P win_examples.py --out <script.ps1> --python-dir <the Python 3.12 folder there> \\
        --run readme README.md "pip install larmorx=pip install files\\<wheel>" \\
        --run site <site>/content/installation.md

Each `--run <name> <document> [old=new ...]` runs the first powershell block after the line
holding "**An example, start to end**" in a new folder `<name>` beside the script, line by line
(each line as it is typed, `old=new` changing one line), after each line a status line
`@@STATUS <name> <n> <ok>`: a native program's line (python, pip, larmorx, curl.exe) is ok when
its exit status is 0, a PowerShell command's when `$?` is true. Before the lines (the user's own
setup, not typed): Python 3.12 first on `PATH`, as an installed Python is; pip's cache, the
temporary folder and Python's bytecode kept inside the script's folder, so that nothing outside
it is written. The last line: `@@END`.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from run_examples import block_after

MARKER = "**An example, start to end**"
NATIVE = {"python", "py", "pip", "larmorx", "lx", "curl.exe"}


def ps_quote(text: str) -> str:
    """``text`` as a PowerShell single-quoted string."""
    return "'" + text.replace("'", "''") + "'"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--out", required=True, type=Path)
    ap.add_argument("--python-dir", required=True)
    ap.add_argument("--run", nargs="+", action="append", required=True, metavar="ARG")
    a = ap.parse_args()
    out = [
        '$ErrorActionPreference = "Continue"',
        "$here = $PSScriptRoot",
        f'$env:PATH = "{a.python_dir};" + $env:PATH',
        '$env:PIP_CACHE_DIR = "$here\\pip-cache"',
        '$env:PIP_DISABLE_PIP_VERSION_CHECK = "1"',
        '$env:PYTHONDONTWRITEBYTECODE = "1"',
        'New-Item -ItemType Directory -Force "$here\\tmp" | Out-Null',
        '$env:TEMP = "$here\\tmp"',
        '$env:TMP = "$here\\tmp"',
        '"PowerShell $($PSVersionTable.PSVersion)"',
    ]
    for run in a.run:
        name, doc, *changes = run
        typed = block_after(Path(doc).read_text(encoding="utf-8"), MARKER, "powershell")
        for change in changes:
            old, new = change.split("=", 1)
            assert typed.count(old) == 1, f"{old!r} is not one typed line"
            typed[typed.index(old)] = new
        out += [
            f'New-Item -ItemType Directory "$here\\{name}" | Out-Null',
            f'Set-Location "$here\\{name}"',
            f'"@@RUN {name} {len(typed)}"',
        ]
        for n, line in enumerate(typed, 1):
            first = re.split(r"\s+", line.strip(), maxsplit=1)[0]
            native = first in NATIVE or first.endswith(".exe")
            out += ["$global:LASTEXITCODE = 0", line]
            ok = "($LASTEXITCODE -eq 0)" if native else "$?"
            out.append(f'"@@STATUS {name} {n} $({ok}) :: " + {ps_quote(line[:60])}')
        out += ["if (Get-Command deactivate -ErrorAction SilentlyContinue) { deactivate }"]
        out.append('Set-Location "$here"')
    out.append('"@@END"')
    a.out.write_text("\r\n".join(out) + "\r\n", encoding="utf-8-sig")
    print(f"wrote {a.out}: {len(a.run)} runs")
    return 0


if __name__ == "__main__":
    sys.exit(main())
