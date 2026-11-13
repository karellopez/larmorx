# SPDX-License-Identifier: Apache-2.0
"""A stand-in candidate for the comparison loop's own check of its fix-mode checks (README.md,
"What may differ"): the probe's output, corrected from the probe's output alone, never from the
loop's code. FS-12: a rounding that printed -2147483648 prints 0 for a NaN input and 2147483647 for
a positive one, with a notice on standard error; FS-01: the NUL byte before a newline removed. With
``--break nul`` a byte stays in the NUL's place; with ``--break sat`` a NaN rounds to 2147483647: the
loop must refuse both.

    python -P standin.py make OUT --probe P [--break nul|sat]   write the candidate OUT
    python -P compare.py compare --probe P --candidate OUT --scratch DIR --groups numbers --mode fix
"""

from __future__ import annotations

import argparse
import os
import shlex
import struct
import subprocess
import sys
from pathlib import Path


def run(probe: str, brk: str, args: list[str]) -> int:
    r = subprocess.run([probe, *args], capture_output=True)
    out, err = r.stdout, r.stderr
    if args[:1] == ["batch"] and os.environ.get("LARMORX_UPSTREAM_DEFECTS") == "fix":
        lines = Path(args[1]).read_text(encoding="latin-1").splitlines()
        res = out.decode("latin-1").split("\n")
        changed = False
        if len(res) == len(lines) + 1:
            for i, line in enumerate(lines):
                c, *a = line.split()
                if c in ("nint", "nint2") and res[i] == "-2147483648":
                    v = struct.unpack(">d", bytes.fromhex(a[0]))[0]
                    nan = "2147483647" if brk == "sat" else "0"
                    res[i] = nan if v != v else ("2147483647" if v > 0 else res[i])
                    changed = changed or res[i] != "-2147483648"
            out = "\n".join(res).encode("latin-1")
        err = err.replace(b"\x00\n", b"?\n" if brk == "nul" else b"\n")
        if changed:
            err = b"notice: FS-12 corrected\n" + err
    sys.stdout.buffer.write(out)
    sys.stderr.buffer.write(err)
    return r.returncode if r.returncode >= 0 else 128 - r.returncode


def main() -> int:
    if sys.argv[1:2] == ["run"]:
        return run(sys.argv[2], sys.argv[3], sys.argv[4:])
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("command", choices=["make"])
    ap.add_argument("out", type=Path)
    ap.add_argument("--probe", type=Path, required=True)
    ap.add_argument("--break", dest="brk", choices=["none", "nul", "sat"], default="none")
    a = ap.parse_args()
    argv = [
        sys.executable,
        "-I",
        str(Path(__file__).resolve()),
        "run",
        str(a.probe.resolve()),
        a.brk,
    ]
    a.out.write_text(
        "#!/bin/sh\nexec " + " ".join(shlex.quote(x) for x in argv) + ' "$@"\n', encoding="utf-8"
    )
    a.out.chmod(0o755)
    return 0


if __name__ == "__main__":
    sys.exit(main())
