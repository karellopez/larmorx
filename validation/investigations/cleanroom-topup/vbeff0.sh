#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# eff0.sh <dir>: FSL's spline interpolation of image 0 (jac of p0 twice, zero field) into <dir>/eff0
VB_DIR=${VB_DIR:?set VB_DIR to the probes' directory}
W=$(cd "$(dirname "$0")/../../.." && pwd)
src=$1; d=$src/eff0; mkdir -p $d
cp $src/p0.nii.gz $src/acqw.txt $src/tu_movpar.txt $src/eff1/tu_fieldcoef.nii.gz $d/
python -P $W/validation/investigations/cleanroom-topup/at_case.py args $d - -- --imain=p0.nii.gz,p0.nii.gz --inindex=1,1 --datain=acqw.txt --topup=tu --out=hifi --method=jac
