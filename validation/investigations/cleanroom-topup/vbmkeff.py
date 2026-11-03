# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run; VB_DIR: the probes' directory, see README)
import os, sys
import sys; sys.path.insert(0,os.path.dirname(os.path.abspath(__file__)))
from vbg import effective
import numpy as np, nibabel as nib, shutil, os
V=os.environ['VB_DIR']+'/'
for pm in sys.argv[1:]:
    p,m=pm.split('/'); src=V+pm; d=V+'effin/'+pm; os.makedirs(d,exist_ok=True)
    nimg=len([f for f in os.listdir(src) if f.startswith('p') and f.endswith('.nii.gz')])
    ys=effective(V+p+'/vb3D',nimg)
    for a,y in enumerate(ys):
        i=nib.load(f'{src}/p{a}.nii.gz'); nib.save(nib.Nifti1Image(y.astype(np.float32),None,i.header),f'{d}/p{a}.nii.gz')
    for f in ['tu_fieldcoef.nii.gz','acqw.txt','tu_movpar.txt','STDOUT.txt','hifi.nii.gz']: shutil.copy(f'{src}/{f}',d)
