#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# eff.sh <dir with probe inputs> : FSL's interpolation of each image a>=1 at its grid points
# (jac with a zero field: out = (y0 + interp(y_a)) / 2), into <dir>/eff<a>
VB_DIR=${VB_DIR:?set VB_DIR to the probes' directory}
W=$(cd "$(dirname "$0")/../../.." && pwd)
I=$W/validation/investigations/cleanroom-topup
src=$1
nimg=$(ls $src/p*.nii.gz | wc -l)
for ((a=1;a<nimg;a++)); do
  d=$src/eff$a; mkdir -p $d
  cp $src/p0.nii.gz $src/p$a.nii.gz $src/acqw.txt $src/tu_movpar.txt $d/
  python -c "
import nibabel as nib, numpy as np
i=nib.load('$src/tu_fieldcoef.nii.gz'); o=nib.Nifti1Image(np.zeros(i.shape,np.float32),None,i.header); nib.save(o,'$d/tu_fieldcoef.nii.gz')"
  python -P $I/at_case.py args $d - -- --imain=p0.nii.gz,p$a.nii.gz --inindex=1,$((a+1)) --datain=acqw.txt --topup=tu --out=hifi --method=jac
done
