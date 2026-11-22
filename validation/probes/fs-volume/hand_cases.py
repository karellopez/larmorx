# SPDX-License-Identifier: Apache-2.0
"""Hand-written cases of the fs-volume probe (``check.py --cases <file>``), one group per finding.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (the cases are written for FreeSurfer's functions).

    python3 -P hand_cases.py lta > lta_hand_cases.txt
    python3 -P hand_cases.py dns <fs_volume_probe cases 7 4000> > dns_hand_cases.txt
    python3 -P hand_cases.py bvr > bvr_hand_cases.txt
    python3 -P hand_cases.py nan > nan_hand_cases.txt
    python3 -P hand_cases.py pv471 > pv471_hand_cases.txt
    python3 -P hand_cases.py fs508 > fs508_hand_cases.txt
    python3 -P hand_cases.py fs517 > fs517_hand_cases.txt


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

``nan``: the label sums' operand order (relay 43; track E's open group ``vrules11``): the five label
means (``lmeansp``: ``lmeans`` with the source painted) and ``MRIcomputeLabelNbhd`` (``lnbhdp``:
``lnbhd`` with the values painted) of 2 x 2 x 1 float volumes, one label, whose values hold two to
four NaN payloads, infinities of both signs (their sum the default NaN), a signalling NaN; the sum
keeps the last NaN value met in the walk's order (x outermost), the value just read being the
first operand.

``pv471``: ``MRIvoxelsInLabelWithPartialVolumeEffects`` with a negative limit (FS-471; track E's
open group ``vrules13``): a label (-200) below it on volumes whose counted voxels all lie off the
border (FreeSurfer finishes for -1 to -3, crashes from -4), on a volume whose counted voxels lie on
the border (a crash for every negative limit) and on a volume without the label.
``fs508``: ``LTAchangeType`` where FSL's conversion gives no matrix (FS-508, relay 44; track E's
open group ``vrules14``): one and two transforms of singular and regular tkregister, FSL, RAS and
voxel matrices, the first or the second failing, the second's source a geometry of voxel size 0,
the diagnostic level 0 and 1, every route to and from ``FSLREG_TYPE``. FreeSurfer frees the
matrix and goes on: ``hand_cases.py prefix <dump>`` checks that the port's lines (both modes) are
FreeSurfer's up to its first read of the freed matrix (``MatrixInverse: matrix (`` or
``MatrixPrint: unknown type``), then the port's FS-508 error.

``fs517``: ``MRIcropAroundCRS`` whose last voxel lies beyond ``int`` (FS-517; ``vrules15``): centres
of 2147483647, 1e12, -2147483648 and NaN with fields of view 0, 3 and 4; FreeSurfer crashes;
``prefix`` checks the port's lines up to the crash.
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
    with open(path, encoding="utf-8") as fh:
        base = [ln.split() for ln in fh if ln.startswith("dnsn ")]
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


def vol(w: int, h: int, d: int, ty: int = 3, seed: int = 0) -> str:
    """``get_vol``'s tokens: one frame, voxels allocated, unit voxels, an identity geometry,
    plain header fields, the cache recomputed."""
    one, zero = "3f800000", "00000000"
    geom = [
        "1",
        str(w),
        str(h),
        str(d),
        one,
        one,
        one,
        "1",
        *[one if i in (0, 4, 8) else zero for i in range(9)],
        *[zero] * 6,
        "-",
    ]
    head = ["0", "0", *[zero] * 6, one, one, zero, zero, zero, "0" * 16, "0", "1"]
    return " ".join([f"{w} {h} {d} {ty} 1 0 {seed}", *geom, *head])


def paints(vals: list[str], w: int = 2, h: int = 2) -> str:
    """``paint``'s tokens: the voxels of a w x h x 1 volume in the walk's order (x outermost)."""
    at = [(x, y) for x in range(w) for y in range(h)]
    return " ".join(
        [str(len(vals)), *[f"{x} {y} 0 {v}" for (x, y), v in zip(at, vals, strict=True)]]
    )


#: float bits: NaN payloads, a negative and a signalling NaN, infinities, ordinary numbers
NA, NB, NC, ND = "7fc0000a", "7fc0000b", "7fc0000c", "7fc0000d"
SNAN, NNEG, INF, NINF, ONE, TWO = (
    "7f800001",
    "ffc00005",
    "7f800000",
    "ff800000",
    "3f800000",
    "40000000",
)


def nan() -> list[str]:
    sets = [
        [ONE, NA, NB, NC],
        [ONE, NA, NB, ONE],
        [NA, NB, NC, ND],
        [INF, NINF, ONE, NA],
        [SNAN, ONE, NNEG, TWO],
        [NA, INF, NINF, ONE],
        [ONE, INF, NINF, TWO],
        [NNEG, NA, SNAN, ONE],
    ]
    labels = vol(2, 2, 1) + " " + paints([ONE] * 4)
    out = []
    for vals in sets:
        src = vol(2, 2, 1) + " " + paints(vals)
        for k in range(5):
            out.append(f"lmeansp {src} {labels} 1 {k} 0 0 0 1 1")
        for x, y, whalf in [(0, 0, 1), (1, 1, 1), (0, 0, 0), (1, 0, 2)]:
            out.append(f"lnbhdp {labels} 1 {src} {x} {y} 0 {whalf} 4")
    return out


def fill_paint(k_vals: list[str], painted: list[tuple[int, int, int, str]]) -> str:
    """``fill_paint``'s tokens: frames filled, then voxels painted (frame 0)."""
    return " ".join(
        [
            str(len(k_vals)),
            *k_vals,
            str(len(painted)),
            *[f"{x} {y} {z} 0 {v}" for x, y, z, v in painted],
        ]
    )


def pv471() -> list[str]:
    m200, ten, fifty, sixty, seven = "c3480000", "41200000", "42480000", "42700000", "40e00000"
    inner = vol(3, 2, 2) + " " + fill_paint([m200], [])
    cyc = [ten, fifty, sixty]
    ivals = (
        vol(3, 2, 2)
        + " "
        + fill_paint(
            [ten],
            [
                (x, y, z, cyc[(x + 3 * y + 6 * z) % 3])
                for z in range(2)
                for y in range(2)
                for x in range(3)
            ],
        )
    )
    out = []
    for m in (-1, -2, -3, -4, -5, -17):
        out.append(f"pvlab {inner} 1 {ivals} -200 {m} 0 0 0")
    for m in (-1, -3):
        out.append(f"pvlab {inner} 1 {ivals} -200 {m} 1 {vol(3, 2, 2)} 1 {vol(3, 2, 2)} 0")
    one = vol(1, 1, 1) + " " + fill_paint([m200], [])
    for m in (-1, -3, -4):
        out.append(f"pvlab {one} 1 {vol(1, 1, 1)} {fill_paint([seven], [])} -200 {m} 0 0 0")
    edge = vol(3, 1, 1) + " " + fill_paint([m200], [(1, 0, 0, "00000000")])
    evals = vol(3, 1, 1) + " " + fill_paint([ten], [(1, 0, 0, fifty), (2, 0, 0, sixty)])
    for m in (-1, -2, -4):
        out.append(f"pvlab {edge} 1 {evals} -200 {m} 0 0 0")
    absent = vol(3, 1, 1) + " " + fill_paint(["00000000"], [])
    out.append(f"pvlab {absent} 1 {evals} -200 -2 0 0 0")
    return out


def compare(path: str) -> list[str]:
    """The dumped lines of a hand-case run, FreeSurfer's against the port's ``reproduce``, with
    ``CRASH 6`` (the official child's abort) and ``EXIT 134`` (the port's) as one."""
    import json

    out, same = [], 0
    with open(path, encoding="utf-8") as fh:
        rows = [json.loads(x) for x in fh]
    for r in rows:
        a, b = r["official"], r["reproduce"]
        if a.startswith("CRASH 6 |") and b.startswith("EXIT 134 |"):
            a, b = a[len("CRASH 6 |") :], b[len("EXIT 134 |") :]
        same += a == b
        if a != b:
            out.append(f"DIFFER {r['case'][:80]}: {r['official'][:200]} / {r['reproduce'][:200]}")
    out.append(f"{same} of {len(rows)} identical with reproduce (CRASH 6 = EXIT 134)")
    return out


ZERO, ONE_F = "00000000", "3f800000"


def geom(w: int = 4, h: int = 4, d: int = 4, sizes=(ONE_F, ONE_F, ONE_F), c=(ZERO, ZERO, ZERO)):
    """``set_geom``'s 24 tokens: valid, identity direction cosines."""
    cos = [ONE_F if i in (0, 4, 8) else ZERO for i in range(9)]
    return ["1", str(w), str(h), str(d), *sizes, "1", *cos, *c, ZERO, ZERO, ZERO, "-"]


#: 4 x 4 matrices (row by row, float bits): the identity, a shift, a singular one (rank 3), zeros
M_I = [ONE_F if r == c else ZERO for r in range(4) for c in range(4)]
M_T = [
    *(ONE_F, ZERO, ZERO, "40000000"),
    *(ZERO, ONE_F, ZERO, "bf800000"),
    *(ZERO, ZERO, ONE_F, "3f000000"),
    *(ZERO, ZERO, ZERO, ONE_F),
]
M_S = [ONE_F if r == c and r != 2 else ZERO for r in range(4) for c in range(4)]
M_Z = [ZERO] * 16


def lta_toks(ty: int, xfs: list[tuple[list[str], list[str], list[str]]]) -> str:
    """``get_lta_full``'s tokens: ``(matrix, source, destination)`` per transform, no inverses."""
    t = [str(len(xfs)), str(ty), ZERO, "-"]
    for m, src, dst in xfs:
        t += [ZERO, ZERO, ZERO, ONE_F, "0", "0", *m, *src, *dst, "0"]
    return " ".join(t)


def fs508() -> list[str]:
    g1, g2 = geom(), geom(3, 5, 2, (ONE_F, ONE_F, "40000000"), (ONE_F, "40000000", "40400000"))
    g0 = geom(4, 4, 4, (ZERO, ONE_F, ONE_F))
    plans = [
        (14, 15, [(M_S, g1, g2)]),
        (14, 15, [(M_Z, g1, g2)]),
        (14, 15, [(M_S, g1, g2), (M_T, g2, g1)]),
        (14, 15, [(M_T, g1, g2), (M_S, g2, g1)]),
        (14, 15, [(M_S, g1, g2), (M_T, g0, g1)]),
        (14, 15, [(M_S, g1, g2), (M_S, g2, g1)]),
        (14, 15, [(M_T, g1, g2), (M_I, g2, g1), (M_S, g1, g1)]),
        (15, 14, [(M_S, g1, g2)]),
        (15, 14, [(M_T, g1, g2), (M_S, g2, g1)]),
        (15, 14, [(M_S, g1, g2), (M_T, g2, g1)]),
        (15, 1, [(M_S, g1, g2), (M_T, g2, g1)]),
        (15, 0, [(M_S, g1, g2)]),
        (0, 15, [(M_S, g1, g2)]),
        (1, 15, [(M_T, g1, g2), (M_S, g2, g1)]),
        (2, 15, [(M_S, g1, g2)]),
        (21, 15, [(M_S, g1, g2)]),
        (14, 0, [(M_S, g1, g2)]),
    ]
    out = []
    for frm, to, xfs in plans:
        for gd in (0, 1):
            out.append(f"ltachg 1 {gd} {lta_toks(frm, xfs)} {to}")
    return out


def fs517() -> list[str]:
    v = vol(4, 4, 1) + " " + fill_paint([ONE_F], [(1, 1, 0, "40000000")])
    d = {
        "big": "41dfffffffc00000",
        "e12": "426d1a94a2000000",
        "min": "c1e0000000000000",
        "nan": "7ff8000000000000",
        "one": "3ff0000000000000",
        "zero": "0000000000000000",
    }
    out = []
    plans = []
    # each axis beyond int: inside the volume on the other axes (the slice centre 0) or not
    for ax in range(3):
        for k, f in (("big", 3), ("nan", 0), ("e12", 4), ("min", 3), ("nan", 3), ("big", 1)):
            for other in ("zero", "one"):
                ctr = [other] * 3
                ctr[ax] = k
                fov = [1, 1, 1]
                fov[ax] = f
                plans.append((tuple(ctr), tuple(fov)))
    plans.append((("nan", "nan", "nan"), (3, 3, 3)))
    plans.append((("nan", "nan", "nan"), (0, 0, 0)))
    for ctr, fov in plans:
        c = " ".join(d[k] for k in ctr)
        out.append(f"ccrs {v} {c} {fov[0]} {fov[1]} {fov[2]} 0")
        out.append(f"ccrs {v} {c} {fov[0]} {fov[1]} {fov[2]} 2 1 2")
    return out


#: what FreeSurfer prints at its first read of a freed matrix (FS-508; memory contents: the
#: allocator has reused it): the fill's inverse of a matrix whose sizes are not square, or of a
#: square one it finds singular (the fill's own end), or the diagnostic print of one (an unknown
#: type, or rows of no numbers). The port reproduces every earlier transform's fill, so an end
#: there is the read's.
READS = (
    "error: MatrixInverse: matrix (",
    "error: TransformInvert: xform noninvertible",
    "error: MatrixPrint: unknown type",
    "error: MatrixPrint: mat = ",
    ";\\x0a",
)


def prefix(path: str) -> list[str]:
    """The dumped lines of an FS-508 or FS-517 hand-case run. Where the port notes FS-508, its text
    must be FreeSurfer's up to FreeSurfer's first read of the freed matrix (``READS``), followed by
    the port's error; FS-517, FreeSurfer's text must begin with the port's before its error.
    Elsewhere ``reproduce`` must be FreeSurfer's line, and ``fix`` too unless it notes a defect."""
    import json

    out, same, held, noted = [], 0, 0, 0
    with open(path, encoding="utf-8") as fh:
        rows = [json.loads(x) for x in fh]
    for r in rows:
        a = r["official"].partition(" | ")[2]
        for mode, ids in (("reproduce", r["ids"]), ("fix", r["fids"])):
            res, _, b = r[mode].partition(" | ")
            if not ({"FS-508", "FS-517"} & set(ids)):
                if r[mode] == r["official"]:
                    same += 1
                elif mode == "fix" and ids:
                    noted += 1
                else:
                    out.append(
                        f"DIFFER {mode} {r['case'][:60]}: {r['official'][:300]} / {r[mode][:300]}"
                    )
                continue
            pre = b.rpartition("error: ")[0]
            if "FS-508" in ids:
                ok = a.startswith(pre) and a[len(pre) :].startswith(READS)
            else:
                ok = a.startswith(pre)
            ok = ok and res == "EXIT 252"
            held += ok
            if not ok:
                out.append(f"ORDER {mode} {r['case'][:60]}: {a[:400]} / {b[:400]}")
    out.append(
        f"{same} identical, {noted} fix lines with a notice, {held} held to FreeSurfer's first read"
        f" or crash (of {2 * len(rows)} lines, both modes)"
    )
    return out


GROUPS = {
    "lta": lta,
    "dns": dns,
    "bvr": bvr,
    "nan": nan,
    "pv471": pv471,
    "fs508": fs508,
    "fs517": fs517,
    "compare": compare,
    "prefix": prefix,
}

if __name__ == "__main__":
    for line in GROUPS[sys.argv[1]](*sys.argv[2:]):
        print(line)
