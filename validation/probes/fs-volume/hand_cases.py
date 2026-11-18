# SPDX-License-Identifier: Apache-2.0
"""Hand-written cases of the fs-volume probe (``check.py --cases <file>``), one group per finding.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (the cases are written for FreeSurfer's functions).

    python3 -P hand_cases.py lta > lta_hand_cases.txt
    python3 -P hand_cases.py dns <fs_volume_probe cases 7 4000> > dns_hand_cases.txt


``lta``: ``.xfm`` files whose comment loop after the second ``%`` line meets the end of the file
(FS-307's bounds, relay 38): a ``#`` comment, a blank or white-space line, a NUL byte or the end of
a long line read last ends the loop (``could not read row 1 ... (<that raw line>)``); a ``%`` line
read last loops forever (FreeSurfer's child is stopped by its alarm; the port notes FS-307).

``dns``: ``MRIdownsampleN`` by factors that are 0, -0, NaN, negative, infinite or tiny (FS-317,
FS-961, rule B: refused by default; ``reproduce`` as FreeSurfer), each on the first ``dnsn`` cases
of a seeded case file (``fs_volume_probe cases <seed> <n>``) without and with a destination: their
three factor tokens replaced (float bits in hex), everything else as generated.
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


GROUPS = {"lta": lta, "dns": dns}

if __name__ == "__main__":
    for line in GROUPS[sys.argv[1]](*sys.argv[2:]):
        print(line)
