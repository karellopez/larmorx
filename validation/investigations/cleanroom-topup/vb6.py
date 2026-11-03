# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run; VB_DIR: the probes' directory, see README)
import os, sys
import sys; sys.path.insert(0,os.path.dirname(os.path.abspath(__file__)))
from vbm import *
P=sys.argv[1]; m=sys.argv[2]
S=os.environ['VB_DIR']+f'/{P}/'
imgs=load(S+m,2); nx,ny,nz=imgs[0].shape
lsr=out(S+'lsr').reshape(nx,ny,nz,order='F'); SS=sopen(ny).T@sopen(ny)
y1=imgs[0]; y2=imgs[1]; y2e=np.empty_like(y2)
for x in range(nx):
    for z in range(nz): y2e[x,:,z]=(2*np.eye(ny)+0.01*SS)@lsr[x,:,z]-y1[x,:,z]
fl=lambda a:a.reshape(-1,order='F')
f=fl(out(S+m)); fsl=parse(S+m+'/STDOUT.txt'); rl=fsl[-1][0]
rhs=(fl(y1)+fl(y2e)-2*f)/rl
basis={'c':{(0,0,0):1},'x1':{(1,0,0):1,(-1,0,0):1},'y1':{(0,1,0):1,(0,-1,0):1},'z1':{(0,0,1):1,(0,0,-1):1},
 'x2':{(2,0,0):1,(-2,0,0):1},'y2':{(0,2,0):1,(0,-2,0):1},'z2':{(0,0,2):1,(0,0,-2):1},
 'xy':{(a,b,0):1 for a in(1,-1) for b in(1,-1)},'xz':{(a,0,b):1 for a in(1,-1) for b in(1,-1)},'yz':{(0,a,b):1 for a in(1,-1) for b in(1,-1)}}
cols=[];names=[]
for k,st in basis.items():
    Qk=stencil_Q((nx,ny,nz),1,st); v=Qk@f
    if np.any(v): cols.append(v); names.append(k)
A=np.array(cols).T
th,res,rk,sv=np.linalg.lstsq(A,rhs,rcond=None)
print('rank',rk,'of',len(names)); print({n:round(t,4) for n,t in zip(names,th)})
print('resid rel',np.linalg.norm(A@th-rhs)/np.linalg.norm(rhs))
