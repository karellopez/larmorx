# SPDX-License-Identifier: Apache-2.0
"""The qform quaternion nibabel writes for UniformGrid's regridded file (ds006736's PA run with
the AP run's affine) in this environment, next to fMRIPrep's own file. Run with each kept
environment's Python and inside fMRIPrep 25.2.6's image, with and without
``OPENBLAS_CORETYPE=Haswell``: ``python -I probe_quaternion.py <AP epi> <PA epi> <fMRIPrep regrid file>``
(prints nibabel, numpy, its BLAS version, the quaternion written, fMRIPrep's, the affine)."""
import sys, numpy as np, nibabel as nb
ap, pa, ref = sys.argv[1:4]
a, p = nb.load(ap), nb.load(pa)
img = nb.Nifti1Image(np.zeros(a.shape[:3], np.float32), a.affine, p.header)
img.update_header()
h = img.header
cfg = np.show_config(mode="dicts")["Build Dependencies"]["blas"]["version"]
print(nb.__version__, np.__version__, cfg, [float(h[k]) for k in ("quatern_b", "quatern_c", "quatern_d")], [float(nb.load(ref).header[k]) for k in ("quatern_b","quatern_c","quatern_d")], repr(a.affine.tolist()))
