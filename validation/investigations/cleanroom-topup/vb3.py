# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run; VB_DIR: the probes' directory, see README)
import os, sys
import numpy as np, nibabel as nib
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)),'vb2.py')).read().split("for mu in")[0])
lsr_fsl=np.asarray(nib.load(S+'lsr/hifi.nii.gz').dataobj).ravel()
SS=Sopen(8).T@Sopen(8)
mine=np.linalg.solve(KtK+0.01*SS,Kty)
print('lsr max rel diff',np.max(np.abs(mine-lsr_fsl)/lsr_fsl))
f=np.linalg.solve(KtK+Q1,Kty); r=Y-K@f; ss=r@r
print('ss',ss,'f0Q2f0',f@Q2@f,'f0Q3f0',f@Q3@f)
for nm,Qx in [('Q1',Q1),('Q2',Q2),('Q3',Q3)]:
    Sig=np.linalg.inv(KtK+Qx)
    print(nm,'trKtK',np.trace(KtK@Sig),'trQ2',np.trace(Q2@Sig),'trQ3',np.trace(Q3@Sig))
print('need Ephi in',8/(0.5*1.514025e-05),8/(0.5*1.514015e-05))
print('need E2 in',4/(0.5*5.975545e-07),4/(0.5*5.975535e-07))
print('need E3 in',4/(0.5*1.817895e-07),4/(0.5*1.817885e-07))
yeff=(KtK+0.01*SS)@lsr_fsl.astype(float)
print('y1+y2',y[0]+y[1]); print('yeff ',yeff); print('diff',yeff-(y[0]+y[1]))
y2e=yeff-y[0]; Ye=np.concatenate([y[0],y2e]); Ktye=K.T@Ye
for nm,Qx in [('Q1',Q1)]:
    Sig=np.linalg.inv(KtK+Qx); f=Sig@Ktye; r=Ye-K@f
    print('ss',r@r,'ss+tr',r@r+np.trace(KtK@Sig),'E2',f@Q2@f,f@Q2@f+np.trace(Q2@Sig),'E3',f@Q3@f,f@Q3@f+np.trace(Q3@Sig))
