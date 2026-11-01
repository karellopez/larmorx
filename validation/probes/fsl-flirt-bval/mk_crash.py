# SPDX-License-Identifier: Apache-2.0
"""Make tiny constant images for FSL flirt crash reproduction: mk_crash.py <dir> <in shape> <in vox> <ref shape> <ref vox> [value]"""

import sys
from pathlib import Path

import nibabel as nib
import numpy as np

d = Path(sys.argv[1])
d.mkdir(parents=True, exist_ok=True)


def mk(name, shape, vox, val):
    shape = tuple(int(x) for x in shape.split("x"))
    vox = [float(x) for x in vox.split("x")]
    a = np.full(shape, val, np.float32)
    aff = np.diag([*vox, 1.0])
    img = nib.Nifti1Image(a, aff)
    img.header.set_xyzt_units("mm")
    img.set_qform(aff, 1)
    img.set_sform(aff, 1)
    nib.save(img, str(d / name))


val = float(sys.argv[6]) if len(sys.argv) > 6 else 1.0
mk("in.nii", sys.argv[2], sys.argv[3], val)
mk("ref.nii", sys.argv[4], sys.argv[5], val)
print("ok")
