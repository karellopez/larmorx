#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
# probe.sh <name> <dims> <hz> "<gen opts>" <method...>   : FSL's applytopup through the cache
VB_DIR=${VB_DIR:?set VB_DIR to the probes' directory}
W=$(cd "$(dirname "$0")/../../.." && pwd)
I=$W/validation/investigations/cleanroom-topup
name=$1 dims=$2 hz=$3 opts=$4; shift 4
for m in "$@"; do
  d=$VB_DIR/$name/$m
  eval python -P $I/vb_probe.py $d $dims $hz $opts
  nimg=$(ls $d/p*.nii.gz | wc -l)
  imain=$(for ((i=0;i<nimg;i++)); do printf "p$i.nii.gz,"; done); imain=${imain%,}
  idx=$(seq -s, 1 $nimg)
  python -P $I/at_case.py args $d - -- --imain=$imain --inindex=$idx --datain=acqw.txt --topup=tu --out=hifi --method=$m
done
