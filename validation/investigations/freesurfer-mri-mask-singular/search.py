# SPDX-License-Identifier: Apache-2.0
"""Transforms that invert but whose voxel-to-voxel form is singular in single precision (FS-1135).

Writes an oblique 8^3 pair of volumes (voxels 1.1 x 0.7 x 1.3 mm, rotated) and ``--count`` linear
transform arrays of a seeded random rank-2 3 x 3 matrix moved one to three ulps (``float``) in one
element, with the pair's geometry, into ``--out``; runs ``<program> -xform <lta> in.mgz mask.mgz
o.mgz`` on each (``--program``: the official ``mri_mask`` or a candidate's command line) and counts
the exit statuses: 0 (the matrix inverts in both forms), 252 (``TransformInvert: xform
noninvertible``: its own matrix is singular), and the cases of FS-1135 (``MRIlinearTransform: xform
is singular``: 252 or, with ``-T`` given, 139 in FreeSurfer). The open probe
``freesurfer-mri-mask-probe`` keeps two of the matrices found (seeds 11 and 12, cases 4 and 44).

    scripts/orchestration/boxed.sh 4 python -P validation/investigations/freesurfer-mri-mask-singular/search.py \\
        --out <folder> --program '<mri_mask or candidate>' [--type 0|1] [--seed 11] [--count 400]
"""

from __future__ import annotations

import argparse
import shlex
import subprocess
from collections import Counter
from pathlib import Path

import nibabel as nib
import numpy as np


def geometry(img_path: Path) -> str:
    img = nib.load(str(img_path))
    n = img.shape[0]
    vs = img.header.get_zooms()[:3]
    m = img.affine[:3, :3] / np.array(vs)
    c = img.affine @ np.array([n / 2, n / 2, n / 2, 1.0])

    def e(v: float) -> str:
        return f"{v:.15e}"

    return (
        f"valid = 1  # volume info valid\nfilename = {img_path.name}\nvolume = {n} {n} {n}\n"
        f"voxelsize = {e(vs[0])} {e(vs[1])} {e(vs[2])}\n"
        f"xras   = {e(m[0, 0])} {e(m[1, 0])} {e(m[2, 0])}\n"
        f"yras   = {e(m[0, 1])} {e(m[1, 1])} {e(m[2, 1])}\n"
        f"zras   = {e(m[0, 2])} {e(m[1, 2])} {e(m[2, 2])}\n"
        f"cras   = {e(c[0])} {e(c[1])} {e(c[2])}\n"
    )


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--program", required=True)
    ap.add_argument("--type", type=int, default=0, help="0 voxel to voxel, 1 scanner to scanner")
    ap.add_argument("--seed", type=int, default=11)
    ap.add_argument("--count", type=int, default=400)
    ap.add_argument("--extra", default="", help="options given before -xform (e.g. '-T 1')")
    a = ap.parse_args(argv)
    a.out.mkdir(parents=True, exist_ok=True)
    vs = np.array([1.1, 0.7, 1.3])
    th, ph = 0.3, 0.2
    r = np.array([[np.cos(th), -np.sin(th), 0], [np.sin(th), np.cos(th), 0], [0, 0, 1]])
    r = r @ np.array([[1, 0, 0], [0, np.cos(ph), -np.sin(ph)], [0, np.sin(ph), np.cos(ph)]])
    ob = np.eye(4)
    ob[:3, :3] = r * vs
    ob[:3, 3] = [3.1, -7.3, 11.9]
    g = np.random.default_rng(7)
    for name in ("in.mgz", "mask.mgz"):
        nib.save(nib.MGHImage(g.uniform(0, 4, (8, 8, 8)).astype(np.float32), ob), str(a.out / name))
    gm, gv = geometry(a.out / "mask.mgz"), geometry(a.out / "in.mgz")
    rng = np.random.default_rng(a.seed)
    counts: Counter = Counter()
    found = []
    for k in range(a.count):
        u, v = rng.normal(size=3), rng.normal(size=3)
        rng.normal(size=3)  # a third draw, unused (relay 52's search made it: its cases)
        m3 = np.outer(u, [1, 0, 0]) + np.outer(v, [0, 1, 0]) + np.outer(u + v, [0, 0, 1])
        m3 = m3.astype(np.float32)
        i, j = rng.integers(0, 3, 2)
        for _ in range(int(rng.integers(1, 4))):
            m3[i, j] = np.nextafter(m3[i, j], np.float32(100))
        m = np.eye(4, dtype=np.float32)
        m[:3, :3] = m3
        m[:3, 3] = rng.normal(size=3)
        rows = "\n".join(" ".join(f"{float(x):.9e}" for x in row) for row in m)
        lta = a.out / f"l{k}.lta"
        lta.write_text(
            f"type      = {a.type} # x\nnxforms   = 1\nmean      = 0.0000 0.0000 0.0000\n"
            f"sigma     = 1.0000\n1 4 4\n{rows}\nsrc volume info\n{gm}dst volume info\n{gv}",
            encoding="utf-8",
        )
        p = subprocess.run(
            [*shlex.split(a.program), *shlex.split(a.extra), "-xform", lta.name, "in.mgz",
             "mask.mgz", "o.mgz"],
            cwd=a.out,
            capture_output=True,
            text=True,
            errors="replace",
            timeout=60,
        )  # fmt: skip
        status = 128 - p.returncode if p.returncode < 0 else p.returncode
        singular = "xform is singular" in p.stderr
        counts[(status, singular)] += 1
        if singular:
            found.append(k)
    for (status, singular), n in sorted(counts.items()):
        print(f"status {status}{' (singular as voxel to voxel)' if singular else ''}: {n}")
    print("cases singular as voxel to voxel:", found[:20], "..." if len(found) > 20 else "")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
