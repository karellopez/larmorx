# SPDX-License-Identifier: Apache-2.0
"""Two of ``specs/freesurfer-mri.md`` §9's statements checked on the prebuilt probe as models of the
printed numbers (black box: the probe's own printed matrices and the float product of
``specs/freesurfer-maths.md`` §8.1, nothing else).

    python -P models.py surface --probe P --scratch DIR [--n 40]
        surfaceRASFromRAS_ of geometries with oblique cosines is the float product K x inv(T) of the
        tkregister vox2ras K and the scanner vox2ras's inverse inv(T) the probe's ``tkreg`` command
        prints, bit for bit; counts how often it differs from VGras2tkreg's matrix (the inverse of
        tkregister RAS to scanner RAS)
    python -P models.py fs331 --probe P --scratch DIR
        FS-331: every line of the batch ``tkreg-given`` (compare.py) against the model: the inverse
        of K written row by row from the given matrix's first number, the end of the program when one
        of its elements (r, c), r, c <= 4, is not finite, else the product of T and the whole given
        matrix (4 rows) or nothing (more rows)

The probe runs through the oracle-output cache (one thread, the family ``freesurfer``) when the
validation package is importable (``PYTHONPATH=validation/src``), else directly. Exit status 1 when a
line differs from its model.
"""

from __future__ import annotations

import argparse
import importlib.util
import random
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent


def _compare():
    spec = importlib.util.spec_from_file_location("fs_mri_compare", HERE / "compare.py")
    mod = importlib.util.module_from_spec(spec)
    sys.modules["fs_mri_compare"] = mod
    spec.loader.exec_module(mod)
    return mod


C = _compare()


def f32(h: str) -> np.float32:
    return np.frombuffer(bytes.fromhex(h)[::-1], dtype=np.float32)[0]


def matrices(tokens: list[str]) -> list[np.ndarray | None]:
    """The matrices of a printed result (``given`` markers skipped, ``null`` as None)."""
    out: list[np.ndarray | None] = []
    i = 0
    while i < len(tokens):
        if tokens[i] == "given":
            i += 1
            continue
        if tokens[i] == "null":
            out.append(None)
            i += 1
            continue
        r, c = int(tokens[i]), int(tokens[i + 1])
        i += 2
        vals = [f32(t) for t in tokens[i : i + r * c]]
        out.append(np.array(vals, dtype=np.float32).reshape(r, c))
        i += r * c
    return out


def fproduct(a: np.ndarray, b: np.ndarray) -> np.ndarray:
    """The float product of specs/freesurfer-maths.md §8.1: each element the float sum, from 0, of the
    float products in order."""
    out = np.zeros((a.shape[0], b.shape[1]), dtype=np.float32)
    with np.errstate(all="ignore"):
        for i in range(a.shape[0]):
            for j in range(b.shape[1]):
                s = np.float32(0.0)
                for k in range(a.shape[1]):
                    s = np.float32(s + np.float32(a[i, k] * b[k, j]))
                out[i, j] = s
    return out


def same(a: np.ndarray, b: np.ndarray) -> bool:
    return a.shape == b.shape and np.array_equal(a.view(np.uint32), b.view(np.uint32))


def run(runner, scratch: Path, name: str, lines: list[str]) -> list[str]:
    path = scratch / f"{name}.txt"
    path.write_text("".join(f"@10 {ln}\n" for ln in lines), encoding="latin-1")
    env = dict(C.BASE_ENV)
    out = runner._run([str(runner.probe), "batch", str(path)], env, "", "freesurfer-mri-probe")
    return [ln.decode("latin-1") for ln in C.parse(out)]


def geometry_of(line: str) -> str:
    """The geometry G of a ``ras2s V G M`` line (V is 6 numbers, G 24)."""
    return " ".join(line.split()[7:31])


def surface(runner, scratch: Path, n: int) -> int:
    r = random.Random(5)
    lines = []
    for _ in range(n):
        cos = [r.uniform(-1.0, 1.0) for _ in range(9)]
        sizes = [r.choice([1.0, 0.9375, 1.5, 2.0, 3.0]) for _ in range(3)]
        dims = [r.randint(1, 7) for _ in range(3)]
        centre = [r.uniform(-100.0, 100.0) for _ in range(3)]
        g = C.gfix(*dims, sizes=tuple(sizes), cos=tuple(cos), centre=tuple(centre))
        lines += [f"tkreg {C.V2} {g} 0 0", f"ras2s {C.V2} {g} null"]
    out = run(runner, scratch, "surface", lines)
    equal = differ = 0
    for k in range(0, len(out), 2):
        t = matrices(out[k].split(" | ")[0].split())
        s = matrices(out[k + 1].split(" | ")[0].split())
        model = fproduct(t[0], t[3])  # K, inv(T) (README.md, the tkreg command)
        equal += same(model, s[3])
        differ += not same(s[2], s[3])
    print(
        f"surface: {n} geometries: surfaceRASFromRAS_ equal to K x inv(T) in {equal}, "
        f"different from VGras2tkreg's matrix in {differ}"
    )
    return 0 if equal == n else 1


def fs331(runner, scratch: Path) -> int:
    lines = C.Gen.tkreg_given()
    out = run(runner, scratch, "tkreg-given", lines)
    geoms: list[str] = []
    for ln in lines:
        if geometry_of(ln) not in geoms:
            geoms.append(geometry_of(ln))
    tk = run(runner, scratch, "tkreg-of-given", [f"tkreg {C.V2} {g} 0 0" for g in geoms])
    mats = {
        g: None if t.startswith("EXIT") else matrices(t.split(" | ")[0].split())
        for g, t in zip(geoms, tk, strict=True)
    }
    good = bad = 0
    for ln, o in zip(lines, out, strict=True):
        tok = ln.split()
        rows, cols = int(tok[31]), int(tok[32])
        given = np.array([f32(x) for x in tok[33 : 33 + rows * cols]], dtype=np.float32)
        res = o.split(" | ")[0].split()
        m = mats[geometry_of(ln)]
        if m is None or m[5] is None:  # K without an inverse: the program ends in the product
            ok = res[:2] == ["EXIT", "252"]
        else:
            flat = given.copy()
            flat[:16] = m[5].flatten()  # inv(K), row by row from the first number
            g = flat.reshape(rows, cols)
            if not np.all(np.isfinite(g[:4, :4])):
                ok = res[:2] == ["EXIT", "252"] and b"m2 is null".hex() in o
            elif rows != 4:
                ok = res[0] == "null" and b"m1/m2 dim mismatch".hex() in o
            else:
                got = matrices(res)[0]
                ok = res[0] == "given" and got is not None and same(got, fproduct(m[2], g))
        good += ok
        bad += not ok
        if not ok and bad <= 3:
            print(f"DIFFERS: {ln[:120]}\n    probe {o[:200]}")
    print(f"fs331: tkreg-given, {len(lines)} lines: as the model {good}, not {bad}")
    return 1 if bad else 0


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("command", choices=["surface", "fs331"])
    ap.add_argument("--probe", type=Path, required=True)
    ap.add_argument("--scratch", type=Path, required=True)
    ap.add_argument("--n", type=int, default=40)
    ap.add_argument("--no-cache", action="store_true")
    a = ap.parse_args()
    scratch = a.scratch.resolve()
    scratch.mkdir(parents=True, exist_ok=True)
    runner = C.Runner(a.probe.resolve(), scratch, not a.no_cache)
    if a.command == "surface":
        return surface(runner, scratch, a.n)
    return fs331(runner, scratch)


if __name__ == "__main__":
    sys.exit(main())
