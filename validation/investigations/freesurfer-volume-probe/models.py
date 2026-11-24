# SPDX-License-Identifier: Apache-2.0
"""Statements of ``specs/freesurfer-volume.md`` checked on the prebuilt probe as models of the
printed voxels (black box: the inputs as compare.py makes them and the probe's printed results,
nothing else).

    python -P models.py fs291 --probe P --scratch DIR
        every ``invc`` and ``sam`` line of the groups vscale, vtwo and vrules against §9's and §10's
        statement of the byte walk (FS-291 under ``reproduce``): each row of slice z, row y of the
        first source's extent, its first ``width`` bytes as ``uchar`` voxels at the place of element 0
        of that row in each volume (a tensor volume's rows all at its first voxel, §6), the result
        written at the same place of the destination; the destination's printed bytes must be the
        model's. Lines where FreeSurfer crashed or read memory it does not own (§69's memory lines),
        and those whose walk leaves a volume's rows (FS-290), are counted apart.

    python -P models.py skull --probe P --scratch DIR
        every line of the group vskull against §20's walks (and §15's centre of mass), from the
        inputs as compare.py makes them: the printed box, or the end of the program, and the
        messages on standard output.

    python -P models.py cma --probe P --scratch DIR
        every ``cmalab`` line of the group vcma against §23's tables: the label tests (each a set of
        numbers, ranges or a combination, compared in the value's own type, a NaN equal to nothing),
        ``IS_CLASS``, ``IsSubCorticalGray`` and ``MRIasegContraLatLabel`` with its message.

    python -P models.py view --probe P --scratch DIR
        every ``toimgv`` line of the groups vimage and vrules9 without a given image and with the
        frame inside, of the types the nearest sample reads, against §34's steps: the slice
        direction, the view's table, the slice, the extremes from 1e7 and -1e7, the range's check,
        the bytes and the image's voxel sizes.

    python -P models.py smooth --probe P --scratch DIR
        every ``smpar`` line of the groups vnbhd and vrules9 on shorts against §35's statement of
        MRIsmoothParcellation (negative labels counted like any other: FS-467 as the default and
        ``reproduce`` give it).

    python -P models.py nearest --probe P --scratch DIR
        every ``nearnz`` line of the group vnbhd against §35's MRIfindNearestNonzeroLocation.

    python -P models.py physvox --probe P --scratch DIR
        60 transform arrays changed from physical-voxel to RAS-to-RAS (``ltachg``) against §54's
        ``reproduce`` products, with the geometries' matrices taken from the probe's ``vgi2r`` and
        ``vgr2i`` (the arrays: seeded, valid geometries, general 4 x 4 matrices).

    python -P models.py region --probe P --scratch DIR [--candidate C]
        every ``rgbb`` line of the groups vregion, vrules17, vrules18 and vrules20 against §43's statements:
        the mask's extremes (frame 0, magnitudes not below 1e-5, a NaN counted), FreeSurfer's box
        and REGIONgetBoundingBoxEqOdd's steps and lines; the probe must print FreeSurfer's box. With
        ``--candidate``, the candidate's lines too: with ``reproduce`` FreeSurfer's box, with ``fix``
        the box FS-269's decision gives (one more voxel on each axis where FreeSurfer's box ends
        before the mask's last voxel), notices taken out of standard error.

    python -P models.py cov --probe P --scratch DIR [--candidate C]
        every ``cov`` line of the groups vcov and vrules19 whose types it decodes and whose mask has
        the volume's extent against §42's MRIcovarianceMatrix (the sums of ``v2 v1`` in ``double``
        over the voxels in order, divided by the count, as ``float``; a NaN's payload is left out);
        with ``--candidate``, the candidate's lines with ``reproduce`` (FreeSurfer's count) and
        ``fix`` (FS-483's decided count: the voxels the sums take).

    python -P models.py bright --probe P --scratch DIR [--candidate C]
        every ``fbnwm`` line of the groups vbright and vrules24 against §59's FS-1233 (a T1 or a
        white-matter volume not of bytes): the default's result is the probe's own ``fbnwm`` of the
        byte volumes §59 names (the T1's values as FS-802's default takes them, the white matter's
        truncated and clamped to 0..255), with, for a T1 not of bytes, its labels written by value
        into the probe's ``clone`` of the T1; counts the lines with FS-1233 and those whose default
        result differs from FreeSurfer's. With ``--candidate``, the candidate's lines: with
        ``reproduce`` the probe's, with ``fix`` the model's (notices taken out of standard error).

The probe runs through the oracle-output cache (one thread, the family ``freesurfer``) when the
validation package is importable (``PYTHONPATH=validation/src``), else directly. Exit status 1 when a
line differs from its model.
"""

from __future__ import annotations

import argparse
import importlib.util
import math
import struct
import sys
from collections import Counter
from dataclasses import dataclass
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent


def _compare():
    spec = importlib.util.spec_from_file_location("fs_volume_compare", HERE / "compare.py")
    mod = importlib.util.module_from_spec(spec)
    sys.modules["fs_volume_compare"] = mod
    spec.loader.exec_module(mod)
    return mod


C = _compare()

#: Bytes per voxel of the types with a size (README.md's types).
BPV = {0: 1, 1: 4, 2: 8, 3: 4, 4: 2, 6: 4, 7: 8, 9: 4, 10: 2}


def table() -> list[int]:
    """The README's table of floats for the ``t<seed>`` fill (its "Encodings")."""
    text = (HERE / "README.md").read_text(encoding="utf-8")
    row = next(ln for ln in text.splitlines() if ln.startswith("| the table |"))
    return [int(h, 16) for h in row.split("`")[1].split()]


@dataclass
class Vol:
    w: int
    h: int
    d: int
    t: int
    f: int
    hdr: int
    buf: bytearray

    def row(self, z: int, y: int) -> int | None:
        """The byte place of element 0 of row y of slice z (None: a row the volume does not have)."""
        if not (0 <= z < self.d * self.f and 0 <= y < self.h) or self.hdr:
            return None
        if self.t == 6:
            return 0
        return (z * self.h + y) * self.w * BPV[self.t]


def make(params: tuple, tab: list[int]) -> Vol:
    w, h, d, t, f, hdr, fill = params
    n = w * h * d * f * BPV.get(t, 0) if not hdr else 0
    buf = bytearray(max(n, 0))
    if isinstance(fill, str) and fill.startswith("t"):
        s = int(fill[1:])
        for i in range(len(buf) // 4):
            buf[4 * i : 4 * i + 4] = struct.pack("<I", tab[(7 * i + s) % len(tab)])
    elif int(fill) >= 0:
        s = int(fill)
        for i in range(len(buf)):
            buf[i] = (s + 101 * i) % 256
    return Vol(w, h, d, t, f, hdr, buf)


def recording_gen() -> tuple[object, dict[str, tuple]]:
    """A compare.py generator whose working volumes are recorded with their shape and fill."""
    seen: dict[str, tuple] = {}
    gen = C.Gen4b(7)
    orig = gen.wvol

    def wvol(r, shape=None, special=0.05, ct=0.1, **kw):
        s = orig(r, shape=shape, special=special, ct=ct, **kw)
        p = s.split()
        seen[s] = (*map(int, p[:6]), p[6])
        return s

    gen.wvol = wvol
    orig_fixed = C.fixed_w

    def fixed(w, h, d, t, f=1, hdr=0, fill=5):
        s = orig_fixed(w, h, d, t, f, hdr, fill)
        seen[s] = (w, h, d, t, f, hdr, str(fill))
        return s

    C.fixed_w = fixed
    return gen, seen


def split(rest: str, seen: dict[str, tuple]) -> tuple[str, str]:
    """The first working volume (the longest recorded W ``rest`` starts with) and what follows."""
    best = max((s for s in seen if rest.startswith(s + " ")), key=len)
    return best, rest[len(best) + 1 :]


def walk(src: Vol, dsts: list[Vol], op) -> bool:
    """The byte walk of §9: False when it leaves a volume's rows or buffer (FS-290)."""
    for z in range(src.d):
        for y in range(src.h):
            places = [v.row(z, y) for v in dsts]
            if any(p is None for p in places):
                return False
            for x in range(src.w):
                if any(p + x >= len(v.buf) for p, v in zip(places, dsts, strict=True)):
                    return False
                op([(v, p + x) for p, v in zip(places, dsts, strict=True)])
    return True


def nint_byte(o: float) -> int:
    if o != o:  # FS-12 under reproduce: the int's lowest value, whose low byte is 0
        return 0
    return (int(o + 0.5) if o >= 0 else int(o - 0.5)) & 0xFF


def fs291(runner, scratch: Path) -> int:
    gen, seen = recording_gen()
    tab = table()
    batches = [b for g in ("vscale", "vtwo", "vrules") for b in getattr(gen, g)()]
    tally: Counter[str] = Counter()
    bad = 0
    for b in batches:
        path = C.write_batch(scratch, b)
        plain, memory = runner.probe_runs(b, path)
        for line, out, mem in zip(b.lines, plain, memory, strict=True):
            cmd, _, rest = line.partition(" ")
            if cmd not in ("invc", "sam"):
                continue
            res = out.decode("latin-1").rpartition(" | ")[0]
            if mem or res.startswith(("CRASH", "EXIT", "TIMEOUT")):
                tally["memory or crash"] += 1
                continue
            a_s, rest = split(rest, seen)
            a = make(seen[a_s], tab)
            if cmd == "sam":
                sc, rest = rest.split(" ", 1)
                scale = np.frombuffer(bytes.fromhex(sc)[::-1], dtype=np.float32)[0]
                b_s, rest = split(rest, seen)
                bv = make(seen[b_s], tab)
                ds = rest
            else:
                ds, _, th = rest.rpartition(" ")
                thr = struct.unpack(">f", bytes.fromhex(th))[0]
            if ds == "new":
                if cmd == "invc":  # a clone: the source's extent, type and frames
                    dst = make((a.w, a.h, a.d, a.t, a.f, 0, "-1"), tab)
                else:  # one frame of the first source's extent and type
                    dst = make((a.w, a.h, a.d, a.t, 1, 0, "-1"), tab)
            elif ds == "self":
                dst = a
            elif ds == "other":
                dst = bv
            else:
                dst = make(seen[ds], tab)
            types = {a.t, dst.t} | ({bv.t} if cmd == "sam" else set())
            kind = "uchar only" if types == {0} else ("a tensor" if 6 in types else "other types")
            if cmd == "invc":

                def op(at, thr=thr):
                    (va, pa), (vd, pd) = at
                    v = va.buf[pa]
                    if float(v) > thr:
                        v = 255 - v
                    vd.buf[pd] = v

                ok = walk(a, [a, dst], op)
            else:

                def op(at, scale=scale):
                    (va, pa), (vb, pb), (vd, pd) = at
                    with np.errstate(all="ignore"):  # IEEE 754 single precision
                        o = np.float32(va.buf[pa]) * (np.float32(vb.buf[pb]) / scale)
                    o = 255.0 if o > 255 else (0.0 if o < 0 else float(o))
                    vd.buf[pd] = nint_byte(o)

                ok = walk(a, [a, bv, dst], op)
            if not ok:
                tally["leaves a volume's rows"] += 1
                continue
            printed = res.split()[-1]
            if printed == "x" + dst.buf.hex():
                tally[f"as the model ({kind})"] += 1
            else:
                bad += 1
                if bad <= 3:
                    print(
                        f"DIFFERS: {line[:120]}\n  probe {printed[:120]}\n  model x{dst.buf.hex()[:119]}"
                    )
    for k, v in sorted(tally.items()):
        print(f"fs291: {k}: {v}")
    print(f"fs291: differ from the model: {bad}")
    return 1 if bad else 0


def nint(x: float) -> int:
    """FreeSurfer's rounding (half away from zero) of a finite double."""
    return int(x + 0.5) if x >= 0 else int(x - 0.5)


def skull_volume(params: tuple, boxes: list[tuple], tab: list[int]) -> tuple[list[float], tuple]:
    """The volume's frame 0 as floats after the painted boxes (§5's writes)."""
    v = make(params, tab)
    w, h, d, t = v.w, v.h, v.d, v.t
    n = w * h * d
    if t == 0:
        vals = [float(b) for b in v.buf[:n]]
    elif t == 4:
        vals = [float(x) for x in struct.unpack(f"<{n}h", bytes(v.buf[: 2 * n]))]
    else:
        with np.errstate(invalid="ignore"):  # a signalling NaN of the fill, quieted
            vals = list(np.frombuffer(bytes(v.buf[: 4 * n]), dtype="<f4").astype(np.float64))
    for x0, y0, z0, x1, y1, z1, val in boxes:
        if t == 3:
            stored = val
        else:
            lo, hi = (0.0, 255.0) if t == 0 else (-32768.0, 32767.0)
            c = val
            if c < lo:
                c = lo
            if c > hi:
                c = hi
            k = -(2**31) if c != c else nint(c)  # FS-12 under reproduce
            k &= 0xFF if t == 0 else 0xFFFF
            stored = float(k if t == 0 or k < 0x8000 else k - 0x10000)
        for z in range(z0, z1 + 1):
            for y in range(y0, y1 + 1):
                for x in range(x0, x1 + 1):
                    vals[x + y * w + z * w * h] = stored
    return vals, (w, h, d)


def skull_model(vals: list[float], dims: tuple, thresh: int) -> tuple[str, list[str]]:
    """§20's walks (and §15's centre of mass): the printed box, or EXIT 252, and the messages."""
    w, h, d = dims
    tb = thresh & 0xFF
    wt = mx = my = mz = 0.0
    for z in range(d):
        for y in range(h):
            for x in range(w):
                v = vals[x + y * w + z * w * h]
                if v > tb:
                    wt += v
                    mx += float(np.float32(x)) * v
                    my += float(np.float32(y)) * v
                    mz += float(np.float32(z)) * v
    cx, cy, cz = (mx / wt, my / wt, mz / wt) if wt > 0 else (0.0, 0.0, 0.0)

    def sample(x: int, y: int, z: int) -> float:
        if 0 <= x < w and 0 <= y < h and 0 <= z < d:
            return vals[x + y * w + z * w * h]
        return 0.0  # outside_val

    def walk(c: int, step: int, end: int, at, left: bool = False, top: bool = False) -> int:
        n, s, b, longest, e, m = 0, c, 0, 0, c, 0
        p = c
        while (p >= 0) if step < 0 else (p < end):
            v = at(p)
            if v < thresh:
                if top and b > m:
                    m = b
                if n == 0:
                    s = p
                n += 1
                b = 0
            else:
                b += 1
                if b > 30:
                    longest = 0
                if (n > longest) if left else (n >= longest):
                    longest, e = n, s
                    m = 0
                n = 0
            p += step
        if n > longest or (top and n > longest // 2 and m > 15):
            longest, e = n, s
        if longest < 10:
            e = 0 if step < 0 else end - 1
        return e

    msgs = []
    while True:
        y, z = nint(cy), nint(cz)
        bx = walk(nint(cx), -1, w, lambda p, y=y, z=z: sample(p, y, z), left=True)
        right = walk(nint(cx), 1, w, lambda p, y=y, z=z: sample(p, y, z))
        bdx = right - bx + 1
        if bdx > 10:
            break
        cy -= 10
        msgs.append(
            f"left/right detection failed, moving y coord to {nint(cy) + 10} from {nint(cy)}"
        )
        if cy < 0:
            return "EXIT 252", msgs
    xt, z = max(0, nint(cx) - 20), nint(cz)
    by = walk(nint(cy), -1, h, lambda p: sample(xt, p, z), top=True)
    x = nint(cx)
    bdy = walk(nint(cy), 1, h, lambda p: sample(x, p, z)) - by + 1
    y = nint(cy)
    bz = walk(nint(cz), -1, d, lambda p: sample(x, y, p))
    bdz = walk(nint(cz), 1, d, lambda p: sample(x, y, p)) - bz + 1
    if bdz < 5:
        x2 = x - 15
        bz = walk(nint(cz), -1, d, lambda p: sample(x2, y, p))
        bdz = walk(nint(cz), 1, d, lambda p: sample(x2, y, p)) - bz + 1
    return f"0 {bx} {by} {bz} {bdx} {bdy} {bdz}", msgs


def skull(runner, scratch: Path) -> int:
    gen, seen = recording_gen()
    tab = table()
    tally: Counter[str] = Counter()
    bad = 0
    for b in gen.vskull():
        path = C.write_batch(scratch, b)
        plain, memory = runner.probe_runs(b, path)
        for line, out, mem in zip(b.lines, plain, memory, strict=True):
            w_s, rest = split(line.partition(" ")[2], seen)
            params = seen[w_s]
            tok = rest.split()
            thresh, n = int(tok[0]), int(tok[1])
            boxes = []
            for k in range(n):
                q = tok[2 + 7 * k : 9 + 7 * k]
                val = float(np.frombuffer(bytes.fromhex(q[6])[::-1], dtype=np.float32)[0])
                boxes.append((*map(int, q[:6]), val))
            vals, dims = skull_volume(params, boxes, tab)
            res, msgs = skull_model(vals, dims, thresh)
            text = out.decode("latin-1")
            got, _, streams = text.rpartition(" | ")
            so = bytes.fromhex(streams.split(" ")[0][1:]).decode("latin-1")
            want_so = "".join(m + "\n" for m in msgs)
            if mem:
                tally["memory"] += 1
            elif got.strip() == res and so == want_so:
                tally["as the model (" + ("exit" if res.startswith("EXIT") else "a box") + ")"] += 1
            else:
                bad += 1
                if bad <= 3:
                    print(
                        f"DIFFERS: {line[:100]}\n  probe {got[:80]} | {so[:120]!r}\n  model {res} | {want_so[:120]!r}"
                    )
    for k, v in sorted(tally.items()):
        print(f"skull: {k}: {v}")
    print(f"skull: differ from the model: {bad}")
    return 1 if bad else 0


# ---- §23's label tables (the spec's text, restated as data) ------------------------------------

#: Each label test of §23 in the probe's order: a set of numbers, a range (lo, hi), or a combination.
_SETS: dict[str, object] = {
    "IS_FUTURE_WMSA": {87, 88, 89},
    "IS_LH_LAYER": (270, 277),
    "IS_CC": (251, 255),
    "IS_RH_LAYER": (370, 377),
    "IS_LAYER": ("or", "IS_LH_LAYER", "IS_RH_LAYER"),
    "IS_UNKNOWN": {0, 255, 190, 191},
    "IS_BRAIN": "brain",
    "IS_WM": ("or", {2, 41, 187, 186, 28, 60}, "IS_CC", "IS_CEREBELLAR_WM"),
    "IS_CEREBRAL_WM": {2, 41},
    "IS_HYPO": ("or", {77, 78, 79}, "IS_FUTURE_WMSA"),
    "IS_WMSA": ("or", "IS_HYPO"),
    "IS_WMH": ("or", "IS_WM", "IS_HYPO"),
    "IS_THALAMUS": {10, 49},
    "IS_GM": {3, 42},
    "IS_VENTRAL_DC": {28, 60},
    "IS_CEREBELLAR_WM": {7, 46},
    "IS_CEREBELLAR_GM": {8, 47},
    "IS_CEREBELLUM": ("or", "IS_CEREBELLAR_WM", "IS_CEREBELLAR_GM"),
    "IS_CHOROID": {31, 63},
    "IS_HIPPO": {17, 53},
    "IS_AMYGDALA": {18, 54},
    "IS_MTL": ("or", "IS_HIPPO", "IS_AMYGDALA"),
    "IS_CORTEX": {3, 42},
    "IS_LAT_VENT": {4, 43, 44, 5},
    "IS_CSF": ("or", "IS_LAT_VENT", {24, 122, 14, 15}),
    "IS_INF_LAT_VENT": {5, 44},
    "IS_VENTRICLE": ("or", "IS_LAT_VENT", {14, 15}),
    "IS_CAUDATE": {11, 50},
    "IS_PUTAMEN": {12, 51},
    "IS_CLAUSTRUM": {138, 139},
    "IS_ACCUMBENS": {26, 58},
    "IS_PALLIDUM": {13, 52},
    "LABEL_WITH_NO_TOPOLOGY_CONSTRAINT": {82, 81, 78, 79, 31, 63, 77, 98, 129, 122, 118, 120, 80},
    "IS_FIMBRIA": {553, 503, 212},
    "IS_GRAY_MATTER": {3, 17, 18, 12, 11, 8, 26, 42, 53, 54, 51, 50, 47, 58},
    "IS_WHITE_MATTER": {7, 2, 46, 41},
    "IS_FLUID": {4, 5, 43, 44, 0},
    "IS_LH_CLASS": {3, 2, 17, 18, 12, 13, 11, 4, 5, 8, 7, 10, 30, 31, 28, 26},
    "IS_RH_CLASS": {42, 41, 53, 54, 51, 52, 50, 43, 44, 47, 46, 49, 62, 63, 60, 58},
    "IS_GRAY_CLASS": {3, 42, 11, 50},
    "IS_WHITE_CLASS": {2, 41},
    "IS_CSF_CLASS": {4, 43, 24, 122, 14, 15, 72, 555, 505, 215},
}
#: IS_CLASS(l, c) for the classes 0, 1, 2 and 7: CSF, gray, white, white
_CLASSES = ["IS_CSF_CLASS", "IS_GRAY_CLASS", "IS_WHITE_CLASS", "IS_WHITE_CLASS"]


def labtest(name: str, x: float) -> bool:
    """§23's test `name` of the value x (an int, a float or a double: compared as a real number, a
    NaN equal to nothing and neither smaller nor larger)."""
    d = _SETS[name]
    if d == "brain":
        return (not labtest("IS_UNKNOWN", x) and x < 98) or labtest("IS_CC", x)
    if isinstance(d, set):
        return x in d if x == x else False
    if isinstance(d, tuple) and d and d[0] == "or":
        return any(labtest(p, x) if isinstance(p, str) else (x == x and x in p) for p in d[1:])
    lo, hi = d
    return lo <= x <= hi


def labbits(x: float) -> str:
    return "".join("1" if labtest(n, x) else "0" for n in [*_SETS, *_CLASSES])


#: MRIasegContraLatLabel below 1000: the labels that are their own counterpart, and the pairs
CONTRA_SELF = {0, 14, 15, 16, 24, 72, 77, 80, 85, 165, 251, 252, 253, 254, 255, 257, 258, 259, 260}
CONTRA_SELF |= {262, 502, 514, 853, 883, 902, 914}
CONTRA_PAIRS = [(i, i + 39) for i in range(1, 14)] + [(17, 53), (18, 54), (19, 55), (20, 56)]
CONTRA_PAIRS += [(i, i + 32) for i in range(25, 40)]
CONTRA_PAIRS += [(73, 74), (78, 79), (81, 82), (83, 84), (96, 97), (138, 139), (265, 266)]
CONTRA_PAIRS += [(819, 820), (821, 822), (843, 844), (865, 866), (869, 870)]
SUBCORT_GRAY = {10, 49, 11, 50, 12, 51, 13, 52, 17, 53, 18, 54, 26, 58, 28, 60, 27, 59}


def contra(v: int) -> tuple[int, str]:
    """§23's MRIasegContraLatLabel: the label and its message on standard output."""
    if v == 0:
        return 0, ""
    if v > 999:
        for lo, hi, step in (
            (1000, 1998, 1000), (2000, 2998, -1000), (3000, 3998, 1000), (4000, 4998, -1000),
            (11100, 11998, 1000), (12100, 12998, -1000), (13100, 13998, 1000), (14100, 14998, -1000),
        ):  # fmt: skip
            if lo <= v <= hi:
                return v + step, ""
        return {5001: 5002, 5002: 5001}.get(v, -1), ""
    if v in CONTRA_SELF:
        return v, ""
    for a, b in CONTRA_PAIRS:
        if v in (a, b):
            return (b if v == a else a), ""
    return -1, f"MRIasegContraLatLabel() id={v} unrecognized\n"


def f32_of(h: str) -> float:
    return struct.unpack(">f", bytes.fromhex(h))[0]


def cma(runner, scratch: Path) -> int:
    gen = C.Gen5(7)
    tally: Counter[str] = Counter()
    bad = 0
    for b in gen.vcma():
        lines = [ln for ln in b.lines if ln.startswith("cmalab ")]
        b2 = C.Batch(b.group, "vcma-cmalab", lines)
        path = C.write_batch(scratch, b2)
        plain, memory = runner.probe_runs(b2, path)
        for line, out, mem in zip(lines, plain, memory, strict=True):
            tok = line.split()
            n = int(tok[1])
            want, msgs = [], ""
            for k in range(n):
                v, f = int(tok[2 + 2 * k]), f32_of(tok[3 + 2 * k])
                c, m = contra(v)
                want += [labbits(v), labbits(f), labbits(f), str(int(v in SUBCORT_GRAY)), str(c)]
                msgs += m
            text = out.decode("latin-1")
            got, _, streams = text.rpartition(" | ")
            so = bytes.fromhex(streams.split(" ")[0][1:]).decode("latin-1")
            if mem:
                tally["memory"] += 1
            elif got.split() == want and so == msgs:
                tally["as the tables"] += 1
            else:
                bad += 1
                if bad <= 3:
                    print(
                        f"DIFFERS: {line[:100]}\n  probe {got[:200]}\n  model {' '.join(want)[:200]}"
                    )
    for k, v in sorted(tally.items()):
        print(f"cma: {k}: {v} lines")
    print(f"cma: differ from the tables: {bad}")
    return 1 if bad else 0


# ---- part 7b (§34, §35) and §54 ------------------------------------------------------------------

FMT = {0: "<B", 1: "<i", 2: "<q", 3: "<f", 4: "<h", 10: "<H"}
F32 = np.float32


def hexf(h: str) -> float:
    return struct.unpack(">f", bytes.fromhex(h))[0]


def fbits(x: float) -> str:
    return f"{struct.unpack('<I', struct.pack('<f', x))[0]:08x}"


@dataclass
class WVol:
    """A W of a probe line (README.md) with its paint: the voxels, the sizes and the cosines."""

    w: int
    h: int
    d: int
    t: int
    f: int
    hdr: int
    sizes: tuple[float, float, float]
    ras_good: int
    cos: tuple[float, ...]
    buf: bytearray

    def off(self, x: int, y: int, z: int, fr: int = 0) -> int:
        return (((fr * self.d + z) * self.h + y) * self.w + x) * BPV[self.t]

    def get(self, x: int, y: int, z: int, fr: int = 0) -> float:
        """MRIgetVoxVal of a voxel inside (as a float)."""
        return float(F32(struct.unpack_from(FMT[self.t], self.buf, self.off(x, y, z, fr))[0]))

    def set(self, x: int, y: int, z: int, v: float) -> None:
        """MRIsetVoxVal of frame 0 (the type's clipping and nint)."""
        o = self.off(x, y, z)
        if self.t == 3:
            struct.pack_into("<f", self.buf, o, v)
            return
        lo, hi = {0: (0, 255), 1: (-(2.0**31), 2.0**31), 2: (-(2.0**63), 2.0**63)}.get(
            self.t, {4: (-32768, 32767), 10: (0, 65535)}.get(self.t, (0, 0))
        )
        iv = nint(max(lo, min(hi, v)))
        if self.t in (0, 10):
            iv &= 0xFF if self.t == 0 else 0xFFFF
        struct.pack_into(FMT[self.t], self.buf, o, iv)


def read_w(tok: list[str], k: int, tab: list[int], paint: bool = True) -> tuple[WVol, int]:
    """The W at token k (and its paint P when ``paint``); the index after them."""
    w, h, d, t, f, hdr = map(int, tok[k : k + 6])
    fill = tok[k + 6]
    sizes = tuple(hexf(x) for x in tok[k + 7 : k + 10])
    ras_good = int(tok[k + 10])
    cos = tuple(hexf(x) for x in tok[k + 11 : k + 20])
    k += 7 + 19 + 18 + 1
    if tok[k] == "null":
        k += 1
    else:
        k += 2 + int(tok[k]) * int(tok[k + 1])
    if tok[k] != "null":
        raise ValueError("a W with a colour table")
    k += 1
    n = 0 if hdr else w * h * d * f * BPV.get(t, 0)
    if fill.startswith("t") and t == 3:
        s = int(fill[1:])
        buf = bytearray(
            b"".join(struct.pack("<I", tab[(7 * i + s) % len(tab)]) for i in range(n // 4))
        )
    elif fill.startswith("i"):
        ws = [int(x) for x in fill[1:].split(",")]
        buf = bytearray(b"".join(struct.pack("<i", ws[i % len(ws)]) for i in range(n // 4)))
    else:
        s = int(fill[1:]) if fill.startswith("t") else int(fill)
        buf = bytearray(n) if s < 0 else bytearray((s + 101 * i) % 256 for i in range(n))
    v = WVol(w, h, d, t, f, hdr, sizes, ras_good, cos, buf)
    if paint:
        np_ = int(tok[k])
        k += 1
        for _ in range(np_):
            v.set(int(tok[k]), int(tok[k + 1]), int(tok[k + 2]), hexf(tok[k + 3]))
            k += 4
    return v, k


def lines_of(groups: list[str], cmd: str) -> list[C.Batch]:
    out = []
    for b in C.all_batches(C.Gen(7), groups):
        ls = [ln for ln in b.lines if ln.startswith(cmd + " ")]
        if ls:
            out.append(C.Batch(b.group, f"{b.name}-{cmd}", ls))
    return out


def run_model(runner, scratch: Path, name: str, groups: list[str], cmd: str, model) -> int:
    """Runs the lines of ``cmd`` and compares each with ``model(tokens, tab)`` (None: left out)."""
    tab = table()
    tally: Counter[str] = Counter()
    bad = 0
    for b in lines_of(groups, cmd):
        path = C.write_batch(scratch, b)
        plain, memory = runner.probe_runs(b, path)
        for line, out, mem in zip(b.lines, plain, memory, strict=True):
            got = out.decode("latin-1").rpartition(" | ")[0]
            head = got.split(" ", 1)[0]
            if mem or head in ("CRASH", "TIMEOUT"):
                tally["memory or crash"] += 1
                continue
            want = model(line.split(), tab)
            if want is None:
                tally["outside the model"] += 1
            elif got == want:
                tally["as the model"] += 1
            else:
                bad += 1
                if bad <= 3:
                    print(f"DIFFERS: {line[:100]}\n  probe {got[:200]}\n  model {want[:200]}")
    for k, v in sorted(tally.items()):
        print(f"{name}: {k}: {v}")
    print(f"{name}: differ from the model: {bad}")
    return 1 if bad else 0


def view_model(tok: list[str], tab: list[int]) -> str | None:
    """§34's MRItoImageView, steps 1 to 8, without a given image and with the frame inside."""
    v, k = read_w(tok, 1, tab)
    if tok[k] != "null":
        return None
    sl, view, fr = int(tok[k + 1]), int(tok[k + 2]), int(tok[k + 3])
    xr, xa, _xs, _yr, ya, ys, zr, za, zs = v.cos
    if not v.ras_good:
        return "null"
    az = [abs(F32(c)) for c in (zr, za, zs)]
    dirn = 1 if az[0] > az[1] and az[0] > az[2] else 0 if az[1] > az[2] else 2
    W, H, D = v.w, v.h, v.d
    tab9 = {
        (0, 0): (W, H, D, xr > 0, ys > 0, "c"),
        (0, 1): (D, H, W, not za > 0, ys > 0, "s"),
        (0, 2): (W, D, H, xr > 0, not za > 0, "h"),
        (1, 0): (D, H, W, zr > 0, ys > 0, "s"),
        (1, 1): (W, H, D, not xa > 0, ys > 0, "c"),
        (1, 2): (D, W, H, zr > 0, not xa > 0, "q"),
        (2, 0): (W, D, H, xr > 0, zs > 0, "h"),
        (2, 1): (H, D, W, not ya > 0, zs > 0, "r"),
        (2, 2): (W, H, D, xr > 0, not ya > 0, "c"),
    }
    w, h, d, xa_, ya_, kind = tab9.get((dirn, view), (0, 0, 0, False, False, None))
    frac = F32(0.4) if view == dirn else F32(0.6)
    if sl < 0:
        sl = nint(float(F32(frac * F32(d))))
    elif sl >= d:
        return "null"
    if v.hdr or not 0 <= fr < v.f or v.t not in (0, 1, 3, 4, 10):
        return None

    def at(x: int, y: int) -> tuple[int, int, int]:
        xx = w - 1 - x if xa_ else x
        yy = h - 1 - y if ya_ else y
        return {
            "c": (xx, yy, sl),
            "s": (sl, yy, xx),
            "h": (xx, sl, yy),
            "q": (yy, sl, xx),
            "r": (sl, xx, yy),
        }[kind]

    fmin, fmax = F32(1e7), F32(-1e7)
    for y in range(h):
        for x in range(w):
            val = v.get(*at(x, y), fr)
            if val > fmax:
                fmax = F32(val)
            if val < fmin:
                fmin = F32(val)
    rng = F32(fmax - fmin)
    pix = bytearray(w * h)
    if not abs(float(rng)) < 1.1920928955078125e-07:
        for y in range(h):
            for x in range(w):
                q = 255.0 * (v.get(*at(x, y), fr) - float(fmin)) / float(rng)
                iv = int(q) if q == q and -(2.0**31) - 1 < q < 2.0**31 else -(2**31)
                pix[(h - 1 - y) * w + x] = iv & 0xFF
    sx, sy = 1.0, 1.0
    if dirn == 0 and kind is not None:
        xsz, ysz, zsz = v.sizes
        sx, sy = {0: (xsz, ysz), 1: (zsz, ysz), 2: (xsz, zsz)}[view]
    px = "x" + pix.hex() if w * h else "null"
    return f"new {h} {w} {h} {w} 0 1 {fbits(sx)} {fbits(sy)} {px}"


def smooth_model(tok: list[str], tab: list[int]) -> str | None:
    """§35's MRIsmoothParcellation on shorts: the copy's voxels (the printed volume's last field)."""
    v, k = read_w(tok, 1, tab)
    count = int(tok[k])
    if v.t != 4 or v.hdr:
        return None
    out = bytearray(v.buf)
    order = [
        (a, b, c)
        for a in (1, 0, -1)
        for b in (1, 0, -1)
        for c in (1, 0, -1)
        if (a, b, c) != (0, 0, 0)
    ]
    for i in range(1, v.w - 1):
        for j in range(1, v.h - 1):
            for z in range(1, v.d - 1):
                vals = [int(v.get(i + a, j + b, z + c)) for a, b, c in order]
                counts = Counter(vals)
                for x in vals:
                    if counts[x] >= count:
                        struct.pack_into("<h", out, v.off(i, j, z), x)
    return "x" + bytes(out).hex()


def smooth(runner, scratch: Path) -> int:
    def model(tok, tab):
        return smooth_model(tok, tab)

    tab = table()
    tally: Counter[str] = Counter()
    bad = 0
    for b in lines_of(["vnbhd", "vrules9"], "smpar"):
        path = C.write_batch(scratch, b)
        plain, memory = runner.probe_runs(b, path)
        for line, out, mem in zip(b.lines, plain, memory, strict=True):
            got = out.decode("latin-1").rpartition(" | ")[0]
            if mem or got.split(" ", 1)[0] in ("CRASH", "TIMEOUT"):
                tally["memory or crash"] += 1
                continue
            want = model(line.split(), tab)
            if want is None:
                tally["outside the model"] += 1
            elif got.split(" ")[-1] == want:
                tally["as the model"] += 1
            else:
                bad += 1
                if bad <= 3:
                    print(f"DIFFERS: {line[:100]}")
    for k, v in sorted(tally.items()):
        print(f"smooth: {k}: {v}")
    print(f"smooth: differ from the model: {bad}")
    return 1 if bad else 0


def nearest_model(tok: list[str], tab: list[int]) -> str | None:
    """§35's MRIfindNearestNonzeroLocation at points whose rounding is inside the volume."""
    v, k = read_w(tok, 1, tab)
    if v.hdr:
        return None
    n = int(tok[k])
    k += 1
    b = BPV[v.t]

    def byte(x: int, y: int, z: int) -> int:
        return v.buf[(z * v.h + y) * v.w * b + x]

    def clamp(c: int, e: int) -> int:
        return min(max(c, 0), e - 1)

    res = []
    for _ in range(n):
        ws = int(tok[k])
        xr, yr, zr = (struct.unpack(">d", bytes.fromhex(t))[0] for t in tok[k + 1 : k + 4])
        wp = int(tok[k + 4])
        k += 5
        x, y, z = nint(xr), nint(yr), nint(zr)
        if not (0 <= x < v.w and 0 <= y < v.h and 0 <= z < v.d):
            return None
        if byte(x, y, z) > 0:
            res += [fbits(float(byte(x, y, z))), "-7", "-7", "-7"]
            continue
        md, mv, place = F32(100000), 0.0, ("-7", "-7", "-7")
        wh = int((ws - 1) / 2)
        for zk in range(-wh, wh + 1):
            zi = clamp(z + zk, v.d)
            dz = F32(zi - zr)
            for yk in range(-wh, wh + 1):
                yi = clamp(y + yk, v.h)
                dy = F32(yi - yr)
                for xk in range(-wh, wh + 1):
                    xi = clamp(x + xk, v.w)
                    dx = F32(xi - xr)
                    if byte(xi, yi, zi) > 0:
                        sq = F32(F32(F32(dx * dx) + F32(dy * dy)) + F32(dz * dz))
                        dist = F32(np.sqrt(np.float64(sq)))
                        if dist < md:
                            md, mv = dist, float(byte(xi, yi, zi))
                            if wp:
                                place = (str(xi), str(yi), str(zi))
        res += [fbits(mv), *place]
    return " ".join(res)


def view(runner, scratch: Path) -> int:
    return run_model(runner, scratch, "view", ["vimage", "vrules9"], "toimgv", view_model)


def nearest(runner, scratch: Path) -> int:
    return run_model(runner, scratch, "nearest", ["vnbhd"], "nearnz", nearest_model)


def mprod(a: np.ndarray, b: np.ndarray) -> np.ndarray:
    """``specs/freesurfer-maths.md`` §8.1's float product."""
    o = np.zeros((a.shape[0], b.shape[1]), dtype=np.float32)
    for i in range(a.shape[0]):
        for j in range(b.shape[1]):
            acc = F32(0)
            for q in range(a.shape[1]):
                acc = F32(acc + F32(a[i, q] * b[q, j]))
            o[i, j] = acc
    return o


def read_m(tok: list[str], k: int) -> tuple[np.ndarray, int]:
    r, c = int(tok[k]), int(tok[k + 1])
    v = np.array([hexf(x) for x in tok[k + 2 : k + 2 + r * c]], dtype=np.float32)
    return v.reshape(r, c), k + 2 + r * c


def physvox(runner, scratch: Path) -> int:
    """§54's physical-voxel to RAS-to-RAS under ``reproduce``: ``S'(dst) ((inv(V(dst)) (M V(src)))
    S(src))`` bit for bit, V and inv(V) the probe's own ``vgi2r`` and ``vgr2i`` of the geometries."""
    import random

    gen = C.Gen10(7)
    r = random.Random("7:physvox")
    lines, cases = [], []
    for _ in range(60):
        src, dst = (gen.geom(r, ext=(1, 5)).split(" ") for _ in range(2))
        src[0] = dst[0] = "1"
        m = gen.affine(r)
        zero = C.f32(0.0)
        lt = " ".join([zero] * 4 + [m, "null", "null", "0", "0", " ".join(src), "1", "null"])
        lt += " " + " ".join(dst) + " 1 null"
        inv = " ".join([zero] * 4 + [gen.affine(r), "null", "null", "0", "0", " ".join(dst), "1"])
        inv += " null " + " ".join(src) + " 1 null"
        lines.append(f"ltachg 1 2 {C.f32(1.0)} x {lt} {inv} 1")
        lines.append(f"vgi2r {' '.join(src)}")
        lines.append(f"vgr2i {' '.join(dst)}")
        cases.append((m, src, dst))
    b = C.Batch("physvox", "physvox", lines)
    path = C.write_batch(scratch, b)
    plain, memory = runner.probe_runs(b, path)
    tally: Counter[str] = Counter()
    bad = 0
    for i, (m, src, dst) in enumerate(cases):
        outs = [
            plain[3 * i + j].decode("latin-1").rpartition(" | ")[0].split(" ") for j in range(3)
        ]
        if any(memory[3 * i : 3 * i + 3]) or outs[0][0] != "self" or outs[2][0] == "null":
            tally["outside the model"] += 1
            continue
        got, _ = read_m(outs[0], 1 + 4 + 4)
        vs, _ = read_m(outs[1], 0)
        ivd, _ = read_m(outs[2], 0)
        m0, _ = read_m(m.split(" "), 0)
        s, si = np.eye(4, dtype=np.float32), np.eye(4, dtype=np.float32)
        for q in range(3):
            s[q, q] = F32(hexf(src[4 + q]))
            si[q, q] = F32(1.0 / np.float64(F32(hexf(dst[4 + q]))))
        want = mprod(si, mprod(mprod(ivd, mprod(m0, vs)), s))
        if np.array_equal(want.view(np.uint32), got.view(np.uint32)):
            tally["as the model"] += 1
        else:
            bad += 1
    for k, v in sorted(tally.items()):
        print(f"physvox: {k}: {v}")
    print(f"physvox: differ from the model: {bad}")
    return 1 if bad else 0


def read_mn(tok: list[str], k: int) -> tuple[np.ndarray | None, int]:
    """M (README.md's encodings) or ``null``."""
    if tok[k] == "null":
        return None, k + 1
    return read_m(tok, k)


def read_lt_in(tok: list[str], k: int) -> tuple[tuple, int]:
    """An LT as compare.py writes it: its matrix and the 24 tokens of each geometry."""
    m, k = read_mn(tok, k + 4)
    _, k = read_mn(tok, k)
    _, k = read_mn(tok, k)
    k += 2
    gs = []
    for _ in range(2):
        gs.append(tok[k : k + 24])
        _, k = read_mn(tok, k + 25)
    return (m, gs[0], gs[1]), k


def read_lt_out(tok: list[str], k: int) -> tuple[np.ndarray | None, int]:
    """A printed LT: its matrix; the geometries printed (G, the cache, two matrices) skipped."""
    m, k = read_mn(tok, k + 4)
    _, k = read_mn(tok, k)
    _, k = read_mn(tok, k)
    k += 2
    for _ in range(2):
        k += 24
        k += 1 if tok[k] == "null" else 17
        _, k = read_mn(tok, k)
        _, k = read_mn(tok, k)
    return m, k


#: §54's changes between the voxel (0), RAS-to-RAS (1) and physical-voxel (2) types, the RAS-to-voxel
#: matrix each needs (of the source or the destination), and the message when it is absent.
LTA_CHANGES = {
    (1, 0): ("dst", "LTAgetV2V: passed volumes did not have the info on i_to_r or r_to_i."),
    (1, 2): ("dst", "LTAgetV2V: passed volumes did not have the info on i_to_r or r_to_i."),
    (0, 1): ("src", "LTAgetR2R: passed volumes did not have the info on r_to_i or i_to_r"),
    (0, 2): (None, ""),
    (2, 0): (None, ""),
    (2, 1): ("dst", "LTAgetV2V: passed volumes did not have the info on i_to_r or r_to_i."),
}


def lta_change(pair: tuple, m, src, dst, vm: dict, ivm: dict) -> np.ndarray:
    """§54's product for one transform (``reproduce`` for physical-voxel to RAS-to-RAS)."""

    def sz(g, inv):
        s = np.eye(4, dtype=np.float32)
        for q in range(3):
            x = np.float64(F32(hexf(g[4 + q])))
            s[q, q] = F32(1.0 / x) if inv else F32(x)
        return s

    ks, kd = " ".join(src), " ".join(dst)
    if pair == (1, 0):
        return mprod(ivm[kd], mprod(m, vm[ks]))
    if pair == (1, 2):
        vox = mprod(ivm[kd], mprod(m, vm[ks]))
        return mprod(sz(dst, False), mprod(vox, sz(src, True)))
    if pair == (0, 1):
        return mprod(vm[kd], mprod(m, ivm[ks]))
    if pair == (0, 2):
        return mprod(sz(dst, False), mprod(m, sz(src, True)))
    if pair == (2, 0):
        return mprod(sz(dst, True), mprod(m, sz(src, False)))
    return mprod(sz(dst, True), mprod(mprod(ivm[kd], mprod(m, vm[ks])), sz(src, False)))


def ltatypes(runner, scratch: Path) -> int:
    """§54's changes of a transform array between voxel, RAS-to-RAS and physical-voxel (``ltachg``):
    every such line of the groups vltapv and vrules10 and 180 seeded arrays (30 per change, one or
    two transforms, valid geometries, general affine matrices), against the products of §54 with the
    geometries' matrices from the probe's own ``vgi2r`` and ``vgr2i``, the validity checks first, the
    end of the program where an inverse it needs is absent, the array's new type."""
    import random

    old = []
    for b in lines_of(["vltapv", "vrules10"], "ltachg"):
        old += b.lines
    gen = C.Gen10(7)
    r = random.Random("7:ltatypes")
    own = []
    zero = C.f32(0.0)
    for pair in LTA_CHANGES:
        for _ in range(30):
            n = r.choice([1, 1, 2])
            fw, inv = [], []
            for _ in range(n):
                src, dst = (gen.geom(r, ext=(1, 5)).split(" ") for _ in range(2))
                src[0] = dst[0] = "1"
                a, b2 = " ".join(src) + " 1 null", " ".join(dst) + " 1 null"
                fw.append(" ".join([zero] * 4 + [gen.affine(r), "null", "null", "0", "0", a, b2]))
                inv.append(" ".join([zero] * 4 + [gen.affine(r), "null", "null", "0", "0", b2, a]))
            own.append(f"ltachg {n} {pair[0]} {C.f32(1.0)} x {' '.join(fw + inv)} {pair[1]}")
    cases = []
    geos: dict[str, None] = {}
    for line in old + own:
        tok = line.split(" ")
        n, ty, tg = int(tok[1]), int(tok[2]), int(tok[-1])
        k, items = 5, []
        for _ in range(n):
            it, k = read_lt_in(tok, k)
            items.append(it)
        cases.append((n, ty, tg, items))
        for _, a, b2 in items:
            geos[" ".join(a)] = None
            geos[" ".join(b2)] = None
    glist = list(geos)
    gl = [f"vgi2r {g}" for g in glist] + [f"vgr2i {g}" for g in glist]
    lines = old + own + gl
    b = C.Batch("ltatypes", "ltatypes", lines)
    plain, memory = runner.probe_runs(b, C.write_batch(scratch, b))
    if any(memory):
        print(f"ltatypes: {sum(memory)} memory lines (left out)")
    outg = [ln.decode("latin-1").rpartition(" | ")[0].split(" ") for ln in plain[len(cases) :]]
    vm, ivm = {}, {}
    for i, g in enumerate(glist):
        vm[g] = read_m(outg[i], 0)[0]
        o = outg[len(glist) + i]
        ivm[g] = None if o[0] == "null" else read_m(o, 0)[0]
    tally: Counter[str] = Counter()
    bad = 0
    for i, (n, ty, tg, items) in enumerate(cases):
        if memory[i]:
            continue
        res, _, err = C.decode(plain[i])
        got = res.decode("latin-1").split(" ")
        pair = (ty, tg)
        if pair not in LTA_CHANGES:
            tally["other changes (outside the model)"] += 1
            continue
        which, msg = LTA_CHANGES[pair]
        want_err = None
        for _, a, b2 in items:
            if a[0] == "0":
                want_err = "LTAchangeType: src geometry must be valid\n"
            elif b2[0] == "0":
                want_err = "LTAchangeType: dst geometry must be valid\n"
            if want_err:
                break
        if want_err is None and any(
            m is None or m.shape != (4, 4) for m, _, _ in items
        ):  # §8.1's messages for other sizes
            tally["matrices of another size (outside the model)"] += 1
            continue
        if want_err is None and which:
            for _, a, b2 in items:
                if ivm[" ".join(a if which == "src" else b2)] is None:
                    want_err = msg
                    break
        if want_err is not None:
            ok = got[:2] == ["EXIT", "252"] and err.startswith(b"error: " + want_err.encode())
            tally["ended as the model"] += ok
            bad += not ok
            continue
        with np.errstate(all="ignore"):
            want = [lta_change(pair, m, a, b2, vm, ivm) for m, a, b2 in items]
        finite = all(np.isfinite(w).all() for w in want) and all(
            np.isfinite(m).all() for m, _, _ in items
        )
        if got[:2] == ["EXIT", "252"] and err.startswith(b"error: TransformInvert"):
            if not finite or any(abs(np.linalg.det(w.astype(np.float64))) < 1e-6 for w in want):
                tally["ended at the inverses' fill (a result not finite or singular)"] += 1
            else:
                bad += 1
                print(f"DIFFERS (fill ends): line {i + 1}")
            continue
        if got[0] != "self" or int(got[1]) != n or int(got[2]) != tg:
            bad += 1
            print(f"DIFFERS (head): line {i + 1}: {' '.join(got[:4])}")
            continue
        k, ms = 5, []
        for _ in range(n):
            mm, k = read_lt_out(got, k)
            ms.append(mm)
        same = all(
            mm is not None and np.array_equal(w.view(np.uint32), mm.view(np.uint32))
            for w, mm in zip(want, ms, strict=True)
        )
        key = "as the model" if finite else "as the model, non-finite values"
        if same:
            tally[f"{key} {pair[0]}->{pair[1]}"] += 1
        elif not finite:
            tally["non-finite values, other NaN bits (outside the model)"] += 1
        else:
            bad += 1
            print(f"DIFFERS: line {i + 1} {pair}")
    for k2, v in sorted(tally.items()):
        print(f"ltatypes: {k2}: {v}")
    print(f"ltatypes: differ from the model: {bad}")
    return 1 if bad else 0


def i32(x: int) -> int:
    """x as a 32-bit two's-complement integer (C's int arithmetic on x86-64)."""
    return (x + 2**31) % 2**32 - 2**31


def region_values(w_s: str) -> tuple[tuple[int, int, int], list[float]]:
    """The extent and frame 0's values of a W with an ``i`` fill (README.md, "Encodings")."""
    p = w_s.split()
    w, h, d, t, _f, hdr = (int(x) for x in p[:6])
    assert hdr == 0 and p[6].startswith("i"), w_s[:60]
    words = [int(x) for x in p[6][1:].split(",")]
    n = w * h * d
    if t == 3:
        raw = struct.pack(f"<{n}I", *(words[i % len(words)] for i in range(n)))
        vals = [float(v) for v in struct.unpack(f"<{n}f", raw)]
    elif t == 1:
        vals = [float(i32(words[i % len(words)])) for i in range(n)]
    elif t == 0:
        nb = (n + 3) // 4
        raw = struct.pack(f"<{nb}I", *(words[i % len(words)] for i in range(nb)))
        vals = [float(b) for b in raw[:n]]
    else:
        raise ValueError(f"type {t} is outside the model")
    return (w, h, d), vals


def region_extremes(dims, vals) -> tuple[list[int], list[int]]:
    """§43: the extremes of the voxels whose magnitude is not below 1e-5 (a NaN counts), from
    1000000 and 0."""
    lo, hi = [1000000] * 3, [0] * 3
    w, h, d = dims
    for z in range(d):
        for y in range(h):
            for x in range(w):
                v = vals[x + w * (y + h * z)]
                if abs(v) < 1e-5:
                    continue
                for a, c in enumerate((x, y, z)):
                    lo[a] = min(lo[a], c)
                    hi[a] = max(hi[a], c)
    return lo, hi


def region_box(dims, lo, hi, pads: list[int], decided: bool) -> list[int]:
    """§43's box (pads before and after each axis): FreeSurfer's, or FS-269's decided one."""
    start = [max(i32(lo[a] - pads[2 * a]), 0) for a in range(3)]

    def size(a: int, one: int) -> int:
        return min(
            i32(hi[a] - lo[a] + one + pads[2 * a] + pads[2 * a + 1]), i32(dims[a] - start[a])
        )

    out = [size(a, 0) for a in range(3)]
    if decided and lo[0] <= hi[0]:
        out = [size(a, 1) if start[a] + out[a] - 1 < hi[a] else out[a] for a in range(3)]
    return start + out


def region_eqodd(dims, box: list[int]) -> tuple[str, str]:
    """§43's REGIONgetBoundingBoxEqOdd from a box: (the result, standard output)."""
    w, h, d = dims
    x, y, z, dx, dy, dz = box

    def show(b):
        return f"{b[0]} {b[1]} {b[2]}  {b[3]} {b[4]} {b[5]}\n"

    so = f"Volume size: {w} {h} {d}, OrigRegion: " + show(box)
    if dx > h:
        return "null", so + "ERROR: REGIONgetBoundingBoxEqOdd(): dx is bigger than MRI height\n"
    if dy > w:
        return "null", so + "ERROR: REGIONgetBoundingBoxEqOdd(): dy is bigger than MRI width\n"
    isodd = math.fmod(dz, 2) != 0

    def res(*b):
        return " ".join(map(str, b))

    if dx == dy and isodd:
        return res(x, y, z, dx, dy, dz), so
    if not isodd:
        if dz < d:
            dz += 1
        elif z > 0:
            z, dz = z - 1, dz + 1
        else:
            return "null", so + (
                f"ERROR: REGIONgetBoundingBoxEqOdd(): cannot make z odd: z={z}, dz={dz}, nz={d}\n"
            )
    if dx == dy:
        return res(x, y, z, dx, dy, dz), so
    dxy = max(dx, dy)
    if x + dxy <= w and y + dxy <= h:
        return res(x, y, z, dxy, dxy, dz), so
    delta = abs(dx - dy)
    d1 = delta // 2
    d2 = delta - d1
    if dx < dy:
        if x - d1 > 0 and x + dx + d2 <= w:
            return res(x - d1, y, z, dx + delta, dy, dz), so
        if x < w - (x + dx):
            x, dx = 0, dy
        else:
            dx = dy
            x = w - dx
        b = [x, y, z, dx, dy, dz]
        return res(*b), so + f"Volume size: {w} {h} {d}, xAsymRegion: " + show(b)
    if y - d1 > 0 and y + dy + d2 <= h:
        return res(x, y - d1, z, dx, dy + delta, dz), so
    if y < h - (y + dy):
        y, dy = 0, dx
    else:
        dy = dx
        y = h - dy
    b = [x, y, z, dx, dy, dz]
    return res(*b), so + f"Volume size: {w} {h} {d}, yAsymRegion: " + show(b)


def region_model(line: str, seen: set[str], decided: bool) -> tuple[str, str]:
    """An ``rgbb`` line's result and standard output by §43 (FreeSurfer's box, or the decided)."""
    rest = line.partition(" ")[2]
    w_s = max((s for s in seen if rest.startswith(s + " ")), key=len)
    tail = rest[len(w_s) + 1 :].split()
    assert tail[0] == "0", line[:80]  # no paint
    kind = int(tail[1])
    pads = [int(v) for v in tail[2:8]] if kind == 1 else [int(tail[2])] * 6
    dims, vals = region_values(w_s)
    lo, hi = region_extremes(dims, vals)
    box = region_box(dims, lo, hi, pads, decided)
    if kind == 2:
        return region_eqodd(dims, box)
    return " ".join(map(str, box)), ""


def region(runner, scratch: Path, candidate: Path | None = None) -> int:
    gen = C.Gen17(7)
    seen: set[str] = set()
    orig = C.Gen13.lab

    def lab(geo, vals, t=1, nf=1, ct="null"):
        s = orig(geo, vals, t=t, nf=nf, ct=ct)
        seen.add(s)
        return s

    gen.lab = lab
    tally: Counter[str] = Counter()
    bad = 0
    for group in ("vregion", "vrules17", "vrules18", "vrules20"):
        lines = [ln for b in getattr(gen, group)() for ln in b.lines if ln.startswith("rgbb ")]
        b = C.Batch(group, f"region-{group}", lines)
        path = C.write_batch(scratch, b)
        plain, memory = runner.probe_runs(b, path)
        runs = {"probe": plain}
        if candidate is not None:
            for mode in ("reproduce", "fix"):
                runs[mode] = runner.candidate_run(candidate, b, path, mode)
        for i, line in enumerate(lines):
            fs = region_model(line, seen, decided=False)
            dec = region_model(line, seen, decided=True)
            tally[f"{group}: lines"] += 1
            tally[f"{group}: the decided box differs"] += fs != dec
            for who, out in runs.items():
                want = dec if who == "fix" else fs
                if memory[i] and who == "probe":
                    tally[f"{group}: memory"] += 1
                    continue
                res, so, se = C.decode(out[i]) if i < len(out) else (b"<missing>", b"", b"")
                se = C.NOTICE.sub(b"", se)
                got = (res.decode("latin-1"), so.decode("latin-1"))
                if got == want and not se:
                    tally[f"{group}: {who} as the model"] += 1
                else:
                    bad += 1
                    if bad <= 5:
                        print(f"DIFFERS ({who}): {group} line {i + 1}: {line[:90]}")
                        print(f"    got   {got!r} {se[:80]!r}\n    model {want!r}")
    for k, v in sorted(tally.items()):
        print(f"region: {k}: {v}")
    print(f"region: differ from the model: {bad}")
    return 1 if bad else 0


#: struct formats of a voxel of the types the covariance model reads
VOXFMT = {0: "B", 1: "i", 2: "q", 3: "f", 4: "h", 10: "H"}


def vol_values(w_s: str) -> tuple[tuple[int, int, int, int], list[float]] | None:
    """The extent, frame count and every voxel's ``MRIgetVoxVal`` (frame-major: index ((f depth
    + s) height + r) width + c) of a W with an ``i`` fill; None for a type outside the model."""
    p = w_s.split()
    w, h, d, t, f, hdr = (int(x) for x in p[:6])
    if hdr or t not in VOXFMT or not p[6].startswith("i"):
        return None
    words = [int(x) for x in p[6][1:].split(",")]
    n = w * h * d * f
    size = struct.calcsize(VOXFMT[t])
    # the probe fills whole words only: bytes past the last whole word stay as allocated (0)
    nw = (n * size) // 4
    raw = struct.pack(f"<{nw}I", *(words[i % len(words)] for i in range(nw)))
    raw += bytes(n * size - len(raw))
    vals = list(struct.unpack(f"<{n}{VOXFMT[t]}", raw)) if n else []
    # MRIgetVoxVal: the value as a float (ints and longs rounded to float)
    vals = [float(np.float32(v)) for v in vals]
    return (w, h, d, f), vals


def cov_model(line: str, seen: set[str], decided: bool) -> tuple[str, str] | None:
    """A ``cov`` line's result and standard output by §42 (FreeSurfer's count or FS-483's
    decided one); None when the line is outside the model (a type it does not decode, a mask of
    another extent)."""
    rest = line.partition(" ")[2]
    w_s = max((s for s in seen if rest.startswith(s + " ")), key=len)
    tail = rest[len(w_s) + 1 :]
    assert tail.startswith("0 "), line[:80]
    tail = tail[2:]
    vv = vol_values(w_s)
    if vv is None:
        return None
    (w, h, d, nf), vals = vv
    mask = None
    if tail != "null":
        m_s = max((s for s in seen if tail.startswith(s + " ")), key=len)
        mv = vol_values(m_s)
        if mv is None or mv[0][:3] != (w, h, d):
            return None
        mask = mv[1]
    order = [(c, r, s) for c in range(w) for r in range(h) for s in range(d)]

    def mval(c: int, r: int, s: int) -> float:
        return mask[(s * h + r) * w + c]

    if mask is None:
        count = w * h * d
        use = order
    else:
        use = [v for v in order if not mval(*v) < 0.5]
        count = len(use) if decided else sum(1 for v in order if mval(*v) > 0.5)
        if count == 0:
            so = (
                ""
                if nf
                else "MRI::Shape::Shape(): volume size must be greater than 0 in every dimension\n"
            )
            return "null", so + "ERROR: no voxels in mask\n"
    out = [[0.0] * nf for _ in range(nf)]
    for f1 in range(nf):
        for f2 in range(f1, nf):
            acc = 0.0
            for c, r, s in use:
                v1 = vals[((f1 * d + s) * h + r) * w + c]
                v2 = vals[((f2 * d + s) * h + r) * w + c]
                acc = v2 * v1 + acc
            with np.errstate(over="ignore"):  # beyond the float range: an infinity, as x86 rounds
                e = float(np.float32(acc / count))
            out[f1][f2] = out[f2][f1] = e
    cells = [struct.pack(">f", x).hex() for row in out for x in row]
    # a volume without frames: its allocation (the probe's, before the call) prints fs.mri §4's line
    so = (
        "" if nf else "MRI::Shape::Shape(): volume size must be greater than 0 in every dimension\n"
    )
    return " ".join([str(nf), str(nf), *cells]), so


def cov(runner, scratch: Path, candidate: Path | None = None) -> int:
    gen = C.Gen16(7)
    seen: set[str] = set()
    orig = C.Gen13.lab

    def lab(geo, vals, t=1, nf=1, ct="null"):
        s = orig(geo, vals, t=t, nf=nf, ct=ct)
        seen.add(s)
        return s

    gen.lab = lab
    tally: Counter[str] = Counter()
    bad = 0
    for group in ("vcov", "vrules19"):
        lines = [ln for b in getattr(gen, group)() for ln in b.lines if ln.startswith("cov ")]
        b = C.Batch(group, f"cov-{group}", lines)
        path = C.write_batch(scratch, b)
        plain, memory = runner.probe_runs(b, path)
        runs = {"probe": plain}
        if candidate is not None:
            for mode in ("reproduce", "fix"):
                runs[mode] = runner.candidate_run(candidate, b, path, mode)
        for i, line in enumerate(lines):
            fs = cov_model(line, seen, decided=False)
            if fs is None:
                tally[f"{group}: outside the model (a mask of another extent)"] += 1
                continue
            dec = cov_model(line, seen, decided=True)
            tally[f"{group}: lines"] += 1
            tally[f"{group}: FS-483's count changes the result"] += fs != dec
            for who, out in runs.items():
                want = dec if who == "fix" else fs
                if memory[i] and who == "probe":
                    tally[f"{group}: memory"] += 1
                    continue
                res, so, se = C.decode(out[i]) if i < len(out) else (b"<missing>", b"", b"")
                se = C.NOTICE.sub(b"", se)
                got = (res.decode("latin-1"), so.decode("latin-1"))
                if got == want and not se:
                    tally[f"{group}: {who} as the model"] += 1
                    continue
                # a NaN's payload is outside the model: the same places NaN, the rest equal
                g, m = got[0].split(), want[0].split()
                if len(g) == len(m) and got[1] == want[1] and not se:
                    pairs = list(zip(g[2:], m[2:], strict=True))

                    def nan(x: str) -> bool:
                        return (int(x, 16) & 0x7F800000) == 0x7F800000 and int(x, 16) & 0x7FFFFF

                    if g[:2] == m[:2] and all(a == z or (nan(a) and nan(z)) for a, z in pairs):
                        tally[f"{group}: {who} as the model but a NaN's payload"] += 1
                        continue
                bad += 1
                if bad <= 5 or who == "probe":
                    print(f"DIFFERS ({who}): {group} line {i + 1}: {line[:90]}")
                    print(f"    got   {got!r}\n    model {want!r}")
    for k, v in sorted(tally.items()):
        print(f"cov: {k}: {v}")
    print(f"cov: differ from the model: {bad}")
    return 1 if bad else 0


#: struct formats of a voxel of the types a T1 of ``fbnwm`` takes (the bright model)
LABFMT = {1: "<i", 2: "<q", 3: "<f", 4: "<h", 10: "<H"}


def t1_byte(v: float) -> int:
    """FS-802's default (§59): a T1 value as a byte (truncated; below 0 and NaN 0, 256 up 255)."""
    if math.isnan(v) or v < 0:
        return 0
    return 255 if v >= 256 else int(v)


def wm_byte(v: float) -> int:
    """FS-1233's default (§59): a white-matter value as a byte (truncated, clamped to 0..255)."""
    if math.isnan(v) or v <= 0:
        return 0
    return int(min(v, 255.0))


def bright_parts(line: str) -> tuple[list[str], list[str]] | None:
    """An ``fbnwm`` line's T1 and white-matter W (47 tokens each: compare.py's ``w_of``), both
    without a painting; None for another form."""
    t = line.split(" ")
    if len(t) != 97 or t[0] != "fbnwm" or t[48] != "0" or t[96] != "0":
        return None
    return t[1:48], t[49:96]


def as_bytes(w: list[str], conv) -> str | None:
    """W P of a byte volume of w's extent and header holding frame 0's values through conv: an
    ``i`` fill of whole words and a painting of the voxels past the last whole word (the probe
    fills whole words only); None outside the model (a type it does not decode, frames)."""
    vv = vol_values(" ".join(w))
    if vv is None or vv[0][3] != 1:
        return None
    (x, y, z, _f), vals = vv
    b = [conv(v) for v in vals]
    n = len(b)
    pad = b + [0] * (-n % 4) if n else [0, 0, 0, 0]
    words = [pad[i] | pad[i + 1] << 8 | pad[i + 2] << 16 | pad[i + 3] << 24 for i in range(0, len(pad), 4)]
    out = list(w)
    out[3], out[6] = "0", C.lfill(words)
    tail = [(i, b[i]) for i in range(n - n % 4, n)]
    paint = [str(len(tail))]
    for i, v in tail:
        paint += [str(i % x), str(i // x % y), str(i // (x * y)), C.f32(float(v))]
    return " ".join(out + paint)


def bright(runner, scratch: Path, candidate: Path | None = None) -> int:
    """§59's MRIfindBrightNonWM with FS-1233 (a T1 or white matter not of bytes): ``reproduce``
    FreeSurfer's line; the default FreeSurfer's labels for the byte volumes the section names (the
    probe's own ``fbnwm`` of them), written by value into the probe's clone of the T1 (``clone``)
    when the T1 is not of bytes."""
    gen = C.Gen20(7)
    tally: Counter[str] = Counter()
    bad = 0
    for group in ("vbright", "vrules24"):
        lines = [ln for b in getattr(gen, group)() for ln in b.lines if ln.startswith("fbnwm ")]
        # the byte volumes' lines and the T1's clone, two per line of another type
        extra: list[str] = []
        plan: list[tuple[int, int] | None] = []
        for line in lines:
            parts = bright_parts(line)
            if parts is None or (parts[0][3] == "0" and parts[1][3] == "0"):
                plan.append(None)
                continue
            t1, wm = parts
            t1b = " ".join(t1) + " 0" if t1[3] == "0" else as_bytes(t1, t1_byte)
            wmb = " ".join(wm) + " 0" if wm[3] == "0" else as_bytes(wm, wm_byte)
            if t1b is None or wmb is None:
                plan.append(None)
                tally[f"{group}: outside the model"] += 1
                continue
            plan.append((len(extra), int(t1[3])))
            extra.append(f"fbnwm {t1b} {wmb}")
            extra.append(f"clone {' '.join(t1)} new")
        b = C.Batch(group, f"bright-{group}", lines)
        path = C.write_batch(scratch, b)
        plain, memory = runner.probe_runs(b, path)
        eb = C.Batch(group, f"bright-{group}-bytes", extra)
        eplain, ememory = runner.probe_runs(eb, C.write_batch(scratch, eb))
        runs = {}
        if candidate is not None:
            for mode in ("reproduce", "fix"):
                runs[mode] = runner.candidate_run(candidate, b, path, mode)
        for i, line in enumerate(lines):
            fs_res, fs_so, fs_se = C.decode(plain[i])
            fs = (fs_res.decode("latin-1"), fs_so.decode("latin-1"))
            if plan[i] is None:
                want_fix = fs
                tally[f"{group}: lines of bytes"] += 1
            else:
                k, t1type = plan[i]
                tally[f"{group}: lines with FS-1233"] += 1
                if ememory[k] or ememory[k + 1] or memory[i]:
                    tally[f"{group}: memory"] += 1
                    continue
                lres, lso, _ = C.decode(eplain[k])
                if t1type == 0:
                    want_fix = (lres.decode("latin-1"), lso.decode("latin-1"))
                else:
                    labels = bytes.fromhex(lres.decode().split(" ")[-1][1:])
                    cres = C.decode(eplain[k + 1])[0].decode("latin-1").split(" ")
                    vox = bytearray(bytes.fromhex(cres[-1][1:]))
                    size = struct.calcsize(LABFMT[t1type])
                    for j, v in enumerate(labels):
                        if v:
                            vox[j * size : (j + 1) * size] = struct.pack(LABFMT[t1type], v)
                    want_fix = (" ".join(cres[:-1] + ["x" + vox.hex()]), lso.decode("latin-1"))
                tally[f"{group}: the default changes the result"] += want_fix != fs
            if memory[i]:
                tally[f"{group}: memory"] += 1
                continue
            for who, out in runs.items():
                want = want_fix if who == "fix" else fs
                res, so, se = C.decode(out[i]) if i < len(out) else (b"<missing>", b"", b"")
                se = C.NOTICE.sub(b"", se)
                got = (res.decode("latin-1"), so.decode("latin-1"))
                if got == want and se == fs_se:
                    tally[f"{group}: {who} as the model"] += 1
                else:
                    bad += 1
                    if bad <= 5:
                        print(f"DIFFERS ({who}): {group} line {i + 1}: {line[:90]}")
                        print(f"    got   {got[1]!r} {se[:80]!r}\n    model {want[1]!r}")
    for k, v in sorted(tally.items()):
        print(f"bright: {k}: {v}")
    print(f"bright: differ from the model: {bad}")
    return 1 if bad else 0


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument(
        "command",
        choices=[
            "fs291",
            "skull",
            "cma",
            "view",
            "smooth",
            "nearest",
            "physvox",
            "ltatypes",
            "region",
            "cov",
            "bright",
        ],
    )
    ap.add_argument("--probe", type=Path, required=True)
    ap.add_argument("--scratch", type=Path, required=True)
    ap.add_argument(
        "--candidate", type=Path, help="region, cov, bright: also check this candidate's lines"
    )
    ap.add_argument("--no-cache", action="store_true")
    a = ap.parse_args()
    scratch = a.scratch.resolve()
    scratch.mkdir(parents=True, exist_ok=True)
    runner = C.Runner(a.probe.resolve(), scratch, not a.no_cache)
    if a.command in ("region", "cov", "bright"):
        fn = {"region": region, "cov": cov, "bright": bright}[a.command]
        return fn(runner, scratch, a.candidate.resolve() if a.candidate else None)
    return {
        "fs291": fs291,
        "skull": skull,
        "cma": cma,
        "view": view,
        "smooth": smooth,
        "nearest": nearest,
        "physvox": physvox,
        "ltatypes": ltatypes,
    }[a.command](runner, scratch)


if __name__ == "__main__":
    sys.exit(main())
