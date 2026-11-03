# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run; VB_DIR: the probes' directory, see README)
import os, sys
import sys; sys.path.insert(0,os.path.dirname(os.path.abspath(__file__)))
from vbm import *
from scipy import ndimage as nd
V=os.environ['VB_DIR']+'/'
for P in ['p4','p3','p1']:
    S=V+P+'/vb3D/'; y1,y2e=y2_effective(S); y2=load(S,2)[1]
    best=[]
    for pm in ['mirror','reflect','grid-wrap']:
        c=nd.spline_filter(y2,order=3,mode=pm,output=np.float64)
        for em in ['wrap','mirror','reflect','nearest','constant']:
            v=c.copy()
            for ax in range(3):
                v=nd.correlate1d(v,[1/6,4/6,1/6],axis=ax,mode=em)
            best.append((np.max(np.abs(v-y2e)),pm,em))
    best.sort(); print(P,best[:4])
