# SPDX-License-Identifier: Apache-2.0
"""Black-box probes of FSL 6.0.7.23's fslmaths for its clean-room original (relay 1): the other
side of the thresholds (FSL-152) per computing type, -save's type and display range, and the
operand rules of images of other sizes (FSL-159). Every FSL run goes through the oracle-output
cache (the replica suite's runner). Run: ``python -P probe_thresholds.py``."""

from __future__ import annotations

import sys
import tempfile
from pathlib import Path

import nibabel as nib
import numpy as np

from larmorx_validation.parity import fsl_maths_replica as rep


def save(d: Path, name: str, a, dtype, shape=None) -> str:
    a = np.asarray(a, dtype=dtype)
    if shape is not None:
        a = a.reshape(shape, order="F")
    img = nib.Nifti1Image(a, np.diag([-2.0, 2.0, 2.0, 1.0]))
    img.set_sform(img.affine, 1)
    img.set_qform(img.affine, 1)
    nib.save(img, str(d / f"{name}.nii.gz"))
    return f"{name}.nii.gz"


def show(d: Path, args: list[str]) -> None:
    p = rep.oracle_run(args, cwd=d, settings={})
    outs = sorted(x for x in d.glob("o*.nii.gz"))
    err = " | ".join(p.stderr.strip().splitlines())[:300]
    print("$ fslmaths", " ".join(args), "-> exit", p.returncode, err, p.stdout.strip()[:100])
    for o in outs:
        img = nib.load(str(o))
        a = np.asarray(img.dataobj).ravel(order="F")
        cal = (float(img.header["cal_min"]), float(img.header["cal_max"]))
        print(f"   {o.name}: {img.get_data_dtype()} {a.tolist()} cal={cal}")
        o.unlink()


PROBES = (
    ["f", "-thr", "0", "o1"],
    ["f", "-bin", "o1"],
    ["f", "-uthr", "1e9", "o1"],
    ["finf", "-thr", "0", "o1"],
    ["finf", "-uthr", "10", "o1"],
    ["finf", "-bin", "o1"],
    ["finf", "-binv", "o1"],
    ["f", "-mas", "finf", "o1"],
    ["-dt", "short", "s", "-thr", "0", "o1"],
    ["-dt", "short", "s", "-uthr", "10", "o1"],
    ["-dt", "short", "s", "-bin", "o1"],
    ["-dt", "char", "c", "-thr", "0", "o1"],
    ["-dt", "char", "c", "-uthr", "10", "o1"],
    ["-dt", "int", "i", "-thr", "0", "o1"],
    ["-dt", "int", "i", "-uthr", "10", "o1"],
    ["-dt", "int", "i", "-bin", "o1"],
    ["-dt", "char", "c", "-add", "1", "-save", "o2", "-range", "o1"],
    ["-dt", "short", "s", "-save", "o2", "o1"],
    ["f", "-range", "-mul", "2", "-save", "o2", "o1"],
    ["a34", "-div", "a24", "o1"],
    ["a34", "-max", "a44", "o1"],
    ["a34", "-min", "a44", "o1"],
    ["a34", "-rem", "a24", "o1"],
)


def main() -> int:
    with tempfile.TemporaryDirectory() as t:
        d = Path(t)
        names = {
            "f": save(d, "f", [1.0, 2.0, 2.0**25, 5.0, 2.0**25], np.float32),
            "finf": save(d, "finf", [1.0, np.inf, -np.inf, 3.0, 0.0], np.float32),
            "s": save(d, "s", [32767, 5, -32768, 0], np.int16),
            "c": save(d, "c", [127, 5, -128, 0], np.int8),
            "i": save(d, "i", [2147483647, 5, -2147483648, 0], np.int32),
            # 3 x 4 x 1 (values 1..12), 2 x 4 x 1 (101..108), 4 x 4 x 1 (201..216)
            "a34": save(d, "a34", np.arange(1, 13), np.float32, (3, 4, 1)),
            "a24": save(d, "a24", np.arange(101, 109), np.float32, (2, 4, 1)),
            "a44": save(d, "a44", np.arange(201, 217) % 7 + 0.5, np.float32, (4, 4, 1)),
        }
        for args in PROBES:
            show(d, [names.get(a, a) for a in args])
    return 0


if __name__ == "__main__":
    sys.exit(main())
