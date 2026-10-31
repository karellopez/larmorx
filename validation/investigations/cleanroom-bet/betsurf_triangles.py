# SPDX-License-Identifier: Apache-2.0
"""betsurf's profiles one triangle at a time (black box).

``python -P betsurf_triangles.py <t1> <t2|-> <mesh.vtk> <matrix> <dir> <first> <count> [step]``
takes triangles ``first, first + step, ...`` of bet2's mesh of ``<t1>`` (a radiologically stored
image, as FSL's coordinates are then the stored voxels times their sizes) and, for each, crops
the T1 (and the T2) to a 48-voxel box around it (``BETSURF_BOX``; 0: the whole image), writes the triangle alone as a mesh (its
coordinates and the matrix moved with the box), and runs FSL 6.0.7.23's betsurf and the
clean-room original (``LARMORX_BIN``) with ``-v``: a single triangle casts a few profiles
along its normal, and the verbose ``nb of profiles`` and ``removed profiles`` lines say how many
of them each program set aside. A difference points at the few profiles where the original's
reading of specs/bet.md §6.4 departs from FSL's. Prints one line per triangle:
``<k> N FSL-removed larmorx-removed``.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
import fslrun

BOX = 48


def read_vtk(p: Path) -> tuple[np.ndarray, np.ndarray]:
    w = p.read_text(encoding="utf-8").split()
    k = w.index("POINTS")
    n = int(w[k + 1])
    pts = np.array(w[k + 3 : k + 3 + 3 * n], float).reshape(n, 3)
    k = w.index("POLYGONS")
    m = int(w[k + 1])
    tri = np.array(w[k + 3 : k + 3 + 4 * m], int).reshape(m, 4)[:, 1:]
    return pts, tri


def removed(text: str) -> tuple[int, int] | None:
    n = re.search(r"nb of profiles : (\d+)", text)
    r = re.search(r"removed profiles : (\S+)%", text)
    if not n or not r:
        return None
    total = int(n[1])
    pct = float(r[1]) if r[1] not in ("-nan", "nan") else 0.0
    return total, round(pct * total / 100)


def main(argv: list[str]) -> int:
    t1p, t2p, meshp, matp, outp, first, count, *rest = argv
    step = int(rest[0]) if rest else 1
    out = Path(outp)
    out.mkdir(parents=True, exist_ok=True)
    t1 = nib.load(t1p)
    if np.linalg.det(t1.affine[:3, :3]) > 0:
        raise SystemExit("a radiologically stored T1 (negative determinant) is needed")
    d = np.array(t1.header.get_zooms()[:3], float)
    a1 = np.asanyarray(t1.dataobj)
    a2 = None if t2p == "-" else np.asanyarray(nib.load(t2p).dataobj)
    pts, tri = read_vtk(Path(meshp))
    mat = np.loadtxt(matp)
    binary = os.environ.get("LARMORX_BIN", "larmorx")
    picked = os.environ.get("TRIS")  # the triangles named (comma-separated) instead
    ks = [int(t) for t in picked.split(",")] if picked else range(int(first), int(first) + int(count) * step, step)
    for k in ks:
        if k >= len(tri):
            break
        v = pts[tri[k]]
        c = np.floor(v.mean(axis=0) / d).astype(int)
        box = int(os.environ.get("BETSURF_BOX", BOX))
        if box > 0:
            lo = np.maximum(c - box // 2, 0)
            hi = np.minimum(lo + box, a1.shape[:3])
        else:
            lo, hi = np.zeros(3, int), np.array(a1.shape[:3])
        crop = tuple(slice(int(lo[a]), int(hi[a])) for a in range(3))
        aff = t1.affine.copy()
        aff[:3, 3] = t1.affine[:3, :3] @ lo + t1.affine[:3, 3]
        dk = out / f"t{k}"
        dk.mkdir(exist_ok=True)
        names = []
        for name, arr in (("t1", a1), ("t2", a2)):
            if arr is None:
                continue
            img = nib.Nifti1Image(np.ascontiguousarray(arr[crop]), aff, t1.header)
            img.set_qform(aff, 1)
            img.set_sform(aff, 1)
            nib.save(img, dk / f"{name}.nii.gz")
            names.append(str(dk / f"{name}.nii.gz"))
        off = lo * d
        lines = ["# vtk DataFile Version 3.0", "surface file", "ASCII", "DATASET POLYDATA",
                 "POINTS 3  float"]
        lines += [" ".join(f"{x:.6f}" for x in p - off) for p in v]
        lines += ["POLYGONS 1 4", "3 0 1 2 "]
        (dk / "mesh.vtk").write_text("\n".join(lines) + "\n", encoding="utf-8")
        m2 = mat.copy()
        m2[:3, 3] = mat[:3, 3] + mat[:3, :3] @ off
        np.savetxt(dk / "m.mat", m2, fmt="%.10g")
        opts = ["-v"] if a2 is not None else ["-v", "-1"]
        args = [*opts, *names, str(dk / "mesh.vtk"), str(dk / "m.mat")]
        f = fslrun.run("betsurf", [*args, "o"], dk / "fsl")
        env = dict(os.environ, LARMORX_IMPLEMENTATION="original", FSLOUTPUTTYPE="NIFTI_GZ")
        lx = subprocess.run([binary, "fsl", "betsurf", *args, str(dk / "lx")], env=env,
                            capture_output=True, text=True, check=False)
        if os.environ.get("LX_BETSURF_DUMP"):
            # a build with the profile dump (a probe of the part's notes) writes each profile
            (dk / "dump.txt").write_text(lx.stderr, encoding="utf-8")
        rf, rl = removed(f.stdout), removed(lx.stdout)
        flag = "" if rf == rl else "  DIFF"
        print(k, rf, rl, flag, flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
