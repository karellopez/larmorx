# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""applytopup's least-squares restoration (`--method=lsr`) of ds005040's pair with FSL's topup
result (fMRIPrep's call, served by the oracle cache): the original's wall time at the given
thread counts (the best of `--repeat` runs; informal), and its output against FSL's (served by
the cache) by §12's metric. The benchmark's own row (`bench.py`) times FSL and the replica too.

    LARMORX_TOPUP_BIN=<larmorx binary> boxed.sh 8 python -P lsr_speed.py --threads 1 14
"""

import argparse
import shutil
import tempfile
import time
from pathlib import Path

import nibabel as nib
import numpy as np

from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_topup as orig
from larmorx_validation.parity import fsl_topup_replica as rep

RUN = "ds005040-sub-002-noreconall"


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--threads", type=int, nargs="+", default=[1, 14])
    ap.add_argument("--repeat", type=int, default=3)
    a = ap.parse_args()
    node = dict(rep.fmriprep_calls())[RUN]
    args = rep.fmriprep_args(node)
    names = {k: v for k, _, v in (x.partition("=") for x in args)}
    with tempfile.TemporaryDirectory(prefix="lx-lsr-speed-") as t_:
        t = Path(t_)
        src = t / "topup"
        src.mkdir()
        assert rep._prepare_fmriprep(node, src)
        r = rep.run_oracle(args, cwd=src, settings={})
        assert r.code == 0, r.stderr[-300:]
        img = nib.load(str(src / names["--imain"]))
        data = np.asanyarray(img.dataobj)
        base = names["--out"]

        def prepare(d: Path) -> None:
            d.mkdir()
            for k, n in enumerate(("up", "dn")):
                nib.save(
                    nib.Nifti1Image(data[..., k], img.affine, img.header), str(d / f"{n}.nii.gz")
                )
            shutil.copy(src / names["--datain"], d / "enc.txt")
            shutil.copy(src / f"{base}_fieldcoef.nii.gz", d / "tu_fieldcoef.nii.gz")
            shutil.copy(src / f"{base}_movpar.txt", d / "tu_movpar.txt")

        lsr = [
            "--imain=up.nii.gz,dn.nii.gz",
            "--inindex=1,2",
            "--datain=enc.txt",
            "--topup=tu",
            "--out=hifi",
        ]
        run = rep.Run(tuple(lsr), program="applytopup")
        fd = t / "fsl"
        prepare(fd)
        before = {p.name for p in fd.iterdir()}
        r = rep._oracle(run, lsr, fd, {})
        assert r.code == 0, r.stderr[-300:]
        fa = orig._files(fd, before)
        mask = orig.brain_mask(fd, lsr, fa, "applytopup")
        for n in a.threads:
            best = None
            for i in range(a.repeat):
                d = t / f"lx{n}-{i}"
                prepare(d)
                t0 = time.perf_counter()
                q = orig.run_original(
                    lsr, cwd=d, settings={}, mode=defects.FIX, threads=n, program="applytopup"
                )
                s = time.perf_counter() - t0
                assert q.code == 0, q.stderr[-300:]
                best = s if best is None else min(best, s)
            got, _ = orig.measures(fd, d, lsr, fa, mask, "applytopup")
            m = "; ".join(f"{x.name[:50]} {x.value:.3g}" for x in got)
            print(f"original, {n} thread(s): {best:.3f} s (best of {a.repeat}); against FSL: {m}")


main()
