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
    gen = C.Gen(7)
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


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("command", choices=["fs291"])
    ap.add_argument("--probe", type=Path, required=True)
    ap.add_argument("--scratch", type=Path, required=True)
    ap.add_argument("--no-cache", action="store_true")
    a = ap.parse_args()
    scratch = a.scratch.resolve()
    scratch.mkdir(parents=True, exist_ok=True)
    runner = C.Runner(a.probe.resolve(), scratch, not a.no_cache)
    return fs291(runner, scratch)


if __name__ == "__main__":
    sys.exit(main())
