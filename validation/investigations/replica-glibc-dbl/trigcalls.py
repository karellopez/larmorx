# SPDX-License-Identifier: Apache-2.0
"""Lists, per function of an ELF binary, its calls to the C library's trigonometric functions.

Shows which of glibc's `sin`, `cos`, `sincos`, `sinf`, `cosf`, `sincosf`, `tan` and `cexp` an
original program's machine code calls in each routine: GCC merges `sin(a)` and `cos(a)` of one
argument into one `sincos` call, which is not the same function bit for bit (GLIBC-07). Run on the
AFNI 26.3.05 oracle's `3dTshift`, `3dAutomask` and `3dUnifize` (README.md).

    python -P validation/investigations/replica-glibc-dbl/trigcalls.py <binary> [<binary> ...]
"""

from __future__ import annotations

import re
import subprocess
import sys
from collections import Counter

PAT = re.compile(r"call\s+[0-9a-f]+ <((?:sin|cos|sincos|sinf|cosf|sincosf|cexp|tan)@[^>]*)>")


def main(paths: list[str]) -> int:
    for path in paths:
        print("==", path)
        text = subprocess.run(
            ["objdump", "-d", "--no-show-raw-insn", "-C", path],
            capture_output=True,
            text=True,
            check=True,
        ).stdout
        for f in re.split(r"\n(?=[0-9a-f]+ <[^>]+>:\n)", text):
            m = re.match(r"([0-9a-f]+) <([^>]+)>:", f)
            if not m:
                continue
            calls = PAT.findall(f)
            if calls:
                print(" ", m.group(2)[:110], dict(Counter(calls)))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
