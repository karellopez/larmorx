# SPDX-License-Identifier: Apache-2.0
"""Runs one command of AFNI's probe (volreg_probe, built by build.sh) and of the replica's
(`cargo run --example volreg_probe`) on the same raw float32 volume, and compares the outputs
bit for bit.

Usage: python -P compare_probe.py <c probe> <rust probe> <input.nii[.gz] or .raw nx ny nz>
       <command> <arguments after the dimensions and before the output ...>
The input's first volume is written as raw float32 next to the outputs (in a temporary folder).
"""

import subprocess
import sys
import tempfile
from pathlib import Path

import nibabel as nib
import numpy as np


def main() -> int:
    cprobe, rprobe, src, cmd, *rest = sys.argv[1:]
    img = nib.load(src)
    data = np.asarray(img.dataobj, dtype=np.float32)
    if data.ndim == 4:
        data = data[..., 0]
    nx, ny, nz = data.shape
    vox = [abs(float(v)) for v in img.header.get_zooms()[:3]]
    with tempfile.TemporaryDirectory() as tmp:
        t = Path(tmp)
        raw = t / "in.raw"
        data.ravel(order="F").astype("<f4").tofile(raw)
        args = [cmd, str(raw), str(nx), str(ny), str(nz), *map(str, vox), *rest]
        outs = []
        for name, probe in (("afni", cprobe), ("larmorx", rprobe)):
            out = t / f"{name}.raw"
            p = subprocess.run([probe, *args, str(out)], capture_output=True, text=True)
            if p.returncode != 0:
                print(name, "failed:", p.stderr)
                return 1
            outs.append(np.fromfile(out, dtype="<f4"))
        a, b = outs
        diff = a.view(np.uint32) != b.view(np.uint32)
        nd = int(diff.sum())
        if nd == 0:
            print(f"{cmd}: bit-identical ({a.size} voxels)")
        else:
            i = int(np.argmax(diff))
            print(
                f"{cmd}: {nd} of {a.size} differ, max |d| {np.abs(a - b).max():.4g}; "
                f"first at {np.unravel_index(i, (nx, ny, nz), order='F')}: {a[i]!r} vs {b[i]!r}"
            )
    return 0


if __name__ == "__main__":
    sys.exit(main())
