#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# fixcmp.sh <case>...: the replica (black box) and the original in fix mode on an at/ case's inputs
VB_DIR=${VB_DIR:?set VB_DIR to the probes' directory}
W=$(cd "$(dirname "$0")/../../.." && pwd)
NC=${LARMORX_NC_PROGRAM:?the replica program larmorx-nc, built from crates-nc}
for c in "$@"; do
  d=$VB_DIR/fix/$c; mkdir -p $d; cp ${AT_DIR:?FSL's at/ case outputs (at_case.py cases)}/$c/*.nii.gz ${AT_DIR:?FSL's at/ case outputs (at_case.py cases)}/$c/*.txt $d/ 2>/dev/null
  args=$(cat ${AT_DIR:?FSL's at/ case outputs (at_case.py cases)}/$c/ARGS.txt)
  (cd $d && FSLOUTPUTTYPE=NIFTI_GZ LARMORX_UPSTREAM_DEFECTS=fix $NC fsl applytopup ${args/--out=hifi/--out=rep} > rep_stdout.txt 2> rep_stderr.txt; echo "rep $?" > EXITS.txt
   FSLOUTPUTTYPE=NIFTI_GZ LARMORX_UPSTREAM_DEFECTS=fix $W/target/release/larmorx fsl applytopup ${args/--out=hifi/--out=mine} > mine_stdout.txt 2> mine_stderr.txt; echo "mine $?" >> EXITS.txt)
done
