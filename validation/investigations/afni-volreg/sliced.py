# SPDX-License-Identifier: Apache-2.0
"""Re-make RobustAverage's `_sliced.nii.gz` (niworkflows/interfaces/images.py) from an oracle run's
gen_avg node: its in_file and t_mask from _report/report.rst.

Usage: sliced.py <gen_avg node dir> <BIDS root> <output.nii.gz>
"""

import ast
import re
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

node, bids, out = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
rep = (node / "_report" / "report.rst").read_text()
exe = rep.split("Execution Inputs")[1]
in_file = re.search(r"\* in_file : (\S+)", exe).group(1)
t_mask = ast.literal_eval(re.search(r"\* t_mask : (\[.*?\])", exe).group(1))
nonneg = re.search(r"\* nonnegative : (\S+)", exe).group(1) == "True"
if in_file.startswith("/work/"):
    work = Path(*node.parts[: node.parts.index("work") + 1])
    src = work / Path(in_file).relative_to("/work")
else:
    src = bids / Path(in_file).relative_to("/data")
img = nib.load(src)
img.header.extensions.clear()
img = nib.squeeze_image(img)
sliced = nib.concat_images(i for i, t in zip(nib.four_to_three(img), t_mask, strict=False) if t)
data = sliced.get_fdata(dtype="float32")
data = np.clip(
    data,
    a_min=0.0 if nonneg else np.percentile(data, 0.2),
    a_max=np.percentile(data, 99.8),
)
gs_drift = np.mean(data, axis=(0, 1, 2))
gs_drift /= gs_drift.max()
data /= gs_drift
data = np.clip(data, a_min=0.0 if nonneg else data.min(), a_max=data.max())
sliced.__class__(data, sliced.affine, sliced.header).to_filename(out)
print(src, sum(t_mask), data.shape, out)
