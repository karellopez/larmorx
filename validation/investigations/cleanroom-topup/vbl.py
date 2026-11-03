# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run; VB_DIR: the probes' directory, see README)
import os, sys
import sys; sys.path.insert(0,os.path.dirname(os.path.abspath(__file__)))
from vbg import *
V=os.environ['VB_DIR']+'/'
fl=lambda a:a.reshape(-1,order='F')
P=sys.argv[1]; d=V+P+'/lsr'
acq=[list(map(float,l.split())) for l in open(d+'/acqw.txt')]; nimg=len(acq)
ys=effective(V+P+'/vb3D',nimg)
coef=np.asarray(nib.load(d+'/tu_fieldcoef.nii.gz').dataobj).astype(float)
for fm in ['k1wrap','coef']:
    h=fl(field_k1(coef)) if fm=='k1wrap' else fl(coef)
    sh=ys[0].shape; total=np.zeros(int(np.prod(sh)))
    colls={}
    for a,r in enumerate(acq): colls.setdefault(0 if r[0]!=0 else 1,[]).append(a)
    for pe,mem in sorted(colls.items()):
        n=sh[pe]; SS=sopen(n).T@sopen(n)
        for L in lines_of(sh,pe):
            ks=[resampling_matrix(h[L],acq[a][pe]*acq[a][3]) for a in mem]
            A=sum(k.T@k for k in ks)+0.01*SS; b=sum(k.T@fl(ys[a])[L] for k,a in zip(ks,mem))
            total[L]+=np.linalg.solve(A,b)
    o=fl(out(d)); print(P,fm,'lsr max abs',np.max(np.abs(total-o)),'rel',np.max(np.abs(total-o))/np.max(np.abs(o)))
np.set_printoptions(precision=1,suppress=True,linewidth=160)
h=field_k1(coef); print('field line x=0,z=0',h[0,:,0]); print('coef', coef[0,:,0])
O=out(d); T=total.reshape(sh,order='F'); print('fsl',O[0,:,0]); print('mine',T[0,:,0]); print('raw sum/2',(ys[0]+ys[1])[0,:,0]/2)
