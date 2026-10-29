# SPDX-License-Identifier: Apache-2.0
"""AFNI-converted types and signed zeros (item 8 of the 2026-10-29 fixes): a float64 head with a
-0 inside the mask, with no scale factor, a NaN one, slope 2 and intercept 0, slope 2 and
intercept 1, through `3dAutomask -apply_prefix` in AFNI 25.2.09 and larmorx's original; prints
the sign of the masked output at that voxel for each. Black-box only.

    python -P signed_zero.py <work>
"""

import os
import subprocess
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

REPO = Path(__file__).resolve().parents[3]
WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE", REPO.parent))
AFNI = Path(os.environ.get("AFNI_BIN", WORKSPACE / "oracles/afni-25.2.09/bin"))
LX = os.environ.get("LX_BIN", str(REPO / "target/release/larmorx"))


def main(work: Path) -> None:
    work.mkdir(parents=True, exist_ok=True)
    x, y, z = np.mgrid[:12, :12, :10]
    head = (100.0 + 0 * x) * (((x - 5.5) ** 2 + (y - 5.5) ** 2 + (z - 4.5) ** 2) < 16)
    head[6, 6, 5] = -0.0
    env = {"HOME": str(work), "PATH": "/usr/bin:/bin", "AFNI_DONT_LOGFILE": "YES",
           "AFNI_NIFTI_NOEXT": "YES", "LARMORX_IMPLEMENTATION": "original",
           "LARMORX_UPSTREAM_DEFECTS": "reproduce"}  # fmt: skip
    for dtype in ("float64", "float32"):
        for label, slope, inter in (("none", None, None), ("nan", np.nan, np.nan),
                                    ("2,0", 2.0, 0.0), ("2,1", 2.0, 1.0), ("1,0", 1.0, 0.0)):  # fmt: skip
            img = nib.Nifti1Image(head.astype(dtype), np.diag([2.0, 2.0, 2.0, 1.0]))
            if slope is not None:
                img.header.set_slope_inter(slope, inter)
            p = work / f"in-{dtype}-{label.replace(',', '_')}.nii"
            nib.save(img, p)
            if slope is not None and np.isnan(slope):
                raw = bytearray(p.read_bytes())
                raw[112:120] = np.array([np.nan, np.nan], "<f4").tobytes()
                p.write_bytes(bytes(raw))
            row = []
            for who, cmd in (("afni", [str(AFNI / "3dAutomask")]), ("lx", [LX, "afni", "3dAutomask"])):
                d = work / f"{who}-{p.stem}"
                d.mkdir(exist_ok=True)
                for f in d.iterdir():
                    f.unlink()
                subprocess.run([*cmd, "-q", "-apply_prefix", "m.nii", str(p)], cwd=d, env=env,
                               capture_output=True, check=True)  # fmt: skip
                v = np.asanyarray(nib.load(d / "m.nii").dataobj.get_unscaled())[6, 6, 5]
                row.append(f"{who} {v!r} signbit {bool(np.signbit(v))}")
            print(f"{dtype:8s} slope,inter {label:5s}: " + "; ".join(row))


if __name__ == "__main__":
    main(Path(sys.argv[1]).resolve())
