# SPDX-License-Identifier: Apache-2.0
"""Runs compare.py over a list of cases; one line of summary per case .

Usage: batch.py <cases file>  (lines: <input> <mode> <options...>; # comments)
"""

import subprocess
import sys
from pathlib import Path

here = Path(__file__).resolve().parent
py = sys.executable
for line in Path(sys.argv[1]).read_text().splitlines():
    line = line.strip()
    if not line or line.startswith("#"):
        continue
    parts = line.split()
    p = subprocess.run([py, "-I", str(here / "compare.py"), *parts], capture_output=True, text=True)
    out = p.stdout + p.stderr
    lines = out.splitlines()
    bad = [
        x
        for x in lines
        if "differ" in x
        or "missing" in x
        or "only in" in x
        or "Traceback" in x
        or "dtype/shape" in x
    ]
    first = lines[0].replace(",", " ").split() if lines else []
    status = "OK " if not bad and len(first) > 4 and first[2] == first[4] else "BAD"
    print(f"{status} {line}  [{lines[0] if lines else 'no output'}]", flush=True)
    if status == "BAD":
        for x in lines[1:40]:
            print("     ", x)
