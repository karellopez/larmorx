# SPDX-License-Identifier: Apache-2.0
"""FSL's decision on single betsurf profiles (black box).

``python -P betsurf_profile_oracle.py <t1> <t2|-> <matrix> <dir> <profiles.jsonl> [i ...]``
reads profiles (one JSON object per line: the voxel ``o`` they start from and the normal ``n``,
as the original's probe dump writes them) and, for each, writes a tiny triangle (0.2 mm across)
centred near the voxel's position in FSL's millimetres with that normal, slightly closer to
its first vertex: bet2's rasterisation draws it into that one voxel, and only the first vertex
casts a profile from it (the others are nearer to it than to the voxel), so betsurf casts
exactly one profile, from that voxel along that normal. FSL 6.0.7.23's ``betsurf -v`` then says
whether it set the profile aside (``removed profiles : 100%``) or kept it (``0%``); the
clean-room original (``LARMORX_BIN``) runs on the same files. One line per profile:
``<i> <voxel> FSL <removed> larmorx <removed>``. The T1 (and T2) are used whole, so that the
profile's samples are those of the full image (the T1's robust maximum is then that of the
whole image: the tiny triangle encloses nothing).
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
import fslrun


def removed(text: str) -> str:
    n = re.search(r"nb of profiles : (\d+)", text)
    r = re.search(r"removed profiles : (\S+)%", text)
    return f"{n[1] if n else '?'}:{r[1] if r else '?'}"


def triangle(x: np.ndarray, n: np.ndarray, size: float = 0.2) -> np.ndarray:
    """Three points around ``x`` in the plane normal to ``n``, ordered so that
    ``(v2 - v0) x (v1 - v0)`` points along ``n`` (bet2's outward orientation), shifted towards
    the first one."""
    n = n / np.linalg.norm(n)
    a = np.array([1.0, 0, 0]) if abs(n[0]) < 0.9 else np.array([0, 1.0, 0])
    u = np.cross(n, a)
    u /= np.linalg.norm(u)
    w = np.cross(n, u)
    pts = [x + size * (np.cos(t) * u + np.sin(t) * w) for t in (0, 2 * np.pi / 3, 4 * np.pi / 3)]
    v0, v1, v2 = pts
    if np.dot(np.cross(v2 - v0, v1 - v0), n) < 0:
        v1, v2 = v2, v1
    # moved by -0.3 (v0 - x): v0 nearest the voxel's position, so only v0 casts its profile
    return np.array([v0, v1, v2]) - 0.3 * (v0 - x)


def main(argv: list[str]) -> int:
    t1p, t2p, matp, outp, dumpp, *which = argv
    out = Path(outp)
    out.mkdir(parents=True, exist_ok=True)
    d = np.array(nib.load(t1p).header.get_zooms()[:3], float)
    recs = [json.loads(ln[5:] if ln.startswith("DUMP ") else ln)
            for ln in Path(dumpp).read_text().splitlines() if ln.strip().startswith(("{", "DUMP "))]
    idx = [int(i) for i in which] if which else range(len(recs))
    binary = os.environ.get("LARMORX_BIN", "larmorx")
    for i in idx:
        r = recs[i]
        x = np.array(r["o"], float) * d
        v = triangle(x, np.array(r["n"], float))
        # the voxel must hold the drawn triangle: keep it within 0.25 voxel of x
        dk = out / f"p{i}"
        dk.mkdir(exist_ok=True)
        # ORACLE_COPIES=k: k copies of the triangle (separate meshes in one file), so that the
        # one profile votes k times and the surfaces' positions (not only the decision) can be
        # compared: the three meshes FSL writes against the original's
        copies = int(os.environ.get("ORACLE_COPIES", "1"))
        lines = ["# vtk DataFile Version 3.0", "surface file", "ASCII", "DATASET POLYDATA",
                 f"POINTS {3 * copies}  float"]
        lines += [" ".join(f"{c:.6f}" for c in p) for _ in range(copies) for p in v]
        lines += [f"POLYGONS {copies} {4 * copies}"]
        lines += [f"3 {3 * c} {3 * c + 1} {3 * c + 2} " for c in range(copies)]
        (dk / "mesh.vtk").write_text("\n".join(lines) + "\n", encoding="utf-8")
        images = [t1p] + ([] if t2p == "-" else [t2p])
        opts = ["-v"] if t2p != "-" else ["-v", "-1"]
        args = [*opts, *images, str(dk / "mesh.vtk"), matp]
        f = fslrun.run("betsurf", [*args, "o"], dk / "fsl")
        env = dict(os.environ, LARMORX_IMPLEMENTATION="original", FSLOUTPUTTYPE="NIFTI_GZ",
                   LX_BETSURF_DUMP="1")
        if copies == 1:
            env["LX_BETSURF_DEBUG"] = "stop"
        else:
            env["LX_BETSURF_DEBUG"] = "1"
        lx = subprocess.run([binary, "fsl", "betsurf", *args, str(dk / "lx")], env=env,
                            capture_output=True, text=True, check=False)
        (dk / "dump.txt").write_text(lx.stderr, encoding="utf-8")
        mine = re.search(r"DEBUG profiles (\d+) dropped (\d+) scalp_only (\d+)", lx.stderr)
        lxr = f"{mine[1]}:{int(mine[2]) + int(mine[3])}" if mine else "?"
        meshes = ""
        if copies > 1:
            for s in ("inskull", "outskull", "outskin"):
                a, b = dk / "fsl" / f"o_{s}_mesh.vtk", dk / f"lx_{s}_mesh.vtk"
                if a.is_file() and b.is_file():
                    pa = np.array(a.read_text().split("POLYGONS")[0].split()[13:], float)
                    pb = np.array(b.read_text().split("POLYGONS")[0].split()[13:], float)
                    dist = np.abs(pa - pb).max() if pa.shape == pb.shape else float("inf")
                    meshes += f" {s} {dist:.6f}"
        print(i, r["o"], "FSL", removed(f.stdout), "larmorx", lxr, meshes, flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
