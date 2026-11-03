# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run; VB_DIR: the probes' directory, see README)
import os, sys
import sys; sys.path.insert(0,os.path.dirname(os.path.abspath(__file__)))
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)),'vbskip3.py')).read().split("w=[1,1,1]")[0])
from scipy import ndimage as nd
# coefficients of the periodic spline of raw1
cp=nd.spline_filter(raw1,order=3,mode='grid-wrap',output=np.float64)
def ev(c, ext):
    # value at x=-0.5 for every (y,z): B-spline weights at t=0.5 for coefficients x=-2..1
    import math
    t=0.5; w=[(1-t)**3/6,(3*t**3-6*t**2+4)/6,(-3*t**3+3*t**2+3*t+1)/6,t**3/6]
    # position -0.5: floor -1; coefficients -2,-1,0,1 with weights for u=-0.5-(-1)=0.5 -> w[i] for idx -2..1
    def cx(i):
        if 0<=i<c.shape[0]: return c[i]
        if ext=='wrap': return c[i%c.shape[0]]
        if ext=='zero': return np.zeros_like(c[0])
        if ext=='mirror': return c[-i] if i<0 else c[2*(c.shape[0]-1)-i]
        if ext=='nearest': return c[0] if i<0 else c[-1]
    return sum(wi*cx(i) for wi,i in zip(w,[-2,-1,0,1]))
AX=int(sys.argv[1]) if len(sys.argv)>1 else 0
cc=np.indices(raw1.shape).astype(float); cc[AX]-=0.5
sl=[slice(None)]*3; sl[AX]=0; sl=tuple(sl)
cands={e:nd.map_coordinates(cp,cc,order=3,prefilter=False,mode=e)[sl] for e in ['grid-wrap','mirror','reflect','nearest','grid-constant']}
cands['raw_edge']=raw1[sl]
fsl=parse(V+EFF+"/STDOUT.txt")
for nm,v in cands.items():
    wrap[sl]=v
    res=[]
    for z in range(sh[2]):
        ZZ=z
        Q=stencil_Q(sh[:2],pe,sten_w([1,1],pe)); r,f=model('vb2D','wrap',False,False,False,False,50,Q,sh[:2],[y[:,:,z] for y in ys]); res+=r[:-1]
    rel=[max(abs(a-b)/abs(b) for a,b in zip(x,z)) for x,z in zip(res,fsl)]
    print(nm,'printed max rel %.3g'%max(rel))
