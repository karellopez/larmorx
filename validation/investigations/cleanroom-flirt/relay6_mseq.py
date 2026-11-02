# SPDX-License-Identifier: Apache-2.0
import sys
from pathlib import Path

lines = Path(sys.argv[1]).read_text().splitlines()
cur, cnt, other = "<start>", 0, 0
seq = []
for ln in lines:
    if ln.startswith(" >> ") or ln.startswith("After free"):
        seq.append((cur, cnt))
        cur, cnt = ln.strip()[:90], 0
    elif ln.startswith("Erroneous"):
        cnt += 1
seq.append((cur, cnt))
for c, k in seq:
    print(f"{k:4d}  {c}")
