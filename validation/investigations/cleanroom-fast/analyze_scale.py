# SPDX-License-Identifier: Apache-2.0
"""Whether FSL 6.0.7.23 applies an ANALYZE 7.5 header's ``funused1`` (SPM's scale factor) when it
reads the image: FSL's ``flirt -applyxfm`` with the identity on FSL's standard CSF prior
(``data/standard/tissuepriors/avg152T1_csf``, big-endian uint8, ``funused1`` = 1/255), with and
without ``-datatype float``, through the oracle cache, against the clean-room original's FLIRT
(``larmorx fsl flirt``, ``reproduce``; ``LARMORX_FAST_BIN`` a ``larmorx`` binary, else this
Python's package). FAST's ``-a`` registers this prior (the findings'
"The standard priors"). A development tool of FAST's clean-room original (relay 2,
2026-11-03).

    PYTHONPATH=python:validation/src python -P validation/investigations/cleanroom-fast/analyze_scale.py

Run it inside scripts/orchestration/boxed.sh.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation import oracle_cache
from larmorx_validation.parity.fsl_flirt_replica import _environment, fsldir


def main() -> None:
    src = fsldir() / "data" / "standard" / "tissuepriors"
    for extra in ([], ["-datatype", "float"]):
        with tempfile.TemporaryDirectory(prefix="lx-fast-anz_") as t:
            d = Path(t)
            for ext in (".hdr", ".img"):
                shutil.copy(src / f"avg152T1_csf{ext}", d / f"csf{ext}")
            (d / "ident.mat").write_text("1 0 0 0\n0 1 0 0\n0 0 1 0\n0 0 0 1\n")
            args = ["-in", "csf", "-ref", "csf", "-applyxfm", "-init", "ident.mat", "-out"]
            env = _environment({"FSLOUTPUTTYPE": "NIFTI_GZ"})
            p = oracle_cache.run(
                [str(fsldir() / "bin" / "flirt"), *args, "fsl", *extra],
                env=env,
                cwd=d,
                outputs=[d],
                policy=oracle_cache.Policy(threads=1, family="fsl"),
                runner="cleanroom-fast",
                inputs=[q for q in sorted(d.iterdir()) if q.is_file()],
                timeout=600,
            )
            exe = os.environ.get("LARMORX_FAST_BIN")
            lx = [exe] if exe else [sys.executable, "-m", "larmorx"]
            env2 = {**env, "LARMORX_UPSTREAM_DEFECTS": "reproduce"}
            q = subprocess.run(
                [*lx, "fsl", "flirt", *args, "orig", *extra],
                cwd=d,
                env=env2,
                capture_output=True,
                text=True,
                check=False,
            )
            for who, code in (("fsl", p.returncode), ("orig", q.returncode)):
                f = d / f"{who}.nii.gz"
                if not f.is_file():
                    print(f"{extra} {who}: exit {code}, no output")
                    continue
                im = nib.load(str(f))
                raw = np.asarray(im.dataobj.get_unscaled())
                print(
                    f"{extra or ['default']} {who}: {im.header.get_data_dtype()} "
                    f"raw {raw.min()}..{raw.max()}"
                )


if __name__ == "__main__":
    main()
