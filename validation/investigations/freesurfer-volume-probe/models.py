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

The probe runs through the oracle-output cache (one thread, the family ``freesurfer``) when the
validation package is importable (``PYTHONPATH=validation/src``), else directly. Exit status 1 when a
line differs from its model.
"""

from __future__ import annotations

import argparse
import importlib.util
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


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("command", choices=["fs291", "skull", "cma"])
    ap.add_argument("--probe", type=Path, required=True)
    ap.add_argument("--scratch", type=Path, required=True)
    ap.add_argument("--no-cache", action="store_true")
    a = ap.parse_args()
    scratch = a.scratch.resolve()
    scratch.mkdir(parents=True, exist_ok=True)
    runner = C.Runner(a.probe.resolve(), scratch, not a.no_cache)
    return {"fs291": fs291, "skull": skull, "cma": cma}[a.command](runner, scratch)


if __name__ == "__main__":
    sys.exit(main())
