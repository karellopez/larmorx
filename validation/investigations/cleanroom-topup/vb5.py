# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run; VB_DIR: the probes' directory, see README)
import os, sys
import sys; sys.path.insert(0,os.path.dirname(os.path.abspath(__file__)))
from vbm import *
P=sys.argv[1]; m=sys.argv[2]
S=os.environ['VB_DIR']+f'/{P}/'
imgs=load(S+m,2); shape=imgs[0].shape; nx,ny=shape[0],shape[1]; nz=shape[2] if len(shape)>2 else 1
lsr=out(S+'lsr').reshape(nx,ny,nz,order='F')
# effective second image from FSL's lsr, line by line
SS=sopen(ny).T@sopen(ny)
y1=imgs[0].reshape(nx,ny,nz,order='F'); y2=imgs[1].reshape(nx,ny,nz,order='F')
y2e=np.empty_like(y2)
for x in range(nx):
    for z in range(nz):
        y2e[x,:,z]=(2*np.eye(ny)+0.01*SS)@lsr[x,:,z]-y1[x,:,z]
print('y2 end deviation max',np.max(np.abs(y2e-y2)))
def flat(a): return a.reshape(-1,order='F')
def vb(y1,y2,shape,Q,iters,mode,a0=1e-10,b0=1e10):
    N=int(np.prod(shape)); nx,ny=shape[0],shape[1]
    Q1=np.kron(np.eye(N//(nx*ny)),np.kron(circ_line(ny),np.eye(nx)))
    Y=np.concatenate([y1,y2]); Kty=y1+y2
    KtK=2*np.eye(N)
    Sig=np.linalg.inv(KtK+Q1); f=Sig@Kty; phi=lam=1.0
    lines=[[x+nx*yy+nx*ny*z for yy in range(ny)] for z in range(N//(nx*ny)) for x in range(nx)]
    res=[(lam/phi,phi,lam)]
    def blocktrace(M,Sig): return np.trace(M@Sig)
    for it in range(iters):
        r=np.concatenate([y1-f,y2-f])
        phi=(a0+(2*N)//2)/(1/b0+0.5*(r@r+np.trace(KtK@Sig)))
        lam=(a0+N//2)/(1/b0+0.5*(f@Q@f+np.trace(Q@Sig)))
        A=phi*KtK+lam*Q
        if mode=='joint':
            Sig=np.linalg.inv(A); f=phi*Sig@Kty
        else:
            Sig=np.zeros((N,N)); fn=f.copy()
            for L in lines:
                L=np.array(L); rest=np.setdiff1d(np.arange(N),L)
                Al=A[np.ix_(L,L)]; Sl=np.linalg.inv(Al)
                src=fn if mode=='gs' else f
                fn[L]=Sl@(phi*Kty[L]-lam*Q[np.ix_(L,rest)]@src[rest])
                Sig[np.ix_(L,L)]=Sl
            f=fn
        res.append((lam/phi,phi,lam))
    return res,f
fsl=parse(S+m+'/STDOUT.txt'); o=flat(out(S+m))
its={'vb2D':50,'vb3D':20,'vb4D':20}[m]
for mode in ['joint','gs','jacobi']:
    for lab,yy2 in [('eff',y2e)]:
        if m=='vb2D':
            allres=[];fs=[]
            for z in range(nz):
                Q=lap_sq((nx,ny),1)
                r,f=vb(flat(y1[:,:,z]),flat(yy2[:,:,z]),(nx,ny),Q,its,mode); allres+=r; fs.append(f)
            f=np.concatenate(fs)
        else:
            Q=lap_sq((nx,ny,nz),1); allres,f=vb(flat(y1),flat(yy2),(nx,ny,nz),Q,its,mode)
        rel=[max(abs(a-b)/abs(b) for a,b in zip(x,z)) for x,z in zip(allres,fsl)]
        print(m,mode,lab,len(fsl),len(allres),'printed max rel %.2g'%max(rel),'first>1e-5',next((i for i,v in enumerate(rel) if v>1e-5),None),'out rel %.2g'%np.max(np.abs(f-o)/np.abs(o)))
        if mode=='joint': print('  fsl',fsl[:3]); print('  mine',[tuple('%.6g'%v for v in t) for t in allres[:3]])
