# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run; VB_DIR: the probes' directory, see README)
import os, sys
import sys; sys.path.insert(0,os.path.dirname(os.path.abspath(__file__)))
from vbm import *
from scipy import ndimage as nd
V=os.environ['VB_DIR']+'/'
np.set_printoptions(precision=3,suppress=True,linewidth=160)
P=sys.argv[1]
S=V+P+'/vb3D/'; y1,y2e=y2_effective(S); y1r,y2=load(S,2); d=y2e-y2
cols=[];names=[]
for nm,src in [('y1',y1),('y2',y2)]:
    for ax in range(3):
        for mode in ['wrap','nearest','mirror','reflect']:
            cols.append(nd.correlate1d(src,[1,-2,1],axis=ax,mode=mode).ravel()); names.append(f'{nm} d2 ax{ax} {mode}')
A=np.array(cols).T
for i,nm in enumerate(names):
    c=np.dot(A[:,i],d.ravel())/max(np.dot(A[:,i],A[:,i]),1e-9); r=np.linalg.norm(d.ravel()-c*A[:,i])/np.linalg.norm(d)
    if r<0.9: print(nm,'coef %.4g resid %.3f'%(c,r))
print(d[:,:,0]); print(d[:,:,-1])
A=np.array([y1.ravel(),y2.ravel(),np.ones(y1.size)]).T
c,*_=np.linalg.lstsq(A,d.ravel(),rcond=None); print('d ~ a y1 + b y2 + c:',c,'resid',np.linalg.norm(d.ravel()-A@c)/np.linalg.norm(d))
A=np.array([y2.ravel()]).T
c,*_=np.linalg.lstsq(A,d.ravel(),rcond=None); print('d ~ b y2:',c,'resid',np.linalg.norm(d.ravel()-A@c)/np.linalg.norm(d))
