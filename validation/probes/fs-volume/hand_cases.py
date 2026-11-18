# SPDX-License-Identifier: Apache-2.0
"""Hand-written cases of the fs-volume probe (``check.py --cases <file>``), one group per finding.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (the cases are written for FreeSurfer's functions).

    python3 -P hand_cases.py lta > lta_hand_cases.txt
    python3 -P hand_cases.py dns <fs_volume_probe cases 7 4000> > dns_hand_cases.txt
    python3 -P hand_cases.py bvr > bvr_hand_cases.txt


``lta``: ``.xfm`` files whose comment loop after the second ``%`` line meets the end of the file
(FS-307's bounds, relay 38): a ``#`` comment, a blank or white-space line, a NUL byte or the end of
a long line read last ends the loop (``could not read row 1 ... (<that raw line>)``); a ``%`` line
read last loops forever (FreeSurfer's child is stopped by its alarm; the port notes FS-307).

``dns``: ``MRIdownsampleN`` by factors that are 0, -0, NaN, negative, infinite or tiny (FS-317,
FS-961, rule B: refused by default; ``reproduce`` as FreeSurfer), each on the first ``dnsn`` cases
of a seeded case file (``fs_volume_probe cases <seed> <n>``) without and with a destination: their
three factor tokens replaced (float bits in hex), everything else as generated.

``bvr``: ``ReadCachedBrainVolumeStats`` of cache lines that name a measure, hold no number and end in
white space (FS-314): FreeSurfer aborts, with ``std::length_error`` when a word of the line is
longer than 15 bytes, else ``std::bad_alloc``; ``reproduce`` writes the same two lines and ends with
134 (the official child: ``CRASH 6``). ``check.py`` skips FS-314's lines; ``--dump`` and
``hand_cases.py compare <dump>`` compare them (``CRASH 6`` and ``EXIT 134`` as one).
"""

from __future__ import annotations

import sys


def hx(b: bytes) -> str:
    return b.hex() if b else "-"


def lta() -> list[str]:
    h = b"MNI Transform File\n"
    tt = b"Transform_Type = Linear;\n"
    texts = [
        h + b"%a\n%b\n#c\n",
        h + b"%a\n%b\n\n",
        h + b"%a\n%b\n   \n",
        h + b"%a\n%b\n%c\n#d\n",
        h + b"%a\n%b\n#c",
        h + b"%a\n%b\n  # c\n",
        h + b"%a\n%b\n\t\n",
        h + b"%a\n%b\n#c\n\n#d\n",
        h + b"%a\n%b\n%c\n\n",
        h + b"%a\n%b\n\x00z\n",
        h + b"%a\n%b\n%" + b"x" * 1000 + b"\n",
        h + b"%a\n%b\n" + b" " * 1000 + b"\n",
        h + b"%a\n%b\n#c\n" + tt,
        h + b"%a\n%b\n#c\n" + tt + b"1 0 0 0\n",
        h + b"%a\n#b\n",
        h + b"%a\n%b\n",
        h + b"%a\n%b\n#c\n%d\n",
        h + b"%a\n%b\n%c",
    ]
    return [f"ltaread {hx(b'.xfm')} {hx(t)} 1 0" for t in texts]


#: (Fc, Fr, Fs) as float bits: 0, -0, NaN (quiet, negative, signalling), -1, -3, -0.5, +inf,
#: -inf, 1e-40 (subnormal), 1e-30, and the ordinary 2 beside them
FACTORS = [
    ("00000000", "40000000", "40000000"),
    ("40000000", "00000000", "40000000"),
    ("40000000", "40000000", "00000000"),
    ("80000000", "40000000", "40000000"),
    ("7fc00000", "40000000", "40000000"),
    ("40000000", "ffc00000", "40000000"),
    ("40000000", "40000000", "7fa00001"),
    ("bf800000", "40000000", "40000000"),
    ("40000000", "c0400000", "40000000"),
    ("40000000", "40000000", "bf000000"),
    ("7f800000", "40000000", "40000000"),
    ("ff800000", "40000000", "40000000"),
    ("000116c2", "40000000", "40000000"),
    ("0da24260", "40000000", "40000000"),
    ("bf800000", "bf800000", "bf800000"),
    ("00000000", "00000000", "00000000"),
]


def dns(path: str) -> list[str]:
    base = [ln.split() for ln in open(path, encoding="utf-8") if ln.startswith("dnsn ")]
    # the destination flag follows the source volume (the command, then a volume's 47 tokens)
    short = [t for t in base if t[48] == "0" and len(t) == 53][:2]
    long_ = [t for t in base if t[48] == "1" and len(t) == 100][:2]
    out = []
    for t in short + long_:
        for f in FACTORS:
            out.append(" ".join(t[:-4] + list(f) + t[-1:]))
    return out


def bvr() -> list[str]:
    m = "# Measure BrainSeg, BrainSegVol, "
    lines = [
        m + "x" * 14 + " \n",
        m + "x" * 15 + " \n",
        m + "x" * 16 + " \n",
        m + "x" * 20 + " \n",
        m + "x" * 100 + " \n",
        m + "x" * 16 + " y \n",
        "# Measure " + "q" * 30 + " BrainSeg, BrainSegVol, y \n",
        m + "y\t\n",
        m + "y\r\n",
        m + "y \n# Measure Mask, MaskVol, 7, mm^3\n",
        "BrainSeg, BrainSegVol \n",
        m + "123456789012345,\n",
        m + "x" * 16 + "\n",
    ]
    return [f"bvr 1 {hx(t.encode())}" for t in lines]


def compare(path: str) -> list[str]:
    """The dumped lines of a hand-case run, FreeSurfer's against the port's ``reproduce``, with
    ``CRASH 6`` (the official child's abort) and ``EXIT 134`` (the port's) as one."""
    import json

    out, same = [], 0
    rows = [json.loads(x) for x in open(path, encoding="utf-8")]
    for r in rows:
        a, b = r["official"], r["reproduce"]
        if a.startswith("CRASH 6 |") and b.startswith("EXIT 134 |"):
            a, b = a[len("CRASH 6 |") :], b[len("EXIT 134 |") :]
        same += a == b
        if a != b:
            out.append(f"DIFFER {r['case'][:80]}: {r['official'][:200]} / {r['reproduce'][:200]}")
    out.append(f"{same} of {len(rows)} identical with reproduce (CRASH 6 = EXIT 134)")
    return out


GROUPS = {"lta": lta, "dns": dns, "bvr": bvr, "compare": compare}

if __name__ == "__main__":
    for line in GROUPS[sys.argv[1]](*sys.argv[2:]):
        print(line)
