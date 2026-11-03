# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run; VB_DIR: the probes' directory, see README)
import os, sys
import sys; sys.path.insert(0,os.path.dirname(os.path.abspath(__file__)))
from vbg import *
V=os.environ['VB_DIR']+'/'
fl=lambda a:a.reshape(-1,order='F')
P,m,mode=sys.argv[1],sys.argv[2],sys.argv[3]
e=V+P+'/vb3D'; ys=effective(e,2)
if mode=='fix':
    y0=ys[0]; o=np.asarray(nib.load(e+'/eff0/hifi.nii.gz').dataobj).astype(float); ys[0]=2*o-y0
f=np.asarray(nib.load(f'{V}rep/{P}/hifi_{m}_{mode}.nii.gz').dataobj).astype(float)
pr=parse(f'{V}rep/{P}/out_{m}_{mode}.txt'); rl=pr[-1][0]; sh=f.shape
rhs=fl(ys[0]+ys[1]-2*f)/rl; ff=fl(f); nd_=3
basis={'c':{(0,0,0):1}}
for a in range(nd_):
    for k,s in [('1',1),('2',2)]:
        o1=[0]*nd_;o1[a]=s;o2=[0]*nd_;o2[a]=-s; basis[f'{"xyz"[a]}{k}']={tuple(o1):1,tuple(o2):1}
for a in range(nd_):
    for b in range(a+1,nd_):
        st={}
        for s in (1,-1):
            for t in (1,-1):
                o=[0]*nd_; o[a]=s; o[b]=t; st[tuple(o)]=1
        basis['xyz'[a]+'xyz'[b]]=st
cols=[];names=[]
for k,st in basis.items():
    v=stencil_Q(sh,int(sys.argv[4]) if len(sys.argv)>4 else 1,st)@ff
    if np.any(v): cols.append(v); names.append(k)
A=np.array(cols).T; th,*_=np.linalg.lstsq(A,rhs,rcond=None)
print(P,m,mode,{n:float('%.5g'%t) for n,t in zip(names,th)},'resid rel %.2g'%(np.linalg.norm(A@th-rhs)/np.linalg.norm(rhs)))
