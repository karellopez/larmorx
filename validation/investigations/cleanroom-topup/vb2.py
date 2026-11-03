# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run; VB_DIR: the probes' directory, see README)
import os, sys
import numpy as np, nibabel as nib
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)),'vb1.py')).read().split("target_phi=")[0])
def circ5(c,a,b,n=8):
    q=np.zeros((n,n))
    for i in range(n):
        q[i,i]+=c
        for d in (1,-1): q[i,(i+d)%n]+=a
        for d in (2,-2): q[i,(i+d)%n]+=b
    return q
Q1=circ5(6,-4,1); Q2=circ5(20,-8,1); Q3=circ5(42,-12,1)
for mu in [1,0.5,0.3,0.2,0.1,0.05,0.01]:
    A=KtK+mu*Q1; Sig=np.linalg.inv(A); f=Sig@Kty; r=Y-K@f
    phi=8/(0.5*(r@r+np.trace(KtK@Sig))); phis=8/(0.5*(r@r))
    l2=4/(0.5*(f@Q2@f+np.trace(Q2@Sig))); l3=4/(0.5*(f@Q3@f+np.trace(Q3@Sig)))
    l2s=4/(0.5*(f@Q2@f)); l3s=4/(0.5*(f@Q3@f))
    print(mu,'phi %.6g %.6g  l2 %.6g %.6g  l3 %.6g %.6g'%(phi,phis,l2,l2s,l3,l3s))
print('target 1.51402e-05 5.97554e-07 1.81789e-07')
