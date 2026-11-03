# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run; VB_DIR: the probes' directory, see README)
import os, sys
import sys; sys.path.insert(0,os.path.dirname(os.path.abspath(__file__)))
from vbm import *
V=os.environ['VB_DIR']+'/'
np.set_printoptions(precision=2,suppress=True,linewidth=150)
y1,y2e=y2_effective(V+'p3/vb3D/'); y2=load(V+'p3/vb3D',2)[1]
d=y2e-y2
for z in range(2): print('z',z); print(d[:,:,z])
