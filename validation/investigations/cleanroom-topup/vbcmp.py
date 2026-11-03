# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run; VB_DIR: the probes' directory, see README)
import os, sys
import sys; sys.path.insert(0,os.path.dirname(os.path.abspath(__file__)))
from vbm import parse
import numpy as np, nibabel as nib, re
V=os.environ['VB_DIR']+'/'
mode=sys.argv[1]; ref=sys.argv[2]  # ref: fsl or rep
for pm in sys.argv[3:]:
    d=V+'mine/'+mode+'/'+pm
    if ref=='fsl': r=V+pm; rs=r+'/STDOUT.txt'; ro=r+'/hifi.nii.gz'
    else:
        p,m=pm.split('/'); rs=f'{V}rep/{p}/out_{m}_{mode}.txt'; ro=f'{V}rep/{p}/hifi_{m}_{mode}.nii.gz'
    a=open(d+'/STDOUT.txt').read(); b=open(rs).read()
    la=[re.sub(r'[-\d.e+]+','#',l) for l in a.splitlines()]; lb=[re.sub(r'[-\d.e+]+','#',l) for l in b.splitlines()]
    pa=parse(d+'/STDOUT.txt'); pb=parse(rs)
    rel=max((max(abs(x-y)/max(abs(y),1e-300) for x,y in zip(u,v)) for u,v in zip(pa,pb)),default=0)
    o=np.asarray(nib.load(d+'/hifi.nii.gz').dataobj).astype(float); f=np.asarray(nib.load(ro).dataobj).astype(float)
    m=np.abs(f)>0.1*np.percentile(np.abs(f),99)
    dr=np.abs(o-f)[m]/np.abs(f)[m]
    print(f'{pm:14s} exit {open(d+"/EXIT.txt").read().strip()} text-same {la==lb} ({len(la)}/{len(lb)}) printed-rel {rel:.2g} out p99 {np.percentile(dr,99):.2g} max {dr.max():.2g} zeros-same {np.array_equal(o==0,f==0)}')
