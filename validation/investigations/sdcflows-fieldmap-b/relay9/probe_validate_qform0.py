# SPDX-License-Identifier: Apache-2.0
import os
import tempfile
from pathlib import Path

import nibabel as nb
import numpy as np
from niworkflows.interfaces.header import ValidateImage
from scipy.spatial.transform import Rotation

import larmorx as lx

a = np.eye(4)
a[:3, :3] = (Rotation.from_rotvec([0.1, 0.05, -0.1]).as_matrix() * [2.0, 2.0, 2.5]).astype(
    np.float32
)
a[:3, 3] = np.float32([-40.0, 30.0, 12.0])
d = tempfile.mkdtemp()
img = nb.Nifti1Image(np.zeros((8, 9, 7), np.int16), None)
img.header.set_qform(a, 0)
img.header.set_sform(a, 1)
p = os.path.join(d, "v.nii.gz")
nb.save(img, p)
os.chdir(d)
r = ValidateImage(in_file=p).run()
print(repr(Path(r.outputs.out_report).read_text()))
m = lx.niworkflows.validate_image(p, out_dir=os.path.join(d, "m"), upstream_defects="reproduce")
print(repr(Path(m.out_report).read_text()))
