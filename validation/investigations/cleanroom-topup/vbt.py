# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run; VB_DIR: the probes' directory, see README)
import os, sys
import sys; sys.path.insert(0,os.path.dirname(os.path.abspath(__file__)))
from vbg import *
V=os.environ['VB_DIR']+'/'
fl=lambda a:a.reshape(-1,order='F')
ORDER=0
HALF=True
VOX=None
def run(P, m, yz_axes='pe', count='all', raw=False, cross_vol=False):
    global VOX
    d=V+P+'/'+m; e=V+P+'/vb3D'
    VOX=[abs(float(v)) for v in nib.load(d+'/p0.nii.gz').header.get_zooms()[:3]]
    acq=[list(map(float,l.split())) for l in open(d+'/acqw.txt')]
    nimg=len(acq)
    ys=effective(e,nimg) if not raw else [np.asarray(nib.load(f'{d}/p{a}.nii.gz').dataobj).astype(float) for a in range(nimg)]
    coef=np.asarray(nib.load(d+'/tu_fieldcoef.nii.gz').dataobj).astype(float); h=fl(field_k1(coef))
    sh=ys[0].shape[:3]; nv=ys[0].shape[3] if ys[0].ndim==4 else 1
    vols=lambda y,v: y[...,v] if y.ndim==4 else y
    o=out(d); o=o if o.ndim==4 else o[...,None]
    total=np.zeros(sh+(nv,)); allres=[]
    colls={}
    for a,r in enumerate(acq): colls.setdefault(0 if r[0]!=0 else 1,[]).append(a)
    for pe in (sorted(colls) if not ORDER else sorted(colls,reverse=True)):
        print("  coll",pe)
        mem=colls[pe]; pts=[acq[a][pe]*acq[a][3] for a in mem]
        if m=='vb2D':
            vox=VOX; w=[vox[pe]/vox[a] for a in range(2)]; st=sten_w(w,pe); Q=stencil_Q(sh[:2],pe,st)
            for v in range(nv):
                for z in range(sh[2]):
                    r,f=vb_block([fl(vols(ys[a],v)[:,:,z]) for a in mem],pts,h.reshape(sh,order='F')[:,:,z].reshape(-1,order='F'),sh[:2],pe,Q,50,count)
                    allres+=r[:-1]; total[:,:,z,v]+=f.reshape(sh[:2],order='F')
        else:
            vox=VOX; w=[vox[pe]/vox[a] for a in range(3)]; st=sten_w(w,pe,half=HALF)
            Q=stencil_Q(sh,pe,st)
            if m=='vb3D':
                for v in range(nv):
                    r,f=vb_block([fl(vols(ys[a],v)) for a in mem],pts,h,sh,pe,Q,20,count)
                    allres+=r[:-1]; total[...,v]+=f.reshape(sh,order='F')
            else:
                N=int(np.prod(sh)); Q4=np.kron(np.eye(nv),Q)
                sh4=sh+(nv,)
                r,f=vb_block([np.concatenate([fl(vols(ys[a],v)) for v in range(nv)]) for a in mem],pts,np.tile(h,nv),sh4,pe,Q4,20,count)
                allres+=r[:-1]; total+=f.reshape(sh4,order='F')
    fsl=parse(d+'/STDOUT.txt')
    print('  first', allres[1], allres[21] if len(allres)>21 else '')
    print(P,m,'yz',yz_axes,count,'raw' if raw else 'eff',compare(allres,fsl,fl(total),fl(o)))
if __name__=='__main__':
    for spec in sys.argv[1:]:
        p,m,*rest=spec.split(':'); kw={}
        for r in rest:
            k,v=r.split('=');
            kw[k]=tuple(map(int,v.split(','))) if k=='yz_axes' else (v=='1' if k=='raw' else v)
        run(p,m,**kw)
