# SPDX-License-Identifier: Apache-2.0
"""tcsh's bracket ranges over every printable character: for each upper bound, the names
``[<\\x01>-<bound>]`` matches in a folder holding one file per printable character (``w5``).

    python3 -I t11.py <folder holding p2.csh and w5/>
"""

import os
import subprocess
import sys

S = sys.argv[1]
os.chdir(S + "/w5")
res = {}
for c in range(0x21, 0x7F):
    h = chr(c)
    if h in "/]^-{}\\":
        continue
    pat = "[\x01-" + h + "]"
    p = subprocess.run(
        ["tcsh", "-f", S + "/p2.csh", pat],
        capture_output=True,
        text=True,
        env={"PATH": "/usr/bin:/bin", "HOME": "/h", "LC_ALL": "C", "LANG": "C"},
        timeout=10,
    )
    words = [line[3:-1] for line in p.stdout.splitlines() if line.startswith("w=[")]
    res[h] = "".join(sorted(words, key=ord))
for h, w in res.items():
    print(repr(h), len(w), w)
