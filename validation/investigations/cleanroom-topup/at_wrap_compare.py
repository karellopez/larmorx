# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
import sys
from pathlib import Path
import nibabel as nib, numpy as np

sys.path.insert(0, str(Path(__file__).parent))
import at_lsr as lsr

for d in sorted(Path(sys.argv[1]).iterdir()):
    want = np.asarray(nib.load(str(d / "hifi.nii.gz")).dataobj, np.float64)
    g = lsr.run(d, "")
    diff = np.abs(g - want)
    nz = np.argwhere(diff > 1e-3)
    lines = sorted({(int(a), int(c)) for a, b, c in nz})
    print(
        d.name,
        "max |d|",
        diff.max(),
        "lines differing",
        lines[:8],
        "FSL nonzero lines",
        sorted({(int(a), int(c)) for a, b, c in np.argwhere(np.abs(want) > 1e-6)})[:8],
    )
    x, y, z = map(int, d.name.split("_")[1:])
    print("  FSL line ", np.round(want[x, :, z], 2))
    print("  mine line", np.round(g[x, :, z], 2))
