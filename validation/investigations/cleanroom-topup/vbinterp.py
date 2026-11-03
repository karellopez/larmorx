# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run; VB_DIR: the probes' directory, see README)
import os, sys
import numpy as np, nibabel as nib
from scipy import ndimage as nd
V=os.environ['VB_DIR']+'/p17/jac/'
y0=np.asarray(nib.load(V+'p0.nii.gz').dataobj).astype(float); y1=np.asarray(nib.load(V+'p1.nii.gz').dataobj).astype(float)
o=np.asarray(nib.load(V+'hifi.nii.gz').dataobj).astype(float); m=o!=0
fm=2*o-y0
for sh in (-0.3,0.3):
    c=np.indices(y1.shape).astype(float); c[0]+=sh
    for pm in ['grid-wrap','mirror','reflect']:
        cf=nd.spline_filter(y1,order=3,mode=pm,output=np.float64)
        for em in ['grid-wrap','mirror','reflect']:
            v=nd.map_coordinates(cf,c,order=3,prefilter=False,mode=em)
            d=np.abs(v-fm)[m]
            if d.max()<50: print(sh,pm,em,'max %.3g p99 %.3g'%(d.max(),np.percentile(d,99)))
# where is the error largest for grid-wrap
c=np.indices(y1.shape).astype(float); c[0]-=0.3
v=nd.map_coordinates(y1,c,order=3,mode='grid-wrap'); d=np.abs(v-fm)*m
print('per x max err',[float('%.2g'%d[x].max()) for x in range(16)])
print('per z max err',[float('%.2g'%d[:,:,z].max()) for z in range(16)])
