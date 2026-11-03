# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
import sys
from pathlib import Path
import nibabel as nib, numpy as np

sys.path.insert(0, str(Path(__file__).parent))
import at_lsr as lsr

d = Path(sys.argv[1])
lsr.WRAP = sys.argv[2] if len(sys.argv) > 2 else ""
want = np.asarray(nib.load(str(d / "hifi.nii.gz")).dataobj, np.float64)
g = lsr.run(d, "")
e = np.abs(g - want) / (np.abs(want) + 1)
bad = e > 1e-3
print("bad", bad.sum(), "by y", np.bincount(np.nonzero(bad)[1], minlength=g.shape[1]))
# lines with bad voxels
lines = {(x, z) for x, y, z in np.argwhere(bad)}
print("bad lines", len(lines), "of", g.shape[0] * g.shape[2])
x, z = sorted(lines)[0]
print("line", x, z, "rel err", np.round(e[x, :, z], 4))
