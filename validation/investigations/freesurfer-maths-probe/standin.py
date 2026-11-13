# SPDX-License-Identifier: Apache-2.0
"""A stand-in candidate that checks ``compare.py``'s own rules (README.md, "What may differ"): it
prints the probe's lines of a batch as ``compare.py refs --save DIR`` saved them (the plain run),
changed as the spec allows a clean room to differ: where FreeSurfer never returned (``TIMEOUT``) in
the groups ``eigen`` and ``svd``, the spec's refusal (§17); in the group ``svd``, on the lines whose
input holds an infinite or NaN number, every printed NaN with its sign flipped (§17.4). With
``--break refusal`` the refusal lines carry no message, with ``--break nanbits`` the NaNs of the
group ``eigen`` are flipped too: ``compare.py compare`` must then report unexplained differences.

    standin.py --save DIR [--break refusal|nanbits] batch <file>

It reads no FreeSurfer code and runs nothing: the saved lines are the probe's own outputs.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

from compare import nonfinite_input  # noqa: E402

REFUSAL = {
    "eigen": b"error: OpenEigenSystem: VNL's eigensystem never returns on this matrix (an infinite or NaN entry)\n",
    "svd": (
        b"/home/rd521/4.13.2_ubuntu20/itk/4.13.2/src/ITK/Modules/ThirdParty/VNL/src/vxl/core/vnl/algo/"
        b"vnl_svd.hxx: suspicious return value (1) from SVDC\n"
    ),
}


def flip_nans(line: bytes) -> bytes:
    res, sep, streams = line.rpartition(b" | ")

    def tok(m: re.Match[bytes]) -> bytes:
        v = int(m.group(0), 16)
        if v & 0x7F800000 == 0x7F800000 and v & 0x7FFFFF:
            return f"{v ^ 0x80000000:08x}".encode()
        return m.group(0)

    return re.sub(rb"\b[0-9a-f]{8}\b", tok, res) + sep + streams


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--save", type=Path, required=True)
    ap.add_argument("--break", dest="brk", choices=["refusal", "nanbits"])
    ap.add_argument("command", choices=["batch"])
    ap.add_argument("file", type=Path)
    a = ap.parse_args()
    name = a.file.stem
    group = name.split("-", 1)[0]
    src = a.file.read_text(encoding="latin-1").splitlines()
    saved = (a.save / f"{name}.0.txt").read_bytes().split(b"\n")[:-1]
    if len(saved) != len(src):
        raise SystemExit(f"{name}: {len(saved)} saved lines for {len(src)} cases")
    out = sys.stdout.buffer
    for case, line in zip(src, saved, strict=True):
        if line.startswith(b"TIMEOUT") and group in REFUSAL:
            msg = b"" if a.brk == "refusal" else REFUSAL[group]
            line = b"refused | x x" + msg.hex().encode()
        elif (group == "svd" and nonfinite_input(case)) or (
            group == "eigen" and a.brk == "nanbits"
        ):
            line = flip_nans(line)
        out.write(line + b"\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
