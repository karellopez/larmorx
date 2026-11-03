# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run; VB_DIR: the probes' directory, see README)
import os, sys
import sys; sys.path.insert(0,os.path.dirname(os.path.abspath(__file__)))
from vbg import *
from scipy import ndimage as nd
import itertools
import os
V=os.environ['VB_DIR']+'/'+os.environ.get('PROBE','p14')+'/'
EFF=os.environ.get('EFFM','vb3D')
fl=lambda a:a.reshape(-1,order='F')
ys=effective(V+EFF,2)
raw1=np.asarray(nib.load(V+EFF+'/p1.nii.gz').dataobj).astype(float)
AXIS=int(os.environ.get('AXIS','0'))
c=np.indices(raw1.shape).astype(float); c[AXIS]-=float(os.environ.get('SHIFT','0.5'))
wrap=nd.map_coordinates(raw1,c,order=3,mode='grid-wrap')
sh=raw1.shape; pe=1
bad=np.zeros(sh,bool); _s=[slice(None)]*3; _s[AXIS]=int(os.environ.get('EDGE','0')); bad[tuple(_s)]=True
def model(m, y1x0, init0, upd_skip, stats_valid, count_valid, iters, Q, shape, ys_):
    N=int(np.prod(shape)); lines=lines_of(shape,pe); n=shape[pe]
    badf=fl(bad if len(shape)==3 else bad[:,:,ZZ])
    valid=[not badf[L].any() for L in lines]
    y0=fl(ys_[0]); y1=fl(ys_[1]).copy()
    if y1x0=='zero': y1[badf]=0
    elif y1x0=='wrap': y1[badf]=fl(wrap if len(shape)==3 else wrap[:,:,ZZ])[badf]
    Y=[y0,y1]; cl=circ_line(n); f=np.zeros(N); Sig=[None]*len(lines)
    for i,L in enumerate(lines):
        Sig[i]=np.linalg.inv(2*np.eye(n)+cl)
        if valid[i] or not init0: f[L]=Sig[i]@(Y[0][L]+Y[1][L])
    phi=lam=1.0; res=[(1,1,1)]
    Nv=sum(len(L) for L,v in zip(lines,valid) if v) if count_valid else N
    nobs=2*Nv
    for it in range(iters):
        r2=0;tr=0;trq=0
        for i,L in enumerate(lines):
            if stats_valid and not valid[i]: continue
            r2+=np.sum((Y[0][L]-f[L])**2)+np.sum((Y[1][L]-f[L])**2); tr+=np.trace(2*Sig[i]); trq+=np.trace(Q[np.ix_(L,L)]@Sig[i])
        phi=(1e-10+nobs//2)/(1e-10+0.5*(r2+tr))
        lam=(1e-10+Nv//2)/(1e-10+0.5*(f@Q@f+trq))
        for i,L in enumerate(lines):
            if upd_skip and not valid[i]: continue
            rest=np.ones(N,bool); rest[L]=False
            Sig[i]=np.linalg.inv(phi*2*np.eye(n)+lam*Q[np.ix_(L,L)])
            f[L]=Sig[i]@(phi*(Y[0][L]+Y[1][L])-lam*Q[np.ix_(L,rest)]@f[rest])
        res.append((lam/phi,phi,lam))
    return res,f
w=[1,1,1]
best=[]
REP=len(sys.argv)>1
for m in ['vb3D','vb2D']:
    fsl=parse(V+m+'/STDOUT.txt') if not REP else parse(V.replace('/p14/','/rep/p14/')+'out_'+m+'_fix.txt')
    o=out(V+m) if not REP else np.asarray(nib.load(V.replace('/p14/','/rep/p14/')+'hifi_'+m+'_fix.nii.gz').dataobj).astype(float)
    for opts in itertools.product(['zero','wrap'],[True,False],[True,False],[True,False],[True,False]):
        if m=='vb3D':
            Q=stencil_Q(sh,pe,sten_w(w,pe,half=not REP)); res,f=model(m,*opts,20,Q,sh,ys)
            ff=f.copy(); ff[fl(bad)]=0; oo=fl(o)
        else:
            res=[];fs=[]
            for z in range(sh[2]):
                ZZ=z
                Q=stencil_Q(sh[:2],pe,sten_w(w[:2],pe)); r,f=model(m,*opts,50,Q,sh[:2],[y[:,:,z] for y in ys]); res+=r[:-1]; fs.append(f.reshape(sh[:2],order='F'))
            ff=np.stack(fs,axis=2); ff[bad]=0; ff=fl(ff); oo=fl(o)
        rel=[max(abs(a-b)/abs(b) for a,b in zip(x,z)) for x,z in zip(res,fsl)]
        mask=oo!=0
        best.append((max(rel),m,opts,np.max(np.abs(ff[mask]-oo[mask])/np.abs(oo[mask]))))
best.sort(key=lambda t:(t[1],t[0]))
for b in best:
    if b[0]<0.01: print(b)
print('best per method:',[b for b in best if b[1]=='vb2D'][:2],[b for b in best if b[1]=='vb3D'][:2])
