# SPDX-License-Identifier: Apache-2.0
"""FSL-81: FSL 6.0.7.23's aff2rigid stops with ``NameError: name 'mat' is not defined`` (numpy
2.4.6 has no ``mat``). This probe runs FSL's own script in FSL's own Python with the alias put
back (``mat = numpy.asmatrix`` in the script's namespace, nothing else changed) and compares its matrix file, byte
for byte, with the replica's default (corrected) output (``LARMORX_NC_BIN``), on fMRIPrep's
initial matrix, an affine matrix and seeded random affine matrices. FSL's outputs are kept in
``<out>`` and reused (``--fresh`` runs FSL again).

Usage::

    LARMORX_NC_BIN=<larmorx-nc> python -P aff2rigid_mat.py <out> [--fresh]
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path

import numpy as np

FSL = Path.home() / "fsl-6.0.7.23"
INPUTS = Path(
    os.environ.get(
        "LARMORX_FLIRT_INPUTS",
        Path(__file__).resolve().parents[4] / "oracles" / "inputs" / "fsl-flirt-replica",
    )
)
SHIM = (
    "import sys, numpy; sys.argv = sys.argv[1:]; "
    "exec(compile(open(sys.argv[0]).read(), sys.argv[0], 'exec'), "
    "{'__name__': '__main__', 'mat': numpy.asmatrix})"
)


def matrices(rng: np.random.Generator) -> dict[str, str]:
    out = {"aff": "1.05 0.02 -0.03 2\n-0.01 0.97 0.04 -3\n0.02 -0.05 1.1 1.5\n0 0 0 1\n"}
    init = INPUTS / "init.mat"
    if init.is_file():
        out["fmriprep-init"] = init.read_text()
    for k in range(20):
        a = np.eye(4)
        a[:3, :3] += rng.normal(scale=0.1, size=(3, 3))
        a[:3, 3] = rng.normal(scale=10, size=3)
        out[f"random-{k:02d}"] = "\n".join(" ".join(f"{v:.10g}" for v in row) for row in a) + "\n"
    return out


def main() -> int:
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    fresh = "--fresh" in sys.argv
    nc = os.environ["LARMORX_NC_BIN"]
    env = dict(os.environ, FSLDIR=str(FSL), OPENBLAS_CORETYPE="Haswell")
    bad = 0
    for name, text in matrices(np.random.default_rng(81)).items():
        d = out / name
        d.mkdir(exist_ok=True)
        (d / "in.mat").write_text(text)
        kept = d / "fsl.mat"
        if fresh or not kept.is_file():
            subprocess.run([str(FSL / "bin" / "fslpython"), "-c", SHIM,
                            str(FSL / "bin" / "aff2rigid"), "in.mat", "fsl.mat"],
                           cwd=d, env=env, check=True, capture_output=True)
        (d / "rep.mat").unlink(missing_ok=True)
        subprocess.run([nc, "fsl", "aff2rigid", "in.mat", "rep.mat"], cwd=d,
                       env={**env, "LARMORX_UPSTREAM_DEFECTS": "fix"}, check=True,
                       capture_output=True)
        same = kept.read_bytes() == (d / "rep.mat").read_bytes()
        bad += not same
        print(f"{name}: {'identical' if same else 'DIFFER'}")
    print(f"{bad} differ")
    shutil.rmtree(out / "__tmp", ignore_errors=True)
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
