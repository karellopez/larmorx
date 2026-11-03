# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run; VB_DIR: the probes' directory, see README)
import os, sys
import sys; sys.path.insert(0,os.path.dirname(os.path.abspath(__file__)))
import vbg, vbt, numpy as np, nibabel as nib
from scipy import ndimage as nd
V=sys.argv[1]
y1=np.asarray(nib.load(V+'p1.nii.gz').dataobj).astype(float)
ys=vbg.effective(V,2); e1=ys[1]
for sgn in (0.5,-0.5):
  for mode in ['grid-wrap','mirror','nearest','constant']:
    c=np.indices(y1.shape).astype(float); c[0]+=sgn
    v=nd.map_coordinates(y1,c,order=3,mode=mode)
    print(sgn,mode,'interior max diff',np.max(np.abs(v[1:]-e1[1:])),'x0 value sample',v[0,:3,0])
