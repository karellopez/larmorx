# SPDX-License-Identifier: Apache-2.0
"""Probe behind SDC-07's measurement: are lx.fsl.topup's field coefficients FSL's?

``measure_sdc_g.py`` runs TOPUP through ``lx.fsl.topup`` (the clean-room original by default).
This runs FSL's own ``topup`` (through the oracle-output cache) and ``lx.fsl.topup`` on the
same synthetic anterior-posterior blip pair (LAS, with sform/qform codes 2/0 and 1/1) and
prints the correlation of the two coefficient arrays, and of one with the other mirrored
left-right (the order the sign question depends on). Measured 2026-11-08: r = 1.0000 (mirrored
0.6286) in both cases.

    OPENBLAS_CORETYPE=Haswell PYTHONPATH=<repo>/python:<repo>/validation/src \\
        LARMORX_WORKSPACE=<workspace> python -P probe_sdc07_topup.py
"""

import sys
import tempfile
from pathlib import Path

import nibabel as nb
import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import measure_sdc_g as m
from larmorx_validation import oracle_cache
from larmorx_validation.parity import fsl_versions

import larmorx as lx


def main() -> int:
    rng = np.random.default_rng(7)
    u = m.phantom(rng)
    shift = m.true_field() * m.RO
    aff = m.affine_of("LAS")
    blips = [m.distorted(u, shift, 1, +1.0), m.distorted(u, shift, 1, -1.0)]
    pair = np.stack(blips, -1).astype(np.float32)
    acq = np.array([[0, 1, 0, m.RO], [0, -1, 0, m.RO]])
    for codes in ("sform 2, qform 0", "sform 1, qform 1"):
        with tempfile.TemporaryDirectory() as tmp:
            t = Path(tmp)
            img = nb.Nifti1Image(pair, aff)
            if codes.endswith("1"):
                img.header.set_qform(aff, 1)
                img.header.set_sform(aff, 1)
            img.to_filename(t / "pair.nii.gz")
            np.savetxt(t / "acq.txt", acq, fmt="%d %d %d %.8f")
            env = {
                "PATH": "/usr/bin:/bin",
                "FSLDIR": str(fsl_versions.fsldir()),
                "HOME": str(t),
                "FSLOUTPUTTYPE": "NIFTI_GZ",
                **fsl_versions.openblas_env(),
            }
            r = oracle_cache.run(
                [
                    str(fsl_versions.program("topup")),
                    "--imain=pair",
                    "--datain=acq.txt",
                    f"--config={m.SDCFLOWS_DATA / 'b02b0.cnf'}",
                    "--out=tu",
                    "--fout=fo",
                ],
                env=env,
                cwd=t,
                outputs=[t],
                inputs=[t / "pair.nii.gz", t / "acq.txt"],
                policy=oracle_cache.Policy(threads=1, family="fsl"),
                runner="sdcflows-sdc07",
                timeout=1800,
            )
            fsl = nb.load(t / "tu_fieldcoef.nii.gz").get_fdata()
            tu = lx.fsl.topup(t / "pair.nii.gz", acq, config=m.SDCFLOWS_DATA / "b02b0.cnf")
            mine = np.asarray(tu.coefficients.data, dtype=float)
            same = np.corrcoef(fsl.ravel(), mine.ravel())[0, 1]
            mirrored = np.corrcoef(fsl[::-1].ravel(), mine.ravel())[0, 1]
            print(f"{codes}: FSL exit {r.returncode}, r = {same:.4f} (mirrored {mirrored:.4f})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
