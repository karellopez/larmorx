# SPDX-License-Identifier: Apache-2.0
import os
import tempfile

import nibabel as nb
import numpy as np
import transforms3d
from scipy.spatial.transform import Rotation

from larmorx.io.nibabel_header import _get_qform, load_nibabel, set_xform
from larmorx.io.nifti import NiftiHeader
from larmorx.niworkflows.header import _axangle_and_translation
from larmorx.transforms._linalg import inv, matmul

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
n = nb.load(p)
q = n.get_qform()
s = n.get_sform()
n.set_qform(s, 1)
nq = n.get_qform()
L = load_nibabel(p)
mq = _get_qform(L.header)
f = set_xform(L.header.to_dict(), "qform", s, 1)
mnq = _get_qform(NiftiHeader.from_dict(f))
print("qform same", q.tobytes() == mq.tobytes(), "new qform same", nq.tobytes() == mnq.tobytes())
dn = np.linalg.inv(q) @ nq
dm = matmul(inv(mq), mnq)
print("diff same", dn.tobytes() == dm.tobytes(), np.abs(dn - dm).max())
T, R, Z, S = transforms3d.affines.decompose44(dn)
print(
    "t3d angle",
    transforms3d.axangles.mat2axangle(R)[1],
    "mine",
    _axangle_and_translation(dn)[0],
    _axangle_and_translation(dm)[0],
)
h = n.header
for k in ("quatern_b", "quatern_c", "quatern_d", "qoffset_x", "qoffset_y", "qoffset_z"):
    print(k, repr(float(h[k])), repr(float(f[k])))
print("pixdim", h["pixdim"][:4].tolist(), list(f["pixdim"])[:4])
print("sform", s.tolist())
