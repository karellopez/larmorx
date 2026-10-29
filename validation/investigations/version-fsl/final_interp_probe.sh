#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# mcflirt's final interpolation in FSL 6.0.7.17 and 6.0.7.23: the same series corrected with the
# default (trilinear), -sinc_final, -nn_final and -spline_final by both installations, in FSL's
# clean environment (FSL never on the PATH). compare_final_interp.py then says which outputs
# agree. Usage: final_interp_probe.sh <input series> <output directory>
set -euo pipefail
in=$1
out=$2
mkdir -p "$out"
for v in 6.0.7.17 6.0.7.23; do
  if [ "$v" = 6.0.7.17 ]; then fsldir=$HOME/fsl; else fsldir=$HOME/fsl-$v; fi
  for o in default sinc_final nn_final spline_final; do
    d=$out/$v-$o
    mkdir -p "$d"
    extra=()
    if [ "$o" != default ]; then extra=("-$o"); fi
    set +e
    env -i HOME="$HOME" PATH=/usr/bin:/bin FSLDIR="$fsldir" FSLOUTPUTTYPE=NIFTI_GZ \
      "$fsldir/bin/mcflirt" -in "$in" -out "$d/out" -mats -plots "${extra[@]}" >"$d/log" 2>&1
    echo "$v $o exit $?"
    set -e
  done
done
