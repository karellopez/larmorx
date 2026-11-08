# SPDX-License-Identifier: Apache-2.0
"""How tedana's t2smap ends without --mask when nilearn's compute_epi_mask finds nothing (TED-03,
relay 5b): a constant image (a mask is found), a small homogeneous one (empty mask, then
IndexError in the adaptive mask), one voxel (argmax ValueError). Run: <env>/bin/python -I
epi_mask_failures.py <empty dir>"""

import os
import subprocess
import sys

import nibabel as nb
import numpy as np

d = sys.argv[1]
os.makedirs(d, exist_ok=True)
rng = np.random.default_rng(3)
aff = np.diag([2.0, 2, 2, 1])


def write(name, shape, f):
    files = []
    for e, te in enumerate([0.015, 0.03, 0.045]):
        x = f(shape, te)
        p = os.path.join(d, f"{name}-e{e + 1}.nii.gz")
        nb.Nifti1Image(x.astype(np.float32), aff).to_filename(p)
        files.append(p)
    return files


cases = {
    "const": write("const", (6, 6, 6, 5), lambda s, te: np.full(s, 1000 * np.exp(-te / 0.03))),
    "small": write(
        "small",
        (5, 5, 5, 5),
        lambda s, te: 1000 * np.exp(-te / 0.03) * (1 + 0.01 * rng.standard_normal(s)),
    ),
    "onevox": write(
        "onevox",
        (1, 1, 1, 5),
        lambda s, te: 1000 * np.exp(-te / 0.03) * (1 + 0.01 * rng.standard_normal(s)),
    ),
}
for name, files in cases.items():
    out = os.path.join(d, name + "-out")
    r = subprocess.run(
        [
            sys.executable,
            "-c",
            "import sys; from tedana.workflows import t2smap; t2smap._main(sys.argv[1:])",
            "-d",
            *files,
            "-e",
            "0.015",
            "0.03",
            "0.045",
            "--out-dir",
            out,
        ],
        capture_output=True,
        text=True,
    )
    print("==", name, r.returncode)
    print("\n".join(r.stderr.splitlines()[-12:]))
