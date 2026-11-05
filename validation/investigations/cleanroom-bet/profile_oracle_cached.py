# SPDX-License-Identifier: Apache-2.0
"""FSL's decision on single betsurf profiles, through the oracle-output cache (black box).

``python -P profile_oracle_cached.py <t1> <t2|-> <matrix> <dir> <dump> [--jobs 16] [i ...]``:
as ``betsurf_profile_oracle.py`` (a 0.2 mm triangle drawn into one voxel casts exactly one profile
from that voxel along a chosen normal, and FSL 6.0.7.23's ``betsurf -v`` says whether it set the
profile aside: ``removed profiles : 100%``, or kept it: ``0%``), with FSL run through the cache
(``use`` mode) and in parallel; the original (``LARMORX_BIN``) runs on the same files. ``<dump>`` holds one profile per line (``DUMP {...}``: the voxel
``o``, the normal ``n`` and the original's decision ``est``, as a temporary probe build of the
original writes them into the file ``LX_BETSURF_DUMP`` names, never a product switch). One line per profile: ``<i> <voxel> FSL
<profiles>:<removed> original <profiles>:<removed> <its decision>``; with ``--copies k`` the triangle is written k times, so the
profile votes k times and FSL's three meshes say where its estimates lie. The clean-room catch-up,
relay 7 (2026-11-05)."""

from __future__ import annotations

import argparse
import concurrent.futures
import json
import os
import re
import subprocess
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
from betsurf_profile_oracle import triangle


def removed(text: str) -> str:
    n = re.search(r"nb of profiles : (\d+)", text)
    r = re.search(r"removed profiles : (\S+)%", text)
    return f"{n[1] if n else '?'}:{r[1] if r else '?'}"


def one(job: tuple) -> str:
    from larmorx_validation.parity import fsl_bet as fb

    i, rec, t1p, t2p, matp, out, copies = job
    d = np.array(nib.load(t1p).header.get_zooms()[:3], float)
    x = np.array(rec["o"], float) * d
    v = triangle(x, np.array(rec["n"], float))
    dk = Path(out) / f"p{i}"
    dk.mkdir(parents=True, exist_ok=True)
    lines = [
        "# vtk DataFile Version 3.0",
        "surface file",
        "ASCII",
        "DATASET POLYDATA",
        f"POINTS {3 * copies}  float",
    ]
    lines += [" ".join(f"{c:.6f}" for c in p) for _ in range(copies) for p in v]
    lines += [f"POLYGONS {copies} {4 * copies}"]
    lines += [f"3 {3 * c} {3 * c + 1} {3 * c + 2} " for c in range(copies)]
    (dk / "mesh.vtk").write_text("\n".join(lines) + "\n", encoding="utf-8")
    images = [t1p] + ([] if t2p == "-" else [t2p])
    opts = ["-v"] if t2p != "-" else ["-v", "-1"]
    args = [*opts, *images, str(dk / "mesh.vtk"), matp, "o"]
    r = fb.oracle_run("betsurf", args, fb.environment("NIFTI_GZ", ()), dk)
    # the original on the same files (LARMORX_BIN, the probe build), its decision and samples
    dump = dk / "dump.jsonl"
    dump.unlink(missing_ok=True)
    env = dict(
        os.environ,
        LARMORX_IMPLEMENTATION="original",
        FSLOUTPUTTYPE="NIFTI_GZ",
        LX_BETSURF_DUMP=str(dump),
    )
    (dk / "lx").mkdir(exist_ok=True)
    lx = subprocess.run(
        [
            os.environ.get("LARMORX_BIN", "larmorx"),
            "fsl",
            "betsurf",
            *args[:-1],
            str(dk / "lx" / "o"),
        ],
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )
    (dk / "stderr.txt").write_text(lx.stderr, encoding="utf-8")
    text = dump.read_text(encoding="utf-8") if dump.exists() else ""
    est = next(
        (json.loads(ln[5:])["est"] for ln in text.splitlines() if ln.startswith("DUMP ")), "?"
    )
    return (
        f"{i} {rec['o']} FSL {removed(r.out)} original {removed(lx.stdout)} {est} "
        f"(full run: {rec.get('est')})"
    )


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("t1")
    ap.add_argument("t2")
    ap.add_argument("matrix")
    ap.add_argument("dir")
    ap.add_argument("dump")
    ap.add_argument("which", nargs="*", type=int)
    ap.add_argument("--jobs", type=int, default=16)
    ap.add_argument("--copies", type=int, default=1)
    a = ap.parse_args(argv)
    recs = [
        json.loads(ln[5:])
        for ln in Path(a.dump).read_text(encoding="utf-8").splitlines()
        if ln.startswith("DUMP ")
    ]
    idx = a.which or range(len(recs))
    jobs = [(i, recs[i], a.t1, a.t2, a.matrix, a.dir, a.copies) for i in idx]
    with concurrent.futures.ProcessPoolExecutor(a.jobs) as ex:
        for line in ex.map(one, jobs):
            print(line, flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
