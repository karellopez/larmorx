# SPDX-License-Identifier: Apache-2.0
"""Which of final_interp_probe.sh's outputs agree: every pair of corrected series (data as
stored) and of matrices, across the two FSL versions and the four final interpolations.

Usage: python -P compare_final_interp.py <the probe's output directory>
"""

from __future__ import annotations

import sys
from pathlib import Path

import nibabel as nib
import numpy as np


def main() -> int:
    root = Path(sys.argv[1])
    runs = sorted(p for p in root.iterdir() if (p / "out.nii.gz").is_file())
    data = {p.name: np.asanyarray(nib.load(p / "out.nii.gz").dataobj) for p in runs}
    pars = {p.name: (p / "out.par").read_text(encoding="utf-8") for p in runs}
    names = list(data)
    for i, a in enumerate(names):
        for b in names[i + 1 :]:
            same = data[a].tobytes() == data[b].tobytes()
            diff = float(np.max(np.abs(data[a].astype(float) - data[b].astype(float))))
            print(
                f"{a:28s} {b:28s} data {'identical' if same else f'differ (max {diff:.4g})'}"
                f"; .par {'identical' if pars[a] == pars[b] else 'differ'}"
            )
    return 0


if __name__ == "__main__":
    sys.exit(main())
