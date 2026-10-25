# SPDX-License-Identifier: Apache-2.0
"""Which seeds of the suite's moving phantom (``_moving_phantom``) reach ``atanf`` arguments
where fdlibm's comment value of ``aT[0]`` (``0x3eaaaaaa``) and glibc's (``0x3eaaaaab``) differ:
the replica is run twice, built with each constant, on mcflirt's default call with ``-mats
-plots``, and the seeds whose outputs differ are listed (11 of the first 60; the suite's cases
``edge/atanf-phantom-{2,11,27}`` are three of them).

    python -P atanf_phantoms.py <larmorx-nc with 0x3eaaaaaa> <larmorx-nc> <work dir> [n seeds]

The first program is a build of ``crates-nc`` with ``AT0`` set to ``0x3eaaaaaa`` in
``larmorx-nc-core/src/fdlibm.rs`` (a scratch copy; never committed). Run with
``PYTHONPATH=<repository>/validation/src``.
"""

from __future__ import annotations

import gzip
import os
import subprocess
import sys
from pathlib import Path

import nibabel as nib
from larmorx_validation.parity.fsl_mcflirt_replica import _moving_phantom


def outputs(exe: str, d: Path, inp: Path) -> dict[str, bytes]:
    """Every file of the replica's run (``reproduce``), images as their data bytes."""
    d.mkdir(parents=True, exist_ok=True)
    env = dict(
        os.environ,
        LARMORX_UPSTREAM_DEFECTS="reproduce",
        OMP_NUM_THREADS="1",
        FSLOUTPUTTYPE="NIFTI_GZ",
    )
    args = [exe, "fsl", "mcflirt", "-in", str(inp), "-out", "out_mcf", "-mats", "-plots"]
    subprocess.run(args, cwd=d, env=env, capture_output=True, check=False)
    out = {}
    for p in sorted(d.rglob("*")):
        if p.is_file():
            data = p.read_bytes()
            out[p.relative_to(d).as_posix()] = (
                gzip.decompress(data)[352:] if p.name.endswith(".gz") else data
            )
    return out


def main() -> int:
    old, new, work = sys.argv[1], sys.argv[2], Path(sys.argv[3]).resolve()
    n = int(sys.argv[4]) if len(sys.argv) > 4 else 60
    hits = []
    for seed in range(n):
        d = work / f"s{seed}"
        d.mkdir(parents=True, exist_ok=True)
        inp = d / "in.nii.gz"
        nib.save(_moving_phantom(seed), inp)
        a, b = outputs(old, d / "old", inp), outputs(new, d / "new", inp)
        diff = [k for k in a if a.get(k) != b.get(k)]
        if diff:
            hits.append(seed)
            print(seed, "differs:", diff[:4], flush=True)
    print(f"{len(hits)} of {n} seeds reach the constant: {hits}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
