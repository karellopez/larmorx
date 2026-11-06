# SPDX-License-Identifier: Apache-2.0
"""The time step (``pixdim[4]``) FSL 6.0.7.23's programs write for inputs whose time step is 0 or
negative, against larmorx's clean-room originals (a black-box probe; FSL through the oracle cache,
the runners of ``probe.py``): prelude, fslmaths and fslroi on 4D inputs of time step 0 and -2,
whole series and one volume, and prelude on a 3D phase of -2.

    boxed.sh 4 python -P time_step.py <work dir> <larmorx program>   (PYTHONPATH=<checkout>/validation/src)
"""

from __future__ import annotations

import sys
import tempfile
from pathlib import Path

import nibabel as nib
import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import probe


def _save(path: Path, data: np.ndarray, tr: float) -> Path:
    img = nib.Nifti1Image(data, np.diag([2.0, 2.0, 2.0, 1.0]))
    pd = img.header["pixdim"]
    pd[4] = tr
    img.header["pixdim"] = pd
    nib.save(img, path)
    return path


def main() -> int:
    base = Path(tempfile.mkdtemp(prefix="lx-time-step_", dir=sys.argv[1]))
    larmorx = sys.argv[2]
    home = base / "home"
    home.mkdir()
    rng = np.random.default_rng(1)
    x = rng.normal(100, 10, (8, 8, 6, 3)).astype(np.float32)
    ph = np.linspace(-1, 1, x.size, dtype=np.float32).reshape(x.shape)
    differ = 0
    for tr in (0.0, -2.0):
        src = _save(base / f"s{int(tr)}.nii.gz", x, tr)
        phase = _save(base / f"p{int(tr)}.nii.gz", ph, tr)
        phase3 = _save(base / f"q{int(tr)}.nii.gz", ph[..., 0], tr)
        mag = _save(base / "m.nii.gz", np.full(x.shape, 100.0, np.float32), 2.5)
        mag3 = _save(base / "m3.nii.gz", np.full(x.shape[:3], 100.0, np.float32), 2.5)
        calls = (
            ["fslmaths", str(src), "-mul", "2", "out"],
            ["fslmaths", str(src), "-Tmean", "out"],
            ["fslroi", str(src), "out", "0", "2"],
            ["fslroi", str(src), "out", "1", "1"],
            ["prelude", "-p", str(phase), "-a", str(mag), "-o", "out"],
            ["prelude", "-p", str(phase), "-a", str(mag), "-o", "out", "--start=1", "--end=1"],
            ["prelude", "-p", str(phase3), "-a", str(mag3), "-o", "out"],
        )
        for argv in calls:
            env = probe.fsl_env("NIFTI_GZ", home, False)
            res = []
            for who in ("fsl", "original"):
                d = Path(tempfile.mkdtemp(prefix="lx-time-step-run_", dir=base))
                if who == "fsl":
                    code = probe.run_fsl(argv, d, env, []).returncode
                else:
                    code = probe.run_original(larmorx, argv, d, env)
                h = nib.load(d / "out.nii.gz").header
                res.append((code, float(h["pixdim"][4]), int(h["dim"][0])))
            same = res[0] == res[1]
            differ += not same
            words = " ".join(Path(a).name for a in argv)
            print(
                f"time step {tr:4}: {words}: FSL {res[0]}, original {res[1]} {'' if same else 'DIFFER'}"
            )
    print(f"{differ} differ")
    return 1 if differ else 0


if __name__ == "__main__":
    sys.exit(main())
