# SPDX-License-Identifier: Apache-2.0
"""fslmaths' morphological filters with a kernel image (``-kernel file``): when FSL prints
``ERROR:: Empty mask image`` and the off-centre warning (findings 25 of
``docs/findings/fsl-maths-original.md``). Kernels of zeros, of values below 0.5, of even and odd
sizes with their values above 0.5 in an odd or even extent; images with and without zero
voxels; ``-dilM``, ``-dilD``, ``-dilF``, ``-ero``, ``-eroF``, ``-fmedian``; FSL 6.0.7.23 through
the oracle cache and the original (``<larmorx>``), their standard error and whether their
outputs are identical.

Run (in a memory box): ``python -P probe_kernel_file.py <dir> <larmorx>``.
"""

import itertools
import subprocess
import sys
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation import oracle_cache
from larmorx_validation.parity import fsl_stats_replica as rep

KERNELS = {
    "zeros3": np.zeros((3, 1, 1)),
    "low3": np.full((3, 1, 1), 0.25),
    "half3": np.full((3, 1, 1), 0.5),
    "ones2": np.ones((2, 1, 1)),
    "odd-dims-even-extent": np.array([[[0.0]], [[1.0]], [[1.0]]]),
    "even-dims-odd-extent": np.array([[[0.0]], [[1.0]], [[0.0]], [[0.0]]]),
    "zero1": np.zeros((1, 1, 1)),
}
IMAGES = {
    "ramp-zeros": lambda: (np.arange(6 * 5 * 4, dtype=np.float32) % 7).reshape((6, 5, 4)),
    "no-zeros": lambda: np.full((6, 5, 4), -2.5, np.float32),
}
OPS = ("-dilM", "-dilD", "-dilF", "-ero", "-eroF", "-fmedian")


def _save(d: Path, name: str, a) -> str:
    p = d / f"{name}.nii.gz"
    if not p.exists():
        nib.save(nib.Nifti1Image(np.asarray(a, np.float32), np.diag([-1.0, 1, 1, 1])), str(p))
    return p.name


def main(argv: list[str]) -> int:
    d = Path(argv[0])
    d.mkdir(parents=True, exist_ok=True)
    exe = argv[1]
    for (kn, k), (im, mk), op in itertools.product(KERNELS.items(), IMAGES.items(), OPS):
        kf, imf = _save(d, f"k_{kn}", k), _save(d, f"i_{im}", mk())
        tag = f"{kn}_{im}{op}".replace("-", "_")
        words = [imf, "-kernel", "file", kf, op]
        p = oracle_cache.run(
            [str(rep.fsldir() / "bin" / "fslmaths"), *words, f"f_{tag}"],
            env=rep._environment({}),
            cwd=d,
            outputs=[d],
            policy=oracle_cache.Policy(threads=1, family="fsl"),
            runner="cleanroom-maths-kernel-file",
            inputs=[d / kf, d / imf],
            timeout=600,
        )
        q = subprocess.run(
            [exe, "fsl", "fslmaths", *words, f"o_{tag}"],
            cwd=d,
            capture_output=True,
            text=True,
            env={"FSLOUTPUTTYPE": "NIFTI_GZ", "OMP_NUM_THREADS": "1",
                 "LARMORX_IMPLEMENTATION": "original", "LARMORX_UPSTREAM_DEFECTS": "reproduce"},
        )  # fmt: skip
        same = "-"
        if p.returncode == 0 and q.returncode == 0:
            a = np.asarray(nib.load(str(d / f"f_{tag}.nii.gz")).dataobj)
            b = np.asarray(nib.load(str(d / f"o_{tag}.nii.gz")).dataobj)
            same = "identical" if np.array_equal(a, b, equal_nan=True) else "DIFFER"
        flag = "" if (p.stderr == q.stderr and p.returncode == q.returncode) else "  <-- streams"
        print(f"{kn:22s} {im:10s} {op:8s} FSL exit {p.returncode} {p.stderr.strip()!r:60.60s} | "
              f"original exit {q.returncode} {q.stderr.strip()!r:60.60s} | {same}{flag}")  # fmt: skip
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
