# SPDX-License-Identifier: Apache-2.0
"""FSL-187 in BET's and FLIRT's clean-room originals, by black-box runs (sub-task 4 of fslmaths'
and fslstats' clean-room part): does a value far above a histogram's range count in the first
bin (FSL's behaviour), and does ``reproduce`` give FSL's?

Inputs (made in ``<dir>``, fixed bytes): ``fa``, ``fb``, 24^3 smooth images in [0, 0.99) (``fb``
``fa`` shifted by a voxel), each with one voxel at 1e12, voxel sizes 2 mm (``fa0``, ``fb0``: the
same without it); ``outl``, the replica
suite's 7 x 6 x 5 image of values in [0, 1) with one at 1e12 (its ``d13/e-far``). Runs:

- the robust range of each (``larmorx fsl fslstats -r``: the default counts the far value in the
  last bin, ``reproduce`` in the first, as FSL);
- ``bet2 <outl> -v`` by FSL 6.0.7.23 (through the oracle cache) and by BET's original in both
  modes: the thresholds ``thresh2``, ``thresh98`` it prints are its robust range;
- ``flirt -in fa -ref fb -init ident -schedule measurecost1.sch -cost corratio`` (``normmi``,
  ``mutualinfo``), with and without the far value, by FSL and by FLIRT's original in both
  modes: the cost at the identity, whose histograms' bins the images' robust ranges may set.

Run (in a memory box): ``python -P probe_f187.py <dir> <larmorx binary>``. Since relay 5 the
originals' ``reproduce`` gives FSL's first bin (findings 22 of
``docs/findings/fsl-maths-original.md``): its lines show FSL's numbers in that mode.
"""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

import numpy as np
from larmorx_validation import oracle_cache
from larmorx_validation.parity import fsl_stats_replica as rep

IDENT = "1 0 0 0\n0 1 0 0\n0 0 1 0\n0 0 0 1\n"


def _inputs(d: Path) -> None:
    import nibabel as nib

    x, y, z = np.meshgrid(*(np.arange(24),) * 3, indexing="ij")
    a = ((np.sin(x / 3.0) * np.cos(y / 4.0) + 1.2) * (z + 3) / 30.0).astype(np.float32)
    a = a / a.max() * 0.99
    b = np.roll(a, 1, axis=0).copy()
    a0, b0 = a.copy(), b.copy()
    a[20, 20, 20] = 1e12
    b[20, 20, 20] = 1e12
    aff = np.diag([2.0, 2.0, 2.0, 1.0])
    for n, v in (("fa", a), ("fb", b), ("fa0", a0), ("fb0", b0)):
        p = d / f"{n}.nii.gz"
        if not p.exists():
            im = nib.Nifti1Image(v, aff)
            im.header.set_sform(aff, 1)
            im.header.set_qform(aff, 1)
            nib.save(im, str(p))
    out = d / "outl.nii.gz"
    if not out.exists():
        out.write_bytes((rep.inputs() / "outl.nii.gz").read_bytes())
    (d / "ident.mat").write_text(IDENT, encoding="utf-8")


def _fsl(d: Path, program: str, args: list[str]) -> str:
    argv = [str(rep.fsldir() / "bin" / program), *args]
    p = oracle_cache.run(
        argv,
        env=rep._environment({}),
        cwd=d,
        outputs=[d],
        policy=oracle_cache.Policy(threads=1, family="fsl"),
        runner="cleanroom-maths-f187",
        inputs=[
            d / n for n in ("fa.nii.gz", "fb.nii.gz", "fa0.nii.gz", "fb0.nii.gz", "outl.nii.gz")
        ]
        + [d / "ident.mat"],
        timeout=600,
    )
    return p.stdout + p.stderr


def _ours(d: Path, lx: str, program: str, args: list[str], mode: str) -> str:
    env = {
        k: v
        for k, v in os.environ.items()
        if not k.startswith("FSL") and k not in ("LARMORX_UPSTREAM_DEFECTS",)
    }
    env.update(
        LARMORX_IMPLEMENTATION="original",
        LARMORX_UPSTREAM_DEFECTS=mode,
        FSLOUTPUTTYPE="NIFTI_GZ",
        OMP_NUM_THREADS="4",
    )
    p = subprocess.run(
        [lx, "fsl", program, *args], cwd=d, env=env, capture_output=True, text=True, check=False
    )
    return p.stdout + p.stderr


def _line(text: str, word: str) -> str:
    return next((ln.strip() for ln in text.splitlines() if word in ln), "")


def main(argv: list[str]) -> int:
    d, lx = Path(argv[0]), argv[1]
    d.mkdir(parents=True, exist_ok=True)
    _inputs(d)
    for n in ("fb", "outl"):
        for m in ("fix", "reproduce"):
            r = _ours(d, lx, "fslstats", [f"{n}.nii.gz", "-r"], m).splitlines()[0]
            print(f"fslstats {n} -r ({m}): {r}")
    bet = ["outl.nii.gz", "b_out", "-v"]
    print(f"bet2 FSL: {_line(_fsl(d, 'bet2', bet), 'thresh2')}")
    for m in ("fix", "reproduce"):
        print(f"bet2 original ({m}): {_line(_ours(d, lx, 'bet2', bet, m), 'thresh2')}")
    sch = str(rep.fsldir() / "etc" / "flirtsch" / "measurecost1.sch")
    for cost in ("corratio", "normmi", "mutualinfo"):
        for far in ("", "0"):
            args = ["-in", f"fa{far}.nii.gz", "-ref", f"fb{far}.nii.gz", "-init", "ident.mat"]
            args += ["-schedule", sch, "-cost", cost, "-omat", f"c_{cost}{far}.mat"]
            what = "with the far value" if not far else "without it"
            print(f"flirt {cost} {what} FSL: {_fsl(d, 'flirt', args).split()[:1]}")
            for m in ("fix", "reproduce"):
                got = _ours(d, lx, "flirt", args, m).split()[:1]
                print(f"flirt {cost} {what} original ({m}): {got}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
