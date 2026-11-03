# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run; VB_DIR: the probes' directory, see README)
import os, sys
import sys; sys.path.insert(0,os.path.dirname(os.path.abspath(__file__)))
from vbm import *
V=os.environ['VB_DIR']+'/'
y1,y2=load(V+'p4/jac',2); j=out(V+'p4/jac'); _,y2e=y2_effective(V+'p4/vb3D/')
print('jac vs raw mean: max',np.max(np.abs(j-(y1+y2)/2)),' jac vs eff mean: max',np.max(np.abs(j-(y1+y2e)/2)))
