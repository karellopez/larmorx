#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# mine.sh <mode> <probe/method>... : the original's applytopup on FSL's probe inputs, into vb/mine/<mode>/<probe>/<method>
VB_DIR=${VB_DIR:?set VB_DIR to the probes' directory}
W=$(cd "$(dirname "$0")/../../.." && pwd)
mode=$1; shift
for pm in "$@"; do
  src=$VB_DIR/${SRC:-}$pm; d=$VB_DIR/mine/$mode/${SRC:-}$pm; mkdir -p $d
  cp $src/p*.nii.gz $src/tu_fieldcoef.nii.gz $src/acqw.txt $src/tu_movpar.txt $d/
  nimg=$(ls $d/p*.nii.gz | wc -l); imain=$(for ((i=0;i<nimg;i++)); do printf "p$i.nii.gz,"; done); imain=${imain%,}
  m=$(basename $pm)
  (cd $d && FSLOUTPUTTYPE=NIFTI_GZ LARMORX_UPSTREAM_DEFECTS=$mode $W/target/release/larmorx fsl applytopup --imain=$imain --inindex=$(seq -s, 1 $nimg) --datain=acqw.txt --topup=tu --out=hifi --method=$m > STDOUT.txt 2> STDERR.txt; echo $? > EXIT.txt)
done
